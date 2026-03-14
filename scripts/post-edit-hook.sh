#!/bin/bash
set -uo pipefail

# PostToolUse hook for Edit|Write — runs fmt, clippy, and mutability check.
# Reads hook JSON from stdin, delegates to each check.
# Exit 2 + stderr makes output visible to Claude.

INPUT=$(cat)
cd "$CLAUDE_PROJECT_DIR"

# 1. Format
FMT_OUT=$(cargo fmt --all 2>&1)
FMT_EXIT=$?

# 2. Lint — scoped to the edited file's crate target to avoid flooding context
CLIPPY_OUT=""
CLIPPY_EXIT=0

# 3. Mutability check
FILE=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')
MUT_OUT=""
MUT_EXIT=0
if [ -n "$FILE" ]; then
    MUT_OUT=$(bash scripts/check-mutability.sh "$FILE" 2>&1)
    MUT_EXIT=$?
fi

# Report failures via stderr + exit 2 so Claude sees them
ERRORS=""

if [ $FMT_EXIT -ne 0 ]; then
    ERRORS="${ERRORS}FORMAT ERROR:\n${FMT_OUT}\n\n"
fi

if [ $CLIPPY_EXIT -ne 0 ]; then
    ERRORS="${ERRORS}CLIPPY ERROR:\n${CLIPPY_OUT}\n\n"
fi

if [ $MUT_EXIT -ne 0 ] && [ -n "$MUT_OUT" ]; then
    ERRORS="${ERRORS}${MUT_OUT}\n"
fi

if [ -n "$ERRORS" ]; then
    echo -e "$ERRORS" >&2
    exit 2
fi
