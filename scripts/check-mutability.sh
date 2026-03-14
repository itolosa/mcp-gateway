#!/bin/bash
set -uo pipefail

# Accept file path as argument or read from stdin JSON.
if [ $# -ge 1 ]; then
    FILE="$1"
else
    INPUT=$(cat)
    FILE=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')
fi

if [ -z "$FILE" ] || [ ! -f "$FILE" ] || [[ "$FILE" != *.rs ]]; then
    exit 0
fi

TEST_LINE=$(grep -n '#\[cfg(test)\]' "$FILE" | head -1 | cut -d: -f1)

if [ -n "$TEST_LINE" ]; then
    MUTS=$(head -n "$((TEST_LINE - 1))" "$FILE" | grep -n 'let mut ' || true)
else
    MUTS=$(grep -n 'let mut ' "$FILE" || true)
fi

if [ -n "$MUTS" ]; then
    echo "MUTABILITY CHECK: The following production lines use let mut — try the hardest you can to convert to immutable code (iterators, collect, fold, struct update syntax). Only keep mut if structurally required (IO buffers, process handles, non_exhaustive external types, trait-forced &mut self):" >&2
    echo "$MUTS" >&2
    exit 2
fi
