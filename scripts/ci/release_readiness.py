#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import subprocess
import xml.etree.ElementTree as ET
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
    "scripts/ci/verify_compose_security.py",
)
RELEASE_COMMAND = ["bash", "scripts/ci/test-all.sh"]
PRODUCTION_SECURITY_COMMAND = ["bash", "scripts/ci/production-security-smoke.sh"]
REQUIRED_RELEASE_TEST_CASES = {
    "migration_upgrade_covers_empty_b24_repeat_drift_and_constraints",
    "postgres_streams_are_isolated_idempotent_atomic_and_restartable",
    "persisted_consent_controls_route_snapshot_and_audit_across_revocation",
    "data_deletion_persists_blocks_on_hold_and_verifies_every_real_surface",
    "deletion_cannot_complete_when_a_required_surface_is_missing",
    "filesystem_verification_does_not_misreport_io_failures_as_absence",
    "retained_security_and_privacy_history_rejects_bulk_removal",
    "remote_postgres_uses_verified_tls_and_rejects_an_untrusted_chain",
    "redis_rate_limit_is_shared_across_identity_instances",
    "custom_format_backup_restores_to_an_independent_database_and_detects_tampering",
}
REQUIRED_PRODUCT_BINARIES = {
    "api-server",
    "realtime-server",
    "agent-worker",
    "admin-server",
    "migration-runner",
}
REQUIRED_WEB_SCRIPTS = {"build", "dev", "preview"}


def release_junit_errors(suite: ET.Element) -> list[str]:
    cases = list(suite.iter("testcase"))
    passing_test_cases = {
        case.get("name", "")
        for case in cases
        if all(
            case.find(result) is None
            for result in ("failure", "error", "skipped")
        )
    }
    errors = [
        f"release evidence is missing required passing test: {name}"
        for name in sorted(REQUIRED_RELEASE_TEST_CASES - passing_test_cases)
    ]
    if any(case.find("skipped") is not None for case in cases):
        errors.append("release evidence must not contain ignored tests")
    return errors


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
    generated = data.get("command_artifact_sha256")
    junit_names = (
        [name for name in generated if name.endswith(".junit.xml")]
        if isinstance(generated, dict)
        else []
    )
    if len(junit_names) != 1:
        errors.append("release evidence must contain exactly one command JUnit report")
    else:
        try:
            suite = ET.parse(artifact_base / junit_names[0]).getroot()
        except (OSError, ET.ParseError):
            errors.append("release evidence JUnit report is unreadable")
        else:
            errors.extend(release_junit_errors(suite))
    return errors


def production_security_evidence_errors(
    data: dict, root: Path, artifact_base: Path
) -> list[str]:
    errors = validate_evidence(data, root, artifact_base)
    if data.get("status") != "PASS" or data.get("exit_code") != 0:
        errors.append("production security evidence must record a passing command")
    if data.get("command_argv") != PRODUCTION_SECURITY_COMMAND:
        errors.append(
            "production security evidence must execute "
            "bash scripts/ci/production-security-smoke.sh"
        )
    artifacts = data.get("artifact_sha256")
    if not isinstance(artifacts, dict) or "MANIFEST.md" not in artifacts:
        errors.append("production security evidence must bind MANIFEST.md")
    return errors


