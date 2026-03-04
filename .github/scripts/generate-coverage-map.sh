#!/usr/bin/env bash
set -euo pipefail

# Generates a per-test-case JSON coverage map:
# {
#   "cli::command::tests::parses_no_arguments": {
#     "source_file": "src/cli/command.rs",
#     "covered_lines": ["src/cli/command.rs:196", "src/cli/runner.rs:15"]
#   },
#   "cli_test::add_stdio_writes_config_file": {
#     "source_file": "tests/cli_test.rs",
#     "covered_lines": ["src/cli/command.rs:178", "src/config/store.rs:20"]
#   }
# }

JOBS="${COVERAGE_JOBS:-$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)}"
PROJECT_ROOT="$(pwd)/"

# LLVM tools from Rust toolchain
SYSROOT=$(rustc --print sysroot)
HOST=$(rustc -vV | sed -n 's|host: ||p')
LLVM_PROFDATA="$SYSROOT/lib/rustlib/$HOST/bin/llvm-profdata"
LLVM_COV="$SYSROOT/lib/rustlib/$HOST/bin/llvm-cov"

# Setup instrumented build environment
eval "$(cargo llvm-cov show-env --export-prefix)"
cargo llvm-cov clean --workspace

# Build all test and bin targets (instrumented) and capture build messages
cargo test --no-run --message-format=json 2>/dev/null > build_messages.json
cargo build --message-format=json 2>/dev/null >> build_messages.json

# Discover binaries and enumerate tests — no associative arrays needed
binary_count=$(jq -r 'select(.reason == "compiler-artifact" and .executable != null) | .target.name' build_messages.json | sort -u | wc -l | tr -d ' ')
echo "Found $binary_count binaries"

mkdir -p entries
: > test_list.txt

while IFS=$'\t' read -r kind name src_path executable; do
  [ "$kind" = "lib" ] || [ "$kind" = "test" ] || continue

  while IFS= read -r test_name; do
    [ -z "$test_name" ] && continue

    if [ "$kind" = "test" ]; then
      source_file="${src_path#"$PROJECT_ROOT"}"
      full_test_name="${name}::${test_name}"
    else
      module_path=$(echo "$test_name" | sed 's/::tests::.*$//' | sed 's/::/\//g')
      if [ -f "src/${module_path}.rs" ]; then
        source_file="src/${module_path}.rs"
      else
        source_file="src/${module_path}/mod.rs"
      fi
      full_test_name="$test_name"
    fi

    safe_name=$(echo "${name}__${test_name}" | tr -c 'a-zA-Z0-9\n' '_')
    echo "${safe_name}|${executable}|${full_test_name}|${source_file}" >> test_list.txt
  done < <(LLVM_PROFILE_FILE=/dev/null "$executable" --list 2>/dev/null | grep ': test$' | sed 's/: test$//')
done < <(jq -r 'select(.reason == "compiler-artifact" and .executable != null) |
  [.target.kind[0], .target.name, .target.src_path, .executable] | @tsv' build_messages.json)

TOTAL=$(wc -l < test_list.txt | tr -d ' ')
echo "Enumerated $TOTAL tests to profile"

# Single-pass pipeline: run test → merge → export lcov → parse → JSON entry
echo "Processing tests ($JOBS parallel workers)..."
export LLVM_PROFDATA LLVM_COV PROJECT_ROOT

while IFS='|' read -r safe_name binary test_name source_file; do
  printf '%s\0%s\0%s\0%s\0' "$safe_name" "$binary" "$test_name" "$source_file"
done < test_list.txt | xargs -0 -n4 -P "$JOBS" bash -c '
  safe_name="$1"; binary="$2"; test_name="$3"; source_file="$4"
  profraw_dir=$(mktemp -d)

  # Run test
  LLVM_PROFILE_FILE="${profraw_dir}/${safe_name}_%p_%m.profraw" \
    "$binary" "$test_name" --exact --test-threads=1 > /dev/null 2>&1 || true

  # Merge profraw → profdata
  shopt -s nullglob
  profraw_files=("${profraw_dir}"/${safe_name}_*.profraw)
  shopt -u nullglob
  [ ${#profraw_files[@]} -gt 0 ] || { rm -rf "$profraw_dir"; exit 0; }

  profdata=$(mktemp)
  "$LLVM_PROFDATA" merge -sparse "${profraw_files[@]}" -o "$profdata"
  rm -rf "$profraw_dir"

  # Export lcov and parse directly into JSON entry (single binary, no extra objects)
  "$LLVM_COV" export --format=lcov \
    --instr-profile="$profdata" \
    "$binary" 2>/dev/null \
  | awk -v root="$PROJECT_ROOT" '\''
    /^SF:/ {
      file = substr($0, 4)
      if (index(file, root) == 1) file = substr(file, length(root) + 1)
    }
    /^DA:/ {
      split(substr($0, 4), a, ",")
      if (a[2]+0 > 0 && file !~ /^tests\//) print file ":" a[1]
    }
  '\'' | sort -u | jq -R "select(length > 0)" | jq -s \
    --arg key "$test_name" --arg sf "$source_file" \
    '\''{($key): {"source_file": $sf, "covered_lines": .}}'\'' \
    > "entries/${safe_name}.json"

  rm -f "$profdata"
' _

# Combine all entries into final coverage map
shopt -s nullglob
entry_files=(entries/*.json)
shopt -u nullglob

if [ ${#entry_files[@]} -gt 0 ]; then
  jq -s 'add // {}' "${entry_files[@]}" > coverage-map.json
else
  echo '{}' > coverage-map.json
fi

# Cleanup
rm -rf entries build_messages.json test_list.txt

ENTRIES=$(jq 'length' coverage-map.json)
echo "Coverage map generated with $ENTRIES test entries"
