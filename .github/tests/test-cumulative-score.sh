#!/usr/bin/env bash
set -euo pipefail

# Test harness for calculate-cumulative-score.sh
# Uses synthetic fixtures to test every edge case deterministically.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CALC_SCRIPT="$SCRIPT_DIR/../scripts/calculate-cumulative-score.sh"

PASS=0
FAIL=0
ERRORS=""

setup_workdir() {
  local workdir
  workdir=$(mktemp -d)
  mkdir -p "$workdir/baseline" "$workdir/results/mutants-results-0" "$workdir/previous-killed"
  echo "$workdir"
}

# Create a sorted mutant list with N entries
# Each line: src/file.rs:LINE:5: replace fn_N -> bool with false
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
  local expected_score="$3"
  local expected_killed="$4"
  local expected_total="$5"

  local output_file="$workdir/github_output.txt"
  touch "$output_file"

  local stdout
  stdout=$(cd "$workdir" && GITHUB_OUTPUT="$output_file" LC_ALL=C bash "$CALC_SCRIPT" 2>&1) || {
    FAIL=$((FAIL + 1))
    ERRORS="${ERRORS}\n  FAIL: $test_name — script exited with error:\n    $stdout"
    echo "  FAIL: $test_name — script error"
    rm -rf "$workdir"
    return
  }

  local actual_score actual_killed actual_total
  actual_score=$(grep '^score=' "$output_file" | cut -d= -f2)
  actual_killed=$(echo "$stdout" | grep 'Mutation score:' | sed 's/.*killed=\([0-9]*\).*/\1/')
  actual_total=$(echo "$stdout" | grep 'Mutation score:' | sed 's/.*total=\([0-9]*\).*/\1/')

  local failed=""
  [ "$actual_score" != "$expected_score" ] && failed="${failed}\n    score: expected=$expected_score actual=$actual_score"
  [ "$actual_killed" != "$expected_killed" ] && failed="${failed}\n    killed: expected=$expected_killed actual=$actual_killed"
  [ "$actual_total" != "$expected_total" ] && failed="${failed}\n    total: expected=$expected_total actual=$actual_total"

  if [ -z "$failed" ]; then
    PASS=$((PASS + 1))
    echo "  PASS: $test_name"
  else
    FAIL=$((FAIL + 1))
    ERRORS="${ERRORS}\n  FAIL: $test_name${failed}"
    echo "  FAIL: $test_name${failed}"
  fi

  rm -rf "$workdir"
}

echo "=== Testing calculate-cumulative-score.sh ==="
echo ""

# -------------------------------------------------------------------
# Test 1: First run, no previous killed list — all caught
# -------------------------------------------------------------------
workdir=$(setup_workdir)
make_mutants "$workdir/baseline/current_mutants.txt" 10
cp "$workdir/baseline/current_mutants.txt" "$workdir/results/mutants-results-0/caught.txt"
: > "$workdir/results/mutants-results-0/missed.txt"
: > "$workdir/results/mutants-results-0/timeout.txt"
: > "$workdir/results/mutants-results-0/unviable.txt"
rm -f "$workdir/previous-killed/killed_mutants.txt"
run_test "first run, all caught, no previous" "$workdir" "100.0" 10 10

# -------------------------------------------------------------------
# Test 2: First run, no previous — some missed
# -------------------------------------------------------------------
workdir=$(setup_workdir)
make_mutants "$workdir/baseline/current_mutants.txt" 10
head -8 "$workdir/baseline/current_mutants.txt" > "$workdir/results/mutants-results-0/caught.txt"
tail -2 "$workdir/baseline/current_mutants.txt" > "$workdir/results/mutants-results-0/missed.txt"
: > "$workdir/results/mutants-results-0/timeout.txt"
: > "$workdir/results/mutants-results-0/unviable.txt"
rm -f "$workdir/previous-killed/killed_mutants.txt"
run_test "first run, 8 caught 2 missed" "$workdir" "80.0" 8 10

# -------------------------------------------------------------------
# Test 3: Incremental — all untested carry forward, new ones caught
# -------------------------------------------------------------------
workdir=$(setup_workdir)
make_mutants "$workdir/baseline/current_mutants.txt" 10
# Previous run killed all 10
cp "$workdir/baseline/current_mutants.txt" "$workdir/previous-killed/killed_mutants.txt"
# This run only tests 2 (the last 2) and catches both
tail -2 "$workdir/baseline/current_mutants.txt" > "$workdir/results/mutants-results-0/caught.txt"
: > "$workdir/results/mutants-results-0/missed.txt"
: > "$workdir/results/mutants-results-0/timeout.txt"
: > "$workdir/results/mutants-results-0/unviable.txt"
run_test "incremental, carry forward 8 + catch 2 = 10" "$workdir" "100.0" 10 10

