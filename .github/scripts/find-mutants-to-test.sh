#!/usr/bin/env bash
set -euo pipefail

# Determines which mutants need testing by combining:
# 1. New mutants (from mutant list diff)
# 2. Mutants affected by test/source changes (from per-test coverage map)
# 3. Unproven mutants (not in previous killed list)
#
# Inputs (files in working directory):
#   current_mutants.txt        - sorted list from current cargo mutants --list
#   previous_mutants.txt       - sorted list from last successful run (optional)
#   previous-coverage-map.json - per-test coverage map from last run (optional)
#   previous_killed_mutants.txt - killed mutants from last run (optional)
#
# Environment:
#   COVERED_SHA - commit SHA of last successful mutation run (optional)
#
# Outputs to GITHUB_OUTPUT:
#   mode    - "full", "incremental", or "skip"
#   pattern - regex pattern for --re (only when mode=incremental)

OUTPUT="${GITHUB_OUTPUT:-/dev/stdout}"

# If no previous baseline, run full
if [ ! -f previous_mutants.txt ]; then
  echo "No previous mutant baseline found, running full"
  echo "mode=full" >> "$OUTPUT"
  exit 0
fi

# If baseline exists but killed list is missing, run full
# (incremental mode needs the killed list to carry forward untested results)
if [ ! -f previous_killed_mutants.txt ]; then
  echo "No previous killed list found, running full"
  echo "mode=full" >> "$OUTPUT"
  exit 0
fi

# Strip line:col from mutant strings so shifted lines still match.
normalize() { sed 's/^\([^:]*\):[0-9][0-9]*:[0-9][0-9]*:/\1:/'; }

# Build paired lookup: normalized_signature TAB original_string (for current mutants)
paste <(normalize < current_mutants.txt) current_mutants.txt | \
  LC_ALL=C sort -t$'\t' -k1,1 > current_paired.txt
cut -f1 current_paired.txt | LC_ALL=C sort -u > current_norm.txt

# 1. Find new mutants (normalized: in current but not in previous)
normalize < previous_mutants.txt | LC_ALL=C sort -u > previous_norm.txt
comm -23 current_norm.txt previous_norm.txt > new_norm.txt || true
if [ -s new_norm.txt ]; then
  LC_ALL=C join -t$'\t' -1 1 -2 1 current_paired.txt new_norm.txt | \
    cut -f2 | LC_ALL=C sort -u > new_mutants.txt
else
  : > new_mutants.txt
fi

# 2. Find mutants affected by file changes (using per-test coverage map)
touch affected_mutants.txt
if [ -n "${COVERED_SHA:-}" ] && [ -f previous-coverage-map.json ]; then
  CHANGED_FILES=$(git diff "${COVERED_SHA}...HEAD" --name-only -- 'src/' 'tests/' 2>/dev/null || echo "")

  if [ -n "$CHANGED_FILES" ]; then
    : > affected_lines.txt

    # Check if support files changed (affects all integration tests)
    SUPPORT_CHANGED=false
    if echo "$CHANGED_FILES" | grep -q "^tests/support/"; then
      SUPPORT_CHANGED=true
    fi

    if [ "$SUPPORT_CHANGED" = true ]; then
      # Collect coverage from ALL integration tests
      jq -r 'to_entries[] | select(.value.source_file | startswith("tests/")) | .value.covered_lines[]' \
        previous-coverage-map.json >> affected_lines.txt 2>/dev/null || true
    fi

    # For each changed file, find tests whose source_file matches
    for changed_file in $CHANGED_FILES; do
      [ -z "$changed_file" ] && continue
      echo "$changed_file" | grep -q "^tests/support/" && continue

      jq -r --arg sf "$changed_file" '
        to_entries[] | select(.value.source_file == $sf) | .value.covered_lines[]
      ' previous-coverage-map.json >> affected_lines.txt 2>/dev/null || true
    done

    # Match covered lines to mutants
    if [ -s affected_lines.txt ]; then
      sort -u affected_lines.txt | while IFS= read -r line; do
        [ -z "$line" ] && continue
        awk -v prefix="$line:" 'substr($0, 1, length(prefix)) == prefix' current_mutants.txt || true
      done | sort -u >> affected_mutants.txt
    fi
  fi
fi

# 3. Combine new + affected mutants
cat new_mutants.txt affected_mutants.txt 2>/dev/null | sort -u > target_mutants.txt

# 4. Add unproven mutants (normalized: not in previous killed list)
if [ -f previous_killed_mutants.txt ]; then
  normalize < previous_killed_mutants.txt | LC_ALL=C sort -u > prev_killed_norm.txt
  comm -23 current_norm.txt prev_killed_norm.txt > unproven_norm.txt
  if [ -s unproven_norm.txt ]; then
    LC_ALL=C join -t$'\t' -1 1 -2 1 current_paired.txt unproven_norm.txt | \
      cut -f2 | LC_ALL=C sort -u >> target_mutants.txt
    sort -u -o target_mutants.txt target_mutants.txt
  fi
fi

if [ ! -s target_mutants.txt ]; then
  echo "No new or affected mutants, skipping"
  echo "mode=skip" >> "$OUTPUT"
  exit 0
fi

COUNT=$(wc -l < target_mutants.txt | tr -d ' ')
NEW_COUNT=$(wc -l < new_mutants.txt | tr -d ' ')
AFFECTED_COUNT=$(wc -l < affected_mutants.txt | tr -d ' ')
echo "Found ${COUNT} mutants to test (${NEW_COUNT} new, ${AFFECTED_COUNT} from test changes)"

PATTERN=$(sed 's/[][\\.^$*+?(){}|]/\\&/g' target_mutants.txt | paste -s -d'|' -)
echo "mode=incremental" >> "$OUTPUT"
echo "pattern=${PATTERN}" >> "$OUTPUT"
