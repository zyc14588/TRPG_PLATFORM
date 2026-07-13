#!/usr/bin/env python3
from __future__ import annotations

import json
import sys

from verify_test_inventory import inventory


def main() -> int:
    if sys.argv[1:] != ["--check"]:
        print("usage: discover_tests.py --check", file=sys.stderr)
        return 2
    report, errors = inventory()
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    counts = {name: len(value) for name, value in report.items() if hasattr(value, "__len__")}
    print(json.dumps(counts, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
