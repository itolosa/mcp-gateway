#!/usr/bin/env bash
set -euo pipefail

echo "DEBUG: script started, pwd=$(pwd)"
ls -la results/ baseline/ previous-killed/ 2>&1 || true
ls results/mutants-results-*/caught.txt 2>&1 || true

# Calculates the cumulative mutation score by combining:
# - Previously killed mutants that were NOT retested this run (carry-forward)
# - Mutants killed in this run
#
# Inputs (files in working directory):
#   baseline/current_mutants.txt           - sorted full mutant list for this commit
#   results/mutants-results-*/caught.txt   - caught mutants from shard results
#   results/mutants-results-*/timeout.txt  - timed-out mutants from shard results
#   results/mutants-results-*/unviable.txt - unviable mutants from shard results
#   results/mutants-results-*/missed.txt   - missed mutants from shard results
#   previous-killed/killed_mutants.txt     - killed list from previous run (optional)
#
# Outputs:
#   killed_mutants.txt  - cumulative killed list (written to working directory)
#   GITHUB_OUTPUT       - score, color

OUTPUT="${GITHUB_OUTPUT:-/dev/stdout}"

# Strip line:col from mutant strings so shifted lines still match.
# "src/file.rs:42:5: replace foo" -> "src/file.rs: replace foo"
normalize() { sed 's/^\([^:]*\):[0-9][0-9]*:[0-9][0-9]*:/\1:/'; }

# Collect killed (caught + timeout + unviable) and tested from shard results
cat results/mutants-results-*/caught.txt results/mutants-results-*/timeout.txt results/mutants-results-*/unviable.txt 2>/dev/null | LC_ALL=C sort -u > killed_this_run.txt
cat results/mutants-results-*/caught.txt results/mutants-results-*/timeout.txt results/mutants-results-*/unviable.txt results/mutants-results-*/missed.txt 2>/dev/null | LC_ALL=C sort -u > tested_this_run.txt

# Apply formula: killed = (prev_killed ∩ (current - tested)) ∪ killed_this_run
# Use normalized signatures (line numbers stripped) so mutants that shifted lines
# between commits are still recognized as the same mutant.
if [ -f previous-killed/killed_mutants.txt ]; then
  # Build a lookup table: normalized_signature TAB original_string for current mutants
  # This keeps pairs aligned regardless of sort order differences.
  paste <(normalize < baseline/current_mutants.txt) baseline/current_mutants.txt | \
    LC_ALL=C sort -t$'\t' -k1,1 > current_paired.txt

  normalize < tested_this_run.txt | LC_ALL=C sort -u > tested_norm.txt
  normalize < previous-killed/killed_mutants.txt | LC_ALL=C sort -u > prev_killed_norm.txt

  # Extract normalized signatures of current mutants
  cut -f1 current_paired.txt | LC_ALL=C sort -u > current_norm.txt

  # Untested normalized signatures: current minus tested
  comm -23 current_norm.txt tested_norm.txt > untested_norm.txt

  # Previously killed that are still present and untested
  comm -12 prev_killed_norm.txt untested_norm.txt > surviving_norm.txt

  # Map surviving signatures back to current mutant strings via the paired lookup
  if [ -s surviving_norm.txt ]; then
    LC_ALL=C join -t$'\t' -1 1 -2 1 current_paired.txt surviving_norm.txt | \
      cut -f2 | LC_ALL=C sort -u > surviving_prev.txt
  else
    : > surviving_prev.txt
  fi

  LC_ALL=C sort -u surviving_prev.txt killed_this_run.txt > killed_mutants.txt
else
  cp killed_this_run.txt killed_mutants.txt
fi

total=$(wc -l < baseline/current_mutants.txt | tr -d ' ')
killed=$(wc -l < killed_mutants.txt | tr -d ' ')
if [ "$total" -gt 0 ]; then
  SCORE=$(awk "BEGIN {printf \"%.1f\", $killed * 100.0 / $total}")
else
  SCORE="100.0"
fi
echo "score=${SCORE}" >> "$OUTPUT"
echo "Mutation score: ${SCORE}% (killed=$killed, total=$total)"
if awk "BEGIN {exit !($SCORE >= 100)}"; then echo "color=brightgreen" >> "$OUTPUT"
elif awk "BEGIN {exit !($SCORE >= 90)}"; then echo "color=green" >> "$OUTPUT"
elif awk "BEGIN {exit !($SCORE >= 80)}"; then echo "color=yellowgreen" >> "$OUTPUT"
else echo "color=red" >> "$OUTPUT"; fi
