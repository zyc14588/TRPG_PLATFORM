#!/usr/bin/env python3
from __future__ import annotations

import argparse
import sys
from pathlib import Path

from manifest import OUTPUTS, render
from repo_truth import ROOT


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path)
    args = parser.parse_args()
    expected = render()
    paths = (args.manifest,) if args.manifest else tuple(ROOT / name for name in OUTPUTS)
    errors = []
    for path in paths:
        if path is None or not path.is_file():
            errors.append(f"missing manifest: {path}")
        elif path.read_text(encoding="utf-8") != expected:
            errors.append(f"manifest drift: {path}")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    hashed = next(line.split(": ", 1)[1] for line in expected.splitlines() if line.startswith("Hashed files:"))
    print(f"manifest verified: {hashed} hashed files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
