#!/usr/bin/env bash
set -euo pipefail

# Test harness for find-mutants-to-test.sh

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
FIND_SCRIPT="$SCRIPT_DIR/../scripts/find-mutants-to-test.sh"

PASS=0
FAIL=0
ERRORS=""

setup_workdir() {
  local workdir
  workdir=$(mktemp -d)
  echo "$workdir"
}

make_mutants() {
  local file="$1"
  local count="$2"
  local start_line="${3:-10}"
  : > "$file"
  for i in $(seq 1 "$count"); do
    line=$((start_line + (i - 1) * 10))
    echo "src/file.rs:${line}:5: replace fn_${i} -> bool with false" >> "$file"
  done
  LC_ALL=C sort -o "$file" "$file"
}

run_test() {
  local test_name="$1"
  local workdir="$2"
  local expected_mode="$3"
  local expected_count="${4:-}"

  local output_file="$workdir/github_output.txt"
  : > "$output_file"

  local stdout
  stdout=$(cd "$workdir" && GITHUB_OUTPUT="$output_file" LC_ALL=C bash "$FIND_SCRIPT" 2>&1) || {
    FAIL=$((FAIL + 1))
    ERRORS="${ERRORS}\n  FAIL: $test_name — script exited with error:\n    $stdout"
    echo "  FAIL: $test_name — script error"
    rm -rf "$workdir"
    return
  }

  local actual_mode
  actual_mode=$(grep '^mode=' "$output_file" | cut -d= -f2)

  local failed=""
  [ "$actual_mode" != "$expected_mode" ] && failed="${failed}\n    mode: expected=$expected_mode actual=$actual_mode"

  if [ -n "$expected_count" ] && [ "$expected_mode" = "incremental" ]; then
    local actual_count
    actual_count=$(echo "$stdout" | grep 'Found' | sed 's/Found \([0-9]*\).*/\1/')
    [ "$actual_count" != "$expected_count" ] && failed="${failed}\n    count: expected=$expected_count actual=$actual_count"
  fi

  if [ -z "$failed" ]; then
    PASS=$((PASS + 1))
    echo "  PASS: $test_name (mode=$actual_mode)"
  else
    FAIL=$((FAIL + 1))
    ERRORS="${ERRORS}\n  FAIL: $test_name${failed}"
    echo "  FAIL: $test_name${failed}"
  fi

  rm -rf "$workdir"
}

echo "=== Testing find-mutants-to-test.sh ==="
echo ""

# -------------------------------------------------------------------
# Test 1: No previous baseline -> full
# -------------------------------------------------------------------
workdir=$(setup_workdir)
make_mutants "$workdir/current_mutants.txt" 10
run_test "no previous baseline -> full" "$workdir" "full"

# -------------------------------------------------------------------
# Test 2: Previous baseline but no killed list -> full
# -------------------------------------------------------------------
workdir=$(setup_workdir)
make_mutants "$workdir/current_mutants.txt" 10
make_mutants "$workdir/previous_mutants.txt" 10
run_test "no killed list -> full" "$workdir" "full"

# -------------------------------------------------------------------
# Test 3: No changes -> skip
# -------------------------------------------------------------------
workdir=$(setup_workdir)
make_mutants "$workdir/current_mutants.txt" 10
cp "$workdir/current_mutants.txt" "$workdir/previous_mutants.txt"
cp "$workdir/current_mutants.txt" "$workdir/previous_killed_mutants.txt"
run_test "no changes -> skip" "$workdir" "skip"

# -------------------------------------------------------------------
# Test 4: New mutant added -> incremental, 1 to test
# -------------------------------------------------------------------
workdir=$(setup_workdir)
make_mutants "$workdir/previous_mutants.txt" 10
make_mutants "$workdir/previous_killed_mutants.txt" 10
# Add fn_11
make_mutants "$workdir/current_mutants.txt" 11
run_test "1 new mutant -> incremental" "$workdir" "incremental" "1"

# -------------------------------------------------------------------
# Test 5: LINE SHIFT — mutants shifted, same content -> skip
# Old: fn_1..fn_10 at lines 10..100
# New: fn_1..fn_10 at lines 50..140
# All previously killed. Should skip (nothing truly new).
# -------------------------------------------------------------------
workdir=$(setup_workdir)
make_mutants "$workdir/previous_mutants.txt" 10 10
make_mutants "$workdir/previous_killed_mutants.txt" 10 10
make_mutants "$workdir/current_mutants.txt" 10 50
run_test "LINE SHIFT: all shifted, all killed -> skip" "$workdir" "skip"

# -------------------------------------------------------------------
# Test 6: LINE SHIFT — shifted + 1 new mutant -> incremental, only new
# -------------------------------------------------------------------
workdir=$(setup_workdir)
make_mutants "$workdir/previous_mutants.txt" 10 10
make_mutants "$workdir/previous_killed_mutants.txt" 10 10
# Current: fn_1..fn_10 shifted + fn_11 new
{
  for i in $(seq 1 10); do
    echo "src/file.rs:$((50 + (i-1)*10)):5: replace fn_${i} -> bool with false"
  done
  echo "src/file.rs:200:5: replace fn_11 -> bool with false"
} | LC_ALL=C sort > "$workdir/current_mutants.txt"
run_test "LINE SHIFT: shifted + 1 new -> incremental, 1 to test" "$workdir" "incremental" "1"

# -------------------------------------------------------------------
# Test 7: LINE SHIFT — shifted + 1 unproven -> incremental
# Old: fn_1..fn_10, killed fn_1..fn_9 (fn_10 missed)
# New: all shifted
# Should test fn_10 (unproven, since it was not in killed list)
# -------------------------------------------------------------------
workdir=$(setup_workdir)
make_mutants "$workdir/previous_mutants.txt" 10 10
# Kill only first 9
make_mutants "$workdir/previous_killed_mutants.txt" 9 10
make_mutants "$workdir/current_mutants.txt" 10 50
run_test "LINE SHIFT: 1 unproven -> incremental, 1 to test" "$workdir" "incremental" "1"

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
if [ -n "$ERRORS" ]; then
  echo ""
  echo "Failures:$ERRORS"
fi

[ "$FAIL" -eq 0 ]