def assess(
    root: Path,
    evidence: Path | None = None,
    security_evidence: Path | None = None,
) -> dict:
    blockers: list[dict[str, str]] = []
    missing_binaries = REQUIRED_PRODUCT_BINARIES - cargo_targets(root, "bin")
    blockers.extend(
        {"id": "MISSING_PRODUCT_BINARY", "reason": binary}
        for binary in sorted(missing_binaries)
    )

    web_package_path = root / "apps/web/package.json"
    try:
        web_package = json.loads(web_package_path.read_text(encoding="utf-8"))
        web_scripts = set(web_package.get("scripts", {}))
    except (OSError, json.JSONDecodeError):
        web_scripts = set()
    for path in ("apps/web/index.html", "apps/web/src/app.js"):
        if not (root / path).is_file():
            blockers.append({"id": "MISSING_WEB_ENTRYPOINT", "reason": path})
    for script in sorted(REQUIRED_WEB_SCRIPTS - web_scripts):
        blockers.append({"id": "MISSING_WEB_SCRIPT", "reason": script})

    dockerfiles = [path for path in git_files(root) if Path(path).name.startswith("Dockerfile")]
    if not dockerfiles:
        blockers.append({"id": "NO_PRODUCT_DOCKERFILE", "reason": "no product Dockerfile exists"})

    merged_services: dict[str, dict[str, object]] = {}
    for compose_name in ("compose.yml", "docker-compose.ci.yml"):
        path = root / compose_name
        if not path.is_file():
            blockers.append({"id": "MISSING_COMPOSE", "reason": compose_name})
            continue
        merged_services.update(compose_services(path))
    for service in PRODUCT_SERVICES:
        config = merged_services.get(service)
        if config is None:
            blockers.append({"id": "MISSING_PRODUCT_SERVICE", "reason": service})
        elif config["placeholder"]:
            blockers.append({"id": "PLACEHOLDER_SERVICE", "reason": service})
        elif not config["build"] and "@sha256:" not in str(config["image"]):
            blockers.append({"id": "MUTABLE_PRODUCT_IMAGE", "reason": service})

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

    if security_evidence is None:
        blockers.append(
            {
                "id": "MISSING_PRODUCTION_SECURITY_EVIDENCE",
                "reason": "no production TLS/mTLS and external-secret evidence supplied",
            }
        )
    else:
        try:
            resolved_security_evidence = security_evidence.resolve()
            if resolved_security_evidence.is_relative_to(root.resolve()):
                security_errors = [
                    "production security evidence must be outside the repository"
                ]
            else:
                security_errors = production_security_evidence_errors(
                    json.loads(
                        resolved_security_evidence.read_text(encoding="utf-8")
                    ),
                    root,
                    resolved_security_evidence.parent,
                )
        except (OSError, json.JSONDecodeError) as error:
            security_errors = [str(error)]
        blockers.extend(
            {"id": "INVALID_PRODUCTION_SECURITY_EVIDENCE", "reason": error}
            for error in security_errors
        )

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
        expected_status = "BLOCKED" if blockers else "READY"
        if data.get("status") != expected_status:
            errors.append(
                f"release readiness status must be {expected_status} for the recorded blockers"
            )
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path)
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--security-evidence", type=Path)
    parser.add_argument("--require-ready", action="store_true")
    parser.add_argument("--require-blocked", action="store_true")
    parser.add_argument("--verify-report", type=Path)
    args = parser.parse_args()
    if args.require_ready and args.require_blocked:
        parser.error("--require-ready and --require-blocked are mutually exclusive")
    if args.verify_report:
        if any(
            (
                args.report,
                args.evidence,
                args.security_evidence,
                args.require_ready,
                args.require_blocked,
            )
        ):
            parser.error("--verify-report cannot be combined with assessment options")
        try:
            verified_report = json.loads(args.verify_report.read_text(encoding="utf-8"))
            errors = readiness_report_errors(verified_report)
        except (OSError, json.JSONDecodeError) as error:
            errors = [str(error)]
        if errors:
            print("\n".join(errors))
            return 1
        print(f"release readiness report verified: {verified_report['status']}")
        return 0
    if args.report:
        args.report = args.report.resolve()
        if args.report.is_relative_to(ROOT.resolve()):
            parser.error("--report must be outside the repository")
    report = assess(ROOT, args.evidence, args.security_evidence)
    payload = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(payload, encoding="utf-8")
    print(payload, end="")
    if args.require_blocked:
        errors = readiness_report_errors(report)
        if report["status"] != "BLOCKED":
            errors.append("release readiness is not BLOCKED")
        if errors:
            print("\n".join(errors))
            return 1
    return 1 if args.require_ready and report["status"] != "READY" else 0


if __name__ == "__main__":
    raise SystemExit(main())
