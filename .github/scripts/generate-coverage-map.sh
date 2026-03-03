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

JOBS="${COVERAGE_JOBS:-4}"
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

# Discover binaries from build messages
declare -A BINARY_EXECUTABLE  # target name -> executable path
declare -A BINARY_SOURCE      # target name -> relative source path (tests only)
declare -A BINARY_KIND        # target name -> kind (lib|test|bin)

while IFS=$'\t' read -r kind name src_path executable; do
  BINARY_EXECUTABLE[$name]="$executable"
  BINARY_KIND[$name]="$kind"

  if [ "$kind" = "test" ]; then
    BINARY_SOURCE[$name]="${src_path#"$PROJECT_ROOT"}"
  fi
done < <(jq -r 'select(.reason == "compiler-artifact" and .executable != null) |
  [.target.kind[0], .target.name, .target.src_path, .executable] | @tsv' build_messages.json)

# Collect all unique executables for llvm-cov object passing
ALL_OBJECTS=()
for name in "${!BINARY_EXECUTABLE[@]}"; do
  ALL_OBJECTS+=("${BINARY_EXECUTABLE[$name]}")
done

echo "Found ${#ALL_OBJECTS[@]} binaries: ${!BINARY_EXECUTABLE[*]}"

# Create working directories
mkdir -p profraw profdata lcov entries

# Enumerate all tests and build test list
# Format: safe_name|binary_path|full_test_name|source_file
: > test_list.txt

for name in "${!BINARY_EXECUTABLE[@]}"; do
  binary="${BINARY_EXECUTABLE[$name]}"
  kind="${BINARY_KIND[$name]}"

  [ "$kind" = "lib" ] || [ "$kind" = "test" ] || continue

  while IFS= read -r test_name; do
    [ -z "$test_name" ] && continue

    if [ "$kind" = "test" ]; then
      source_file="${BINARY_SOURCE[$name]}"
      full_test_name="${name}::${test_name}"
    else
      # Unit test: derive source file from module path
      # e.g., cli::command::tests::foo -> cli::command -> src/cli/command.rs
      module_path=$(echo "$test_name" | sed 's/::tests::.*$//' | sed 's/::/\//g')
      if [ -f "src/${module_path}.rs" ]; then
        source_file="src/${module_path}.rs"
      else
        source_file="src/${module_path}/mod.rs"
      fi
      full_test_name="$test_name"
    fi

    safe_name=$(echo "${name}__${test_name}" | tr -c 'a-zA-Z0-9\n' '_')
    echo "${safe_name}|${binary}|${full_test_name}|${source_file}" >> test_list.txt
  done < <(LLVM_PROFILE_FILE=/dev/null "$binary" --list 2>/dev/null | grep ': test$' | sed 's/: test$//')
done

TOTAL=$(wc -l < test_list.txt | tr -d ' ')
echo "Enumerated $TOTAL tests to profile"

# Phase 1: Run each test with unique profraw path (parallel via xargs)
echo "Phase 1: Running tests..."
while IFS='|' read -r safe_name binary test_name _; do
  printf '%s\0%s\0%s\0' "$safe_name" "$binary" "$test_name"
done < test_list.txt | xargs -0 -n3 -P "$JOBS" sh -c '
  LLVM_PROFILE_FILE="profraw/$1_%p_%m.profraw" "$2" "$3" --exact --test-threads=1 > /dev/null 2>&1; exit 0
' _

# Phase 2: Merge profiles and export lcov (parallel via xargs)
echo "Phase 2: Processing profiles..."
export LLVM_PROFDATA LLVM_COV
ALL_OBJECTS_STR=$(printf '%s\n' "${ALL_OBJECTS[@]}")
export ALL_OBJECTS_STR

while IFS='|' read -r safe_name binary _ _; do
  printf '%s\0%s\0' "$safe_name" "$binary"
done < test_list.txt | xargs -0 -n2 -P "$JOBS" bash -c '
  safe_name="$1"; binary="$2"
  shopt -s nullglob
  profraw_files=(profraw/${safe_name}_*.profraw)
  shopt -u nullglob
  [ ${#profraw_files[@]} -gt 0 ] || exit 0

  "$LLVM_PROFDATA" merge -sparse "${profraw_files[@]}" -o "profdata/${safe_name}.profdata"

  extra_objects=()
  while IFS= read -r obj; do
    [ -n "$obj" ] && [ "$obj" != "$binary" ] && extra_objects+=("--object=$obj")
  done <<< "$ALL_OBJECTS_STR"

  "$LLVM_COV" export --format=lcov \
    --instr-profile="profdata/${safe_name}.profdata" \
    "$binary" "${extra_objects[@]}" \
    > "lcov/${safe_name}.lcov" 2>/dev/null || true
' _

# Phase 3: Parse lcov files and build individual JSON entries
echo "Phase 3: Building coverage map..."
while IFS='|' read -r safe_name binary test_name source_file; do
  lcov_file="lcov/${safe_name}.lcov"
  [ -f "$lcov_file" ] || continue

  # Parse lcov: extract covered source lines (skip tests/ files, strip absolute paths)
  awk -v root="$PROJECT_ROOT" '
    /^SF:/ {
      file = substr($0, 4)
      if (index(file, root) == 1) file = substr(file, length(root) + 1)
    }
    /^DA:/ {
      split(substr($0, 4), a, ",")
      if (a[2]+0 > 0 && file !~ /^tests\//) print file ":" a[1]
    }
  ' "$lcov_file" | sort -u | jq -R 'select(length > 0)' | jq -s \
    --arg key "$test_name" --arg sf "$source_file" \
    '{($key): {"source_file": $sf, "covered_lines": .}}' \
    > "entries/${safe_name}.json"
done < test_list.txt

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
rm -rf profraw profdata lcov entries build_messages.json test_list.txt

ENTRIES=$(jq 'length' coverage-map.json)
echo "Coverage map generated with $ENTRIES test entries"
