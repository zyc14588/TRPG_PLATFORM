#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

from repo_truth import (
    PRODUCT_SERVICES,
    ROOT,
    base_commit,
    cargo_targets,
    compose_services,
    git_files,
    validate_evidence,
    worktree_diff_sha256,
)


REQUIRED_WORKFLOWS = (
    "ci.yml",
    "contracts.yml",
    "docker-compose-smoke.yml",
    "golden-scenarios.yml",
    "release.yml",
)
REQUIRED_SCRIPTS = (
    "scripts/ci/validate_workflows.py",
    "scripts/ci/verify_test_inventory.py",
    "scripts/ci/verify_manifest.py",
    "scripts/ci/verify_evidence_schema.py",
)


def assess(root: Path, evidence: Path | None = None) -> dict:
    blockers: list[dict[str, str]] = []
    if not cargo_targets(root, "bin"):
        blockers.append({"id": "NO_PRODUCT_BINARY", "reason": "Cargo has no product binary target"})

    dockerfiles = [path for path in git_files(root) if Path(path).name.startswith("Dockerfile")]
    if not dockerfiles:
        blockers.append({"id": "NO_PRODUCT_DOCKERFILE", "reason": "no product Dockerfile exists"})

    for compose_name in ("compose.yml", "docker-compose.ci.yml"):
        path = root / compose_name
        if not path.is_file():
            blockers.append({"id": "MISSING_COMPOSE", "reason": compose_name})
            continue
        services = compose_services(path)
        for service in PRODUCT_SERVICES:
            config = services.get(service)
            if config is None:
                blockers.append({"id": "MISSING_PRODUCT_SERVICE", "reason": f"{compose_name}:{service}"})
            elif config["placeholder"]:
                blockers.append({"id": "PLACEHOLDER_SERVICE", "reason": f"{compose_name}:{service}"})
            elif not config["build"] and "@sha256:" not in str(config["image"]):
                blockers.append({"id": "MUTABLE_PRODUCT_IMAGE", "reason": f"{compose_name}:{service}"})

    for name in REQUIRED_WORKFLOWS:
        if not (root / ".github" / "workflows" / name).is_file():
            blockers.append({"id": "MISSING_WORKFLOW", "reason": name})
    for name in REQUIRED_SCRIPTS:
        if not (root / name).is_file():
            blockers.append({"id": "MISSING_CI_SCRIPT", "reason": name})

    if evidence is None:
        blockers.append({"id": "MISSING_CURRENT_EVIDENCE", "reason": "no runtime evidence supplied"})
    else:
        try:
            errors = validate_evidence(json.loads(evidence.read_text(encoding="utf-8")), root)
        except (OSError, json.JSONDecodeError) as error:
            errors = [str(error)]
        blockers.extend({"id": "INVALID_CURRENT_EVIDENCE", "reason": error} for error in errors)

    status = subprocess.run(
        ["git", "status", "--porcelain=v1"], cwd=root, check=True, text=True, capture_output=True
    ).stdout.strip()
    if status:
        blockers.append({"id": "DIRTY_WORKTREE", "reason": "release candidates require a clean worktree"})

    return {
        "status": "BLOCKED" if blockers else "READY",
        "base_commit": base_commit(root),
        "worktree_diff_sha256": worktree_diff_sha256(root),
        "generator_version": "p00a-1",
        "blockers": blockers,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path)
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--require-ready", action="store_true")
    args = parser.parse_args()
    report = assess(ROOT, args.evidence)
    payload = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(payload, encoding="utf-8")
    print(payload, end="")
    return 1 if args.require_ready and report["status"] != "READY" else 0


if __name__ == "__main__":
    raise SystemExit(main())
