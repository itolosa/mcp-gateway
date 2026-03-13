#!/usr/bin/env python3
"""Check 100% line and function coverage from LCOV file.

Uses DA entries (source-line level) for line coverage.  For function coverage,
deduplicates by (file, line_number) and takes the max hit count across all
monomorphizations.  A function is considered covered if either its FNDA max
is > 0 OR the DA entry for its line is > 0 (handles async sub-closures whose
FNDA counts are zero in phantom monomorphizations but whose lines are executed).
"""
import sys
from collections import defaultdict

def main():
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <lcov.info>", file=sys.stderr)
        sys.exit(2)

    da_total = da_zero = 0
    current_file = None
    uncovered = []
    fn_to_line = {}
    fn_coverage = defaultdict(int)  # (file, line) -> max hit count
    da_coverage = defaultdict(int)  # (file, line) -> max hit count

    for line in open(sys.argv[1]):
        line = line.strip()
        if line.startswith("SF:"):
            current_file = line[3:]
            fn_to_line = {}
        elif line.startswith("DA:"):
            da_total += 1
            parts = line[3:].split(",")
            lineno = int(parts[0])
            count = int(parts[1])
            if count == 0:
                da_zero += 1
                uncovered.append(f"  {current_file}:{parts[0]}")
            da_coverage[(current_file, lineno)] = max(
                da_coverage[(current_file, lineno)], count
            )
        elif line.startswith("FN:"):
            parts = line[3:].split(",", 1)
            fn_to_line[parts[1]] = int(parts[0])
        elif line.startswith("FNDA:"):
            parts = line[5:].split(",", 1)
            count = int(parts[0])
            name = parts[1]
            if name in fn_to_line:
                key = (current_file, fn_to_line[name])
                fn_coverage[key] = max(fn_coverage[key], count)

    ok = True
    if da_zero:
        print(f"FAIL: {da_zero}/{da_total} lines uncovered")
        for entry in uncovered:
            print(entry)
        ok = False
    else:
        print(f"OK: 100% line coverage ({da_total} lines)")

    fn_total = len(fn_coverage)
    fn_uncovered = []
    for key, count in sorted(fn_coverage.items()):
        if count == 0 and da_coverage.get(key, 0) == 0:
            fn_uncovered.append(key)
    fn_hit = fn_total - len(fn_uncovered)
    if fn_uncovered:
        print(f"FAIL: {fn_hit}/{fn_total} functions covered")
        for f, lineno in fn_uncovered:
            print(f"  {f}:{lineno}")
        ok = False
    else:
        print(f"OK: 100% function coverage ({fn_total} functions)")

    sys.exit(0 if ok else 1)

if __name__ == "__main__":
    main()