# -------------------------------------------------------------------
# Test 4: Incremental — previously killed mutant now missed
# -------------------------------------------------------------------
workdir=$(setup_workdir)
make_mutants "$workdir/baseline/current_mutants.txt" 10
cp "$workdir/baseline/current_mutants.txt" "$workdir/previous-killed/killed_mutants.txt"
# This run retests all 10 but misses 1
head -9 "$workdir/baseline/current_mutants.txt" > "$workdir/results/mutants-results-0/caught.txt"
tail -1 "$workdir/baseline/current_mutants.txt" > "$workdir/results/mutants-results-0/missed.txt"
: > "$workdir/results/mutants-results-0/timeout.txt"
: > "$workdir/results/mutants-results-0/unviable.txt"
run_test "incremental, retest all, 1 now missed = 9/10" "$workdir" "90.0" 9 10

# -------------------------------------------------------------------
# Test 5: Unviable and timeout count as killed
# -------------------------------------------------------------------
workdir=$(setup_workdir)
make_mutants "$workdir/baseline/current_mutants.txt" 10
rm -f "$workdir/previous-killed/killed_mutants.txt"
head -5 "$workdir/baseline/current_mutants.txt" > "$workdir/results/mutants-results-0/caught.txt"
: > "$workdir/results/mutants-results-0/missed.txt"
sed -n '6,8p' "$workdir/baseline/current_mutants.txt" > "$workdir/results/mutants-results-0/timeout.txt"
sed -n '9,10p' "$workdir/baseline/current_mutants.txt" > "$workdir/results/mutants-results-0/unviable.txt"
run_test "caught + timeout + unviable all count as killed" "$workdir" "100.0" 10 10

# -------------------------------------------------------------------
# Test 6: Empty total — edge case
# -------------------------------------------------------------------
workdir=$(setup_workdir)
: > "$workdir/baseline/current_mutants.txt"
: > "$workdir/results/mutants-results-0/caught.txt"
: > "$workdir/results/mutants-results-0/missed.txt"
: > "$workdir/results/mutants-results-0/timeout.txt"
: > "$workdir/results/mutants-results-0/unviable.txt"
rm -f "$workdir/previous-killed/killed_mutants.txt"
run_test "empty total = 100%" "$workdir" "100.0" 0 0

# -------------------------------------------------------------------
# Test 7: LINE SHIFT — mutants change line numbers between runs
# This is the bug that caused 96.6% in CI.
# Previous run killed all 10 at lines 10,20,...,100
# New code shifted them to lines 50,60,...,140
# Only 2 were retested (caught). The other 8 should carry forward.
# -------------------------------------------------------------------
workdir=$(setup_workdir)
# Current baseline has mutants at shifted lines (50,60,...,140)
make_mutants "$workdir/baseline/current_mutants.txt" 10 50
# Previous killed list has mutants at old lines (10,20,...,100)
make_mutants "$workdir/previous-killed/killed_mutants.txt" 10 10
# Retested 2 mutants at new lines, caught both
head -2 "$workdir/baseline/current_mutants.txt" > "$workdir/results/mutants-results-0/caught.txt"
: > "$workdir/results/mutants-results-0/missed.txt"
: > "$workdir/results/mutants-results-0/timeout.txt"
: > "$workdir/results/mutants-results-0/unviable.txt"
run_test "LINE SHIFT: 8 carry forward + 2 caught = 10" "$workdir" "100.0" 10 10

# -------------------------------------------------------------------
# Test 8: LINE SHIFT — partial overlap
# Old: 10 mutants (fn_1..fn_10) at lines 10..100
# New: fn_1..fn_8 shifted to lines 50..120, fn_9 removed, fn_11 added
# Previously killed all 10. Retest fn_11 (caught).
# Should carry forward fn_1..fn_8, kill fn_11 = 9/10
# fn_9 removed so not in total anymore, fn_10 unchanged.
# -------------------------------------------------------------------
workdir=$(setup_workdir)
# Current: fn_1..fn_8 at 50..120, fn_10 at 100, fn_11 at 110
{
  for i in $(seq 1 8); do
    echo "src/file.rs:$((50 + (i-1)*10)):5: replace fn_${i} -> bool with false"
  done
  echo "src/file.rs:100:5: replace fn_10 -> bool with false"
  echo "src/file.rs:110:5: replace fn_11 -> bool with false"
} | LC_ALL=C sort > "$workdir/baseline/current_mutants.txt"

