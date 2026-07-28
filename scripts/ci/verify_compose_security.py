#!/usr/bin/env python3
from __future__ import annotations

import sys

from compose_security_rules import errors
from compose_security_support import ROOT


def main() -> int:
    if sys.argv[1:] != ["--check"]:
        print("usage: verify_compose_security.py --check", file=sys.stderr)
        return 2
    found = errors()
    if found:
        print("\n".join(found), file=sys.stderr)
        return 1
    print("production Compose security contract verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
