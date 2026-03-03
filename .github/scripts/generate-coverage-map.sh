#!/usr/bin/env bash
set -euo pipefail

# Generates a JSON coverage map: { "tests/foo_test.rs": ["src/bar.rs:10", ...], ... }
# Only covers integration tests (tests/*_test.rs). Unit tests live in source files,
# so source changes already trigger mutant list diffs.

# Setup coverage instrumentation
eval "$(cargo llvm-cov show-env --export-prefix)"
cargo llvm-cov clean --workspace
cargo test --no-run 2>/dev/null

echo '{}' > coverage-map.json

for test_file in tests/*_test.rs; do
  [ -f "$test_file" ] || continue
  test_name=$(basename "$test_file" .rs)

  # Clean previous profraw files
  find "${CARGO_LLVM_COV_TARGET_DIR:-target}" -name "*.profraw" -delete 2>/dev/null || true

  # Run this integration test file
  cargo test --test "$test_name" 2>/dev/null || true

  # Export coverage as lcov
  cargo llvm-cov report --lcov > "coverage_${test_name}.lcov" 2>/dev/null || true

  # Parse lcov: extract covered source lines (skip test files themselves)
  LINES=$(awk '
    /^SF:/ { file = substr($0, 4) }
    /^DA:/ {
      split(substr($0, 4), a, ",")
      if (a[2]+0 > 0 && file !~ /^tests\//) print file ":" a[1]
    }
  ' "coverage_${test_name}.lcov" | sort -u)

  # Add to coverage map
  LINES_JSON=$(echo "$LINES" | jq -R 'select(length > 0)' | jq -s .)
  jq --arg key "$test_file" --argjson lines "$LINES_JSON" \
    '.[$key] = $lines' coverage-map.json > tmp.json && mv tmp.json coverage-map.json

  rm -f "coverage_${test_name}.lcov"
done
