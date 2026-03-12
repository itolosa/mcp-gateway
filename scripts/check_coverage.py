#!/usr/bin/env python3
"""Check 100% line and function coverage from LCOV file.

Uses DA entries (source-line level) and FNF/FNH (function level) to avoid
phantom misses from generic monomorphization in llvm-cov's LF/LH counters.
"""
import sys

def main():
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <lcov.info>", file=sys.stderr)
        sys.exit(2)

    da_total = da_zero = fnf = fnh = 0
    current_file = None
    uncovered = []
    for line in open(sys.argv[1]):
        line = line.strip()
        if line.startswith("SF:"):
            current_file = line[3:]
        elif line.startswith("DA:"):
            da_total += 1
            parts = line[3:].split(",")
            if int(parts[1]) == 0:
                da_zero += 1
                uncovered.append(f"  {current_file}:{parts[0]}")
        elif line.startswith("FNF:"):
            fnf += int(line.split(":")[1])
        elif line.startswith("FNH:"):
            fnh += int(line.split(":")[1])

    ok = True
    if da_zero:
        print(f"FAIL: {da_zero}/{da_total} lines uncovered")
        for entry in uncovered:
            print(entry)
        ok = False
    else:
        print(f"OK: 100% line coverage ({da_total} lines)")

    if fnf != fnh:
        print(f"FAIL: {fnh}/{fnf} functions covered")
        ok = False
    else:
        print(f"OK: 100% function coverage ({fnf} functions)")

    sys.exit(0 if ok else 1)

if __name__ == "__main__":
    main()
