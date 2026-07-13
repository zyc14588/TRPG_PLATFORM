#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from repo_truth import ROOT, cargo_targets, git_files, git_modes
from validate_workflows import validate as validate_workflows


def inventory(root: Path = ROOT) -> tuple[dict, list[str]]:
    files = git_files(root)
    fixtures = [path for path in files if path.startswith("fixtures/") and path != "fixtures/README.md"]
    reference_text = []
    reference_roots = ("crates/", "scripts/", "stages/", "policy/", ".github/")
    for name in files:
        if name in fixtures or not (name.startswith(reference_roots) or "/" not in name):
            continue
        path = root / name
        if path.is_file() and path.stat().st_size < 1_000_000:
            try:
                reference_text.append(path.read_text(encoding="utf-8"))
            except UnicodeDecodeError:
                pass
    corpus = "\n".join(reference_text)
    orphans = [name for name in fixtures if name not in corpus and Path(name).name not in corpus]
    allowlist_path = root / "scripts/ci/fixture_allowlist.json"
    allowed = {}
    errors = validate_workflows(root)
    if allowlist_path.is_file():
        entries = json.loads(allowlist_path.read_text(encoding="utf-8"))
        for entry in entries:
            if not all(entry.get(field) for field in ("path", "reason", "owner")):
                errors.append("fixture allowlist entries require path, reason, and owner")
            else:
                allowed[entry["path"]] = entry
    errors.extend(f"orphan fixture: {name}" for name in orphans if name not in allowed)
    errors.extend(f"stale fixture allowlist entry: {name}" for name in allowed if name not in orphans)
    package = json.loads((root / "package.json").read_text(encoding="utf-8"))
    report = {
        "rust_test_targets": sorted(cargo_targets(root, "test")),
        "node_scripts": package.get("scripts", {}),
        "opa_tests": sorted(path for path in files if path.startswith("policy/opa/") and path.endswith("_test.rego")),
        "powershell": sorted(path for path in files if path.endswith(".ps1")),
        "shell": sorted(path for path in files if path.endswith(".sh")),
        "fixtures": fixtures,
        "orphan_fixtures": orphans,
        "workflows": sorted(path for path in files if path.startswith(".github/workflows/") and path.endswith(".yml")),
    }
    for key in ("rust_test_targets", "node_scripts", "opa_tests", "powershell", "shell", "fixtures", "workflows"):
        if not report[key]:
            errors.append(f"empty test inventory category: {key}")
    workflow_text = "\n".join((root / path).read_text(encoding="utf-8") for path in report["workflows"])
    ci_text = workflow_text + "\n" + (root / "scripts/ci/test-all.sh").read_text(encoding="utf-8")
    for script in report["powershell"]:
        if script not in ci_text:
            errors.append(f"PowerShell script is not referenced by CI: {script}")
    modes = git_modes(root)
    for script in (
        "scripts/ci/init-smoke.sh",
        "scripts/ci/service-process-smoke.sh",
        "scripts/ci/test-all.sh",
        "scripts/backup_restore/smoke.sh",
        "scripts/projection_rebuild/verify.sh",
    ):
        if modes.get(script) != "100755":
            errors.append(f"CI shell script is not executable in Git: {script}")
    return report, errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    report, errors = inventory()
    payload = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(payload, encoding="utf-8")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(payload, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
