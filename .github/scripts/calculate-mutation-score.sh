#!/usr/bin/env bash
set -euo pipefail

# Calculates mutation score from shard results: killed / total.
#
# Inputs:
#   baseline/current_mutants.txt  - full mutant list for this commit
#   results/**/caught.txt         - caught mutants from shards
#   results/**/timeout.txt        - timed-out mutants from shards
#   results/**/unviable.txt       - unviable mutants from shards
#
# Outputs (GITHUB_OUTPUT):
#   score - percentage string (e.g. "100.0")
#   color - badge color

OUTPUT="${GITHUB_OUTPUT:-/dev/stdout}"

find results -name caught.txt -o -name timeout.txt -o -name unviable.txt \
  | xargs cat 2>/dev/null | LC_ALL=C sort -u > killed_mutants.txt

total=$(wc -l < baseline/current_mutants.txt | tr -d ' ')
killed=$(wc -l < killed_mutants.txt | tr -d ' ')
if [ "$total" -gt 0 ]; then
  SCORE=$(awk "BEGIN {printf \"%.1f\", $killed * 100.0 / $total}")
else
  SCORE="100.0"
fi

echo "score=${SCORE}" >> "$OUTPUT"
echo "Mutation score: ${SCORE}% (killed=$killed, total=$total)"

if   awk "BEGIN {exit !($SCORE >= 100)}"; then echo "color=brightgreen" >> "$OUTPUT"
elif awk "BEGIN {exit !($SCORE >= 90)}";  then echo "color=green" >> "$OUTPUT"
elif awk "BEGIN {exit !($SCORE >= 80)}";  then echo "color=yellowgreen" >> "$OUTPUT"
else echo "color=red" >> "$OUTPUT"; fi
