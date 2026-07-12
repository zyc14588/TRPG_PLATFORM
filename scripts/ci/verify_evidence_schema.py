#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

from repo_truth import ROOT, validate_evidence


def main() -> int:
    schema = json.loads((ROOT / "scripts/ci/evidence.schema.json").read_text(encoding="utf-8"))
    if not schema.get("required") or not schema.get("statuses"):
        print("invalid evidence schema", file=sys.stderr)
        return 1
    errors = []
    for name in sys.argv[1:]:
        try:
            errors.extend(validate_evidence(json.loads(Path(name).read_text(encoding="utf-8"))))
        except (OSError, json.JSONDecodeError) as error:
            errors.append(str(error))
    legacy = ROOT / "evidence/stages/S09/docker-compose-smoke.txt"
    if legacy.exists():
        try:
            legacy_data = json.loads(legacy.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            legacy_data = {}
        if not validate_evidence(legacy_data):
            errors.append("historical S09 evidence was accepted")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("evidence schema valid; historical PASS evidence rejected")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
