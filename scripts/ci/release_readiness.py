#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import subprocess
from pathlib import Path

from repo_truth import (
    EVIDENCE_GENERATOR_VERSION,
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
RELEASE_COMMAND = ["bash", "scripts/ci/test-all.sh"]
REQUIRED_AUDIT_BLOCKERS = {
    "AUD-001": "V1 product runtime binary is not implemented",
    "AUD-002": "V1 product container image is not implemented",
    "AUD-006": "V1 Compose services remain explicit placeholders",
}


def release_evidence_errors(data: dict, root: Path, artifact_base: Path) -> list[str]:
    errors = validate_evidence(data, root, artifact_base)
    if data.get("status") != "PASS" or data.get("exit_code") != 0:
        errors.append("release evidence must record a passing command")
    if data.get("generator_version") != EVIDENCE_GENERATOR_VERSION:
        errors.append("release evidence generator_version mismatch")
    if data.get("command_argv") != RELEASE_COMMAND:
        errors.append("release evidence must execute bash scripts/ci/test-all.sh")
    artifacts = data.get("artifact_sha256")
    if not isinstance(artifacts, dict) or "MANIFEST.md" not in artifacts:
        errors.append("release evidence must bind MANIFEST.md")
    return errors


def assess(root: Path, evidence: Path | None = None) -> dict:
    blockers: list[dict[str, str]] = [
        {"id": audit_id, "reason": reason}
        for audit_id, reason in REQUIRED_AUDIT_BLOCKERS.items()
    ]
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
            resolved_evidence = evidence.resolve()
            if resolved_evidence.is_relative_to(root.resolve()):
                errors = ["release evidence must be outside the repository"]
            else:
                errors = release_evidence_errors(
                    json.loads(resolved_evidence.read_text(encoding="utf-8")),
                    root,
                    resolved_evidence.parent,
                )
        except (OSError, json.JSONDecodeError) as error:
            errors = [str(error)]
        blockers.extend({"id": "INVALID_CURRENT_EVIDENCE", "reason": error} for error in errors)

    status = subprocess.run(
        ["git", "status", "--porcelain=v1"], cwd=root, check=True, text=True, capture_output=True
    ).stdout.strip()
    if status:
        blockers.append({"id": "DIRTY_WORKTREE", "reason": "release candidates require a clean worktree"})
    whitespace = subprocess.run(
        ["git", "diff", "--check"], cwd=root, check=False, text=True, capture_output=True
    )
    if whitespace.returncode:
        blockers.append({"id": "WHITESPACE_ERROR", "reason": whitespace.stdout.strip()})

    return {
        "status": "BLOCKED" if blockers else "READY",
        "base_commit": base_commit(root),
        "worktree_diff_sha256": worktree_diff_sha256(root),
        "generator_version": EVIDENCE_GENERATOR_VERSION,
        "blockers": blockers,
    }


def readiness_report_errors(data: dict, root: Path = ROOT) -> list[str]:
    errors = []
    if data.get("status") != "BLOCKED":
        errors.append("release readiness must remain BLOCKED")
    if data.get("base_commit") != base_commit(root):
        errors.append("release readiness base_commit mismatch")
    if os.environ.get("GITHUB_SHA", data.get("base_commit")) != data.get("base_commit"):
        errors.append("release readiness provenance does not match GITHUB_SHA")
    if data.get("worktree_diff_sha256") != worktree_diff_sha256(root):
        errors.append("release readiness worktree provenance mismatch")
    if data.get("generator_version") != EVIDENCE_GENERATOR_VERSION:
        errors.append("release readiness generator_version mismatch")
    blockers = data.get("blockers")
    if not isinstance(blockers, list) or not all(isinstance(item, dict) for item in blockers):
        errors.append("release readiness blockers must be an object array")
    else:
        blocker_ids = {item.get("id") for item in blockers}
        for audit_id in REQUIRED_AUDIT_BLOCKERS:
            if audit_id not in blocker_ids:
                errors.append(f"release readiness missing blocker: {audit_id}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path)
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--require-ready", action="store_true")
    parser.add_argument("--require-blocked", action="store_true")
    parser.add_argument("--verify-report", type=Path)
    args = parser.parse_args()
    if args.require_ready and args.require_blocked:
        parser.error("--require-ready and --require-blocked are mutually exclusive")
    if args.verify_report:
        if any((args.report, args.evidence, args.require_ready, args.require_blocked)):
            parser.error("--verify-report cannot be combined with assessment options")
        try:
            errors = readiness_report_errors(json.loads(args.verify_report.read_text(encoding="utf-8")))
        except (OSError, json.JSONDecodeError) as error:
            errors = [str(error)]
        if errors:
            print("\n".join(errors))
            return 1
        print("release readiness report verified: BLOCKED")
        return 0
    if args.report:
        args.report = args.report.resolve()
        if args.report.is_relative_to(ROOT.resolve()):
            parser.error("--report must be outside the repository")
    report = assess(ROOT, args.evidence)
    payload = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(payload, encoding="utf-8")
    print(payload, end="")
    if args.require_blocked:
        errors = readiness_report_errors(report)
        if errors:
            print("\n".join(errors))
            return 1
    return 1 if args.require_ready and report["status"] != "READY" else 0


if __name__ == "__main__":
    raise SystemExit(main())
