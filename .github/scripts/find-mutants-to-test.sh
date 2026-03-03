#!/usr/bin/env bash
set -euo pipefail

# Determines which mutants need testing by combining:
# 1. New mutants (from mutant list diff)
# 2. Mutants affected by test file changes (from coverage map)
#
# Inputs (files in working directory):
#   current_mutants.txt   - sorted list from current cargo mutants --list
#   previous_mutants.txt  - sorted list from last successful run (optional)
#   previous-coverage-map.json - per-integration-test coverage map from last run (optional)
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

# 1. Find new mutants (in current but not in previous)
comm -23 current_mutants.txt previous_mutants.txt > new_mutants.txt || true

# 2. Find mutants affected by test file changes
touch affected_mutants.txt
if [ -f previous-coverage-map.json ] && [ -n "${COVERED_SHA:-}" ]; then
  CHANGED_TEST_FILES=$(git diff "${COVERED_SHA}...HEAD" --name-only -- 'tests/' 2>/dev/null || echo "")

  if [ -n "$CHANGED_TEST_FILES" ]; then
    SUPPORT_CHANGED=false
    if echo "$CHANGED_TEST_FILES" | grep -q "tests/support/"; then
      SUPPORT_CHANGED=true
    fi

    # Collect covered source lines from changed test files
    for test_file in $CHANGED_TEST_FILES; do
      # If support file changed, all integration tests are affected
      if [ "$SUPPORT_CHANGED" = true ]; then
        jq -r '.[][]' previous-coverage-map.json >> affected_lines.txt 2>/dev/null || true
        break
      fi
      jq -r --arg f "$test_file" '.[$f][]? // empty' previous-coverage-map.json >> affected_lines.txt 2>/dev/null || true
    done

    # Match covered lines to mutants
    if [ -f affected_lines.txt ] && [ -s affected_lines.txt ]; then
      sort -u affected_lines.txt | while IFS= read -r line; do
        [ -z "$line" ] && continue
        escaped=$(echo "$line" | sed 's/[][\\.^$*+?(){}|]/\\&/g')
        grep "^${escaped}:" current_mutants.txt || true
      done | sort -u > affected_mutants.txt
    fi
  fi
fi

# 3. Combine new + affected mutants
cat new_mutants.txt affected_mutants.txt 2>/dev/null | sort -u > target_mutants.txt

if [ ! -s target_mutants.txt ]; then
  echo "No new or affected mutants, skipping"
  echo "mode=skip" >> "$OUTPUT"
  exit 0
fi

COUNT=$(wc -l < target_mutants.txt | tr -d ' ')
NEW_COUNT=$(wc -l < new_mutants.txt | tr -d ' ')
AFFECTED_COUNT=$(wc -l < affected_mutants.txt | tr -d ' ')
echo "Found ${COUNT} mutants to test (${NEW_COUNT} new, ${AFFECTED_COUNT} from test changes)"

PATTERN=$(sed 's/[][\\.^$*+?(){}|]/\\&/g' target_mutants.txt | paste -sd'|')
echo "mode=incremental" >> "$OUTPUT"
echo "pattern=${PATTERN}" >> "$OUTPUT"