# Previous killed: fn_1..fn_10 at old lines 10..100
make_mutants "$workdir/previous-killed/killed_mutants.txt" 10 10
# Only test fn_11, catch it
grep "fn_11" "$workdir/baseline/current_mutants.txt" > "$workdir/results/mutants-results-0/caught.txt"
: > "$workdir/results/mutants-results-0/missed.txt"
: > "$workdir/results/mutants-results-0/timeout.txt"
: > "$workdir/results/mutants-results-0/unviable.txt"
run_test "LINE SHIFT: shifted+removed+added, carry forward works" "$workdir" "100.0" 10 10

# -------------------------------------------------------------------
# Test 9: Multiple shards
# -------------------------------------------------------------------
workdir=$(setup_workdir)
make_mutants "$workdir/baseline/current_mutants.txt" 10
rm -f "$workdir/previous-killed/killed_mutants.txt"
mkdir -p "$workdir/results/mutants-results-1"
head -5 "$workdir/baseline/current_mutants.txt" > "$workdir/results/mutants-results-0/caught.txt"
: > "$workdir/results/mutants-results-0/missed.txt"
: > "$workdir/results/mutants-results-0/timeout.txt"
: > "$workdir/results/mutants-results-0/unviable.txt"
tail -5 "$workdir/baseline/current_mutants.txt" > "$workdir/results/mutants-results-1/caught.txt"
: > "$workdir/results/mutants-results-1/missed.txt"
: > "$workdir/results/mutants-results-1/timeout.txt"
: > "$workdir/results/mutants-results-1/unviable.txt"
run_test "multiple shards, all caught" "$workdir" "100.0" 10 10

# -------------------------------------------------------------------
# Test 10: Color thresholds
# -------------------------------------------------------------------
workdir=$(setup_workdir)
make_mutants "$workdir/baseline/current_mutants.txt" 10
rm -f "$workdir/previous-killed/killed_mutants.txt"
head -7 "$workdir/baseline/current_mutants.txt" > "$workdir/results/mutants-results-0/caught.txt"
tail -3 "$workdir/baseline/current_mutants.txt" > "$workdir/results/mutants-results-0/missed.txt"
: > "$workdir/results/mutants-results-0/timeout.txt"
: > "$workdir/results/mutants-results-0/unviable.txt"
output_file="$workdir/github_output.txt"
touch "$output_file"
(cd "$workdir" && GITHUB_OUTPUT="$output_file" LC_ALL=C bash "$CALC_SCRIPT" > /dev/null 2>&1)
actual_color=$(grep '^color=' "$output_file" | cut -d= -f2)
if [ "$actual_color" = "red" ]; then
  PASS=$((PASS + 1))
  echo "  PASS: color=red for 70%"
else
  FAIL=$((FAIL + 1))
  msg="\n  FAIL: color threshold — expected=red actual=$actual_color for 70%"
  ERRORS="${ERRORS}${msg}"
  echo "  FAIL: color threshold$msg"
fi
rm -rf "$workdir"

# -------------------------------------------------------------------
# Test 11: Replay real CI data — run 22765707587 (the buggy run)
# Uses actual fixture data from CI.
# -------------------------------------------------------------------
if [ -d "$SCRIPT_DIR/fixtures/22765707587" ]; then
  workdir=$(setup_workdir)
  cp "$SCRIPT_DIR/fixtures/22765707587/baseline/current_mutants.txt" "$workdir/baseline/"
  cp "$SCRIPT_DIR/fixtures/22765707587/results/mutants-results-0/"* "$workdir/results/mutants-results-0/"
  cp "$SCRIPT_DIR/fixtures/22765707587/previous-killed/killed_mutants.txt" "$workdir/previous-killed/"
  run_test "REPLAY run-22765707587: line shift with real data" "$workdir" "100.0" 232 232
fi

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
if [ -n "$ERRORS" ]; then
  echo ""
  echo "Failures:$ERRORS"
fi

[ "$FAIL" -eq 0 ]
