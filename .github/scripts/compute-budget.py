"""Render and enforce CPU measurements emitted by the split contract test suite."""

import re
import sys

LIMIT = 100_000_000
WARNING_THRESHOLD = 80_000_000
ROW = re.compile(r"COMPUTE_BUDGET ([a-z_]+) ([a-z0-9_-]+) ([0-9]+)")


def main():
    measurements = {}
    for line in sys.stdin:
        # With --nocapture, libtest can prefix the first line with the test name.
        if "COMPUTE_BUDGET " not in line:
            continue
        row = ROW.fullmatch(line[line.index("COMPUTE_BUDGET "):].strip())
        if row is None:
            sys.exit(f"Malformed compute budget measurement: {line.strip()}")
        operation, fixture, instructions = row.groups()
        key = (operation, fixture)
        if key in measurements:
            sys.exit(f"Duplicate compute budget measurement: {operation} {fixture}")
        measurements[key] = int(instructions)

    if not measurements:
        sys.exit("No compute budget measurements found")

    print("## Compute Budget Report\n")
    print(f"CPU instruction limit: {LIMIT:,}\n")
    print("| Operation | Fixture | CPU instructions | Status |")
    print("|---|---|---:|---|")
    over_limit = False
    for (operation, fixture), instructions in sorted(measurements.items()):
        if instructions > LIMIT:
            status = "Over limit"
            over_limit = True
        elif instructions > WARNING_THRESHOLD:
            status = "Above warning threshold"
        else:
            status = "Within budget"
        print(f"| `{operation}` | {fixture} | {instructions:,} | {status} |")
    return int(over_limit)


if __name__ == "__main__":
    sys.exit(main())
