#!/usr/bin/env python3
"""Generate and verify the V1 acceptance matrix from external evidence."""

from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
from pathlib import Path
from typing import Any

from acceptance_evidence_matrix_core import (
    DEFINITION_PATH,
    GENERATOR_VERSION,
    REPAIR_BATCH_COMMANDS,
    REPAIR_BATCHES,
    ROOT,
    SCHEMA_VERSION,
    acceptance_commands,
    base_commit,
    canonical_manifest_text,
    command_evidence,
    command_matches,
    create_not_run_manifest,
    external_input_errors,
    external_output_errors,
    git_tree_sha,
    incomplete_reasons,
    passing_test_names,
    release_candidate_errors,
    render_matrix,
    sha256_file,
)
from acceptance_evidence_matrix_validation import (
    load_manifest,
    manifest_errors,
    matrix_file_errors,
)


REPAIR_BATCH_NEGATIVE_TESTS = {
    "RF01": (
        "forged_all_true_assessment_cannot_issue_a_level4_certificate",
        "caller_supplied_runtime_identity_cannot_be_certified",
        "model_suite_runtime_and_evidence_tampering_invalidates_certificate",
        "injection_visibility_and_tool_instability_fail_closed",
        "timeout_and_partial_execution_cannot_reach_level4",
    ),
    "RF02": (
        "certification_registry_rejects_revocation_tail_truncation",
        "certification_registry_rejects_combined_log_and_anchor_snapshot_rollback",
        "certification_registry_rejects_a_forged_external_checkpoint_mac",
        "local_ai_keeper_without_level4_certification_fails_closed",
    ),
    "RF03": (
        "a_local_provider_label_cannot_hide_a_remote_https_endpoint",
        "local_provider_transport_requires_an_exact_private_network_policy_match",
        "production_provider_rejects_development_memory_credentials",
        "provider_endpoint_rejects_secret_carriers_and_plaintext_production_transport",
        "local_failure_does_not_contact_the_configured_cloud_provider",
        "real_local_embedding::ollama_fresh_path",
        "real_local_embedding::llama_cpp_fresh_path",
        "real_cloud_chat::fresh_path",
    ),
    "RF04": (
        "agent_job::decision_validation_tests::absent_output_and_multiple_provider_calls_fail_closed",
        "agent_job::public_gameplay_tool_binding_tests::model_cannot_change_canonical_gameplay_ids_or_choices",
        "denied_formal_authorization_never_invokes_the_tool_executor",
        "invisible_rag_and_unauthorized_tools_and_invalid_output_fail_closed",
    ),
}


def _executed_evidence_status(path: Path, label: str) -> tuple[str, int]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"{label}: {error}") from error
    if not isinstance(payload, dict):
        raise ValueError(f"{label} must contain a JSON object")
    status = payload.get("status")
    exit_code = payload.get("exit_code")
    if status not in {"PASS", "FAIL"} or type(exit_code) is not int:
        raise ValueError(f"{label} must record an executed PASS or FAIL command")
    if (status == "PASS") != (exit_code == 0):
        raise ValueError(f"{label} status does not match exit_code")
    return status, exit_code


def create_final_manifest(
    row_evidence: dict[int, Path],
    batch_evidence: dict[str, Path],
    manifest_path: Path,
    root: Path = ROOT,
) -> dict[str, Any]:
    """Derive every candidate status from external executed-command evidence."""
    errors = external_output_errors(manifest_path, root, "acceptance evidence manifest")
    if set(row_evidence) != set(range(1, 18)):
        errors.append("final manifest requires row evidence for rows 1 through 17 exactly once")
    if set(batch_evidence) != set(REPAIR_BATCHES):
        errors.append("final manifest requires RF01 through RF04 evidence exactly once")
    assigned = [
        (f"row {row_id} evidence", path)
        for row_id, path in sorted(row_evidence.items())
    ] + [
        (f"{batch_id} evidence", path)
        for batch_id, path in sorted(batch_evidence.items())
    ]
    seen_names: set[str] = set()
    sources: list[dict[str, str]] = []
    for label, path in assigned:
        errors.extend(external_input_errors(path, root, label))
        if path.parent.resolve() != manifest_path.parent.resolve():
            errors.append(f"{label} must be a sibling of the manifest")
        if path.name in seen_names:
            errors.append(f"external evidence files must be unique: {path.name}")
        elif path.is_file() and not path.is_symlink():
            seen_names.add(path.name)
            sources.append({"path": path.name, "sha256": sha256_file(path)})
    if errors:
        raise ValueError("\n".join(errors))

    commands = acceptance_commands(root)
    rows = []
    for row_id in range(1, 18):
        path = row_evidence[row_id]
        status, exit_code = _executed_evidence_status(path, f"row {row_id} evidence")
        rows.append(
            {
                "id": row_id,
                "status": status,
                "command": commands[row_id],
                "exit_code": exit_code,
                "evidence": [path.name],
                "notes": "Status derived from the bound canonical command evidence.",
            }
        )
    batches = []
    for batch_id, finding in REPAIR_BATCHES.items():
        path = batch_evidence[batch_id]
        status, _ = _executed_evidence_status(path, f"{batch_id} evidence")
        batches.append(
            {
                "id": batch_id,
                "finding": finding,
                "status": status,
                "evidence": [path.name],
            }
        )
    return {
        "schema_version": SCHEMA_VERSION,
        "generator_version": GENERATOR_VERSION,
        "candidate_commit": base_commit(root),
        "candidate_tree": git_tree_sha(root),
        "definition_sha256": sha256_file(root / DEFINITION_PATH),
        "source_evidence": sorted(sources, key=lambda item: item["path"]),
        "repair_batches": batches,
        "rows": rows,
    }


def derive_batch_closure_payloads(
    release_evidence: Path,
    output_directory: Path,
    root: Path = ROOT,
) -> dict[str, dict[str, Any]]:
    """Derive RF01-RF04 closures from one passing full-suite evidence report."""
    errors = external_input_errors(release_evidence, root, "release batch closure evidence")
    if release_evidence.parent.resolve() != output_directory.resolve():
        errors.append("release batch closure evidence must be in the output directory")
    for batch_id in REPAIR_BATCHES:
        errors.extend(
            external_output_errors(
                output_directory / f"{batch_id.lower()}-closure.json",
                root,
                f"{batch_id} closure evidence",
            )
        )
    if errors:
        raise ValueError("\n".join(errors))

    name = release_evidence.name
    payload, evidence_errors = command_evidence(name, {name: release_evidence}, root, {})
    if evidence_errors or payload is None:
        raise ValueError(
            "\n".join(
                f"release batch closure evidence: {error}" for error in evidence_errors
            )
        )
    if payload.get("status") != "PASS" or payload.get("exit_code") != 0:
        raise ValueError("release batch closure evidence must record a passing command")
    if not command_matches(payload, REPAIR_BATCH_COMMANDS["RF01"]):
        raise ValueError(
            "release batch closure evidence must execute the full repair acceptance gate"
        )
    executed_tests = passing_test_names(payload, output_directory)
    closures: dict[str, dict[str, Any]] = {}
    for batch_id, finding in REPAIR_BATCHES.items():
        required_tests = REPAIR_BATCH_NEGATIVE_TESTS[batch_id]
        missing = [name for name in required_tests if name not in executed_tests]
        if missing:
            raise ValueError(
                f"{batch_id} closure evidence is missing passing negative tests: "
                + ", ".join(missing)
            )
        closure = dict(payload)
        closure.update(
            batch_id=batch_id,
            head_sha=payload.get("base_commit"),
            tree_sha=payload.get("tree_sha"),
            finding_status={finding: "PASS"},
            negative_tests=list(required_tests),
            not_run=[],
        )
        closures[batch_id] = closure
    return closures


def _write_external(path: Path, text: str, root: Path, label: str) -> None:
    errors = external_output_errors(path, root, label)
    if errors:
        raise ValueError("\n".join(errors))
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _assignments(
    values: list[str], expected: set[str], label: str
) -> dict[str, Path]:
    assigned: dict[str, Path] = {}
    for value in values:
        key, separator, encoded_path = value.partition("=")
        if not separator or key not in expected or not encoded_path:
            raise ValueError(f"invalid {label} assignment: {value}")
        if key in assigned:
            raise ValueError(f"duplicate {label} assignment: {key}")
        assigned[key] = Path(encoded_path)
    if set(assigned) != expected:
        missing = ", ".join(sorted(expected - set(assigned)))
        raise ValueError(f"missing {label} assignments: {missing}")
    return assigned


def _write_verified_manifest(path: Path, data: dict) -> None:
    errors = external_output_errors(path, ROOT, "acceptance evidence manifest")
    if errors:
        raise ValueError("\n".join(errors))
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            handle.write(canonical_manifest_text(data))
        _, validation_errors = load_manifest(temporary)
        if validation_errors:
            raise ValueError("\n".join(validation_errors))
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    create = subparsers.add_parser("create-not-run")
    create.add_argument("--manifest", type=Path, required=True)
    create.add_argument("--evidence", type=Path, action="append", required=True)

    derive = subparsers.add_parser("derive-batch-closures")
    derive.add_argument("--evidence", type=Path, required=True)
    derive.add_argument("--output-directory", type=Path, required=True)

    final = subparsers.add_parser("create-final")
    final.add_argument("--manifest", type=Path, required=True)
    final.add_argument("--row-evidence", action="append", required=True)
    final.add_argument("--batch-evidence", action="append", required=True)

    generate = subparsers.add_parser("generate")
    generate.add_argument("--manifest", type=Path, required=True)
    generate.add_argument("--output", type=Path, required=True)

    validate = subparsers.add_parser("validate")
    validate.add_argument("--manifest", type=Path, required=True)
    validate.add_argument("--matrix", type=Path, required=True)
    validate.add_argument("--require-ready", action="store_true")
    args = parser.parse_args()

    try:
        if args.command == "create-not-run":
            data = create_not_run_manifest(args.evidence, args.manifest)
            _write_external(
                args.manifest,
                canonical_manifest_text(data),
                ROOT,
                "acceptance evidence manifest",
            )
            print(f"acceptance evidence manifest created: {args.manifest}")
            return 0
        if args.command == "derive-batch-closures":
            if args.output_directory.is_symlink():
                raise ValueError("batch closure output directory must not be a symlink")
            args.output_directory.mkdir(parents=True, exist_ok=True)
            closures = derive_batch_closure_payloads(
                args.evidence, args.output_directory
            )
            for batch_id, payload in closures.items():
                output = args.output_directory / f"{batch_id.lower()}-closure.json"
                _write_external(
                    output,
                    canonical_manifest_text(payload),
                    ROOT,
                    f"{batch_id} closure evidence",
                )
                print(f"{batch_id} closure evidence derived: {output}")
            return 0
        if args.command == "create-final":
            rows = _assignments(
                args.row_evidence,
                {str(row_id) for row_id in range(1, 18)},
                "row evidence",
            )
            batches = _assignments(
                args.batch_evidence,
                {"RF01", "RF02", "RF03", "RF04"},
                "batch evidence",
            )
            data = create_final_manifest(
                {int(row_id): path for row_id, path in rows.items()},
                batches,
                args.manifest,
            )
            _write_verified_manifest(args.manifest, data)
            print(f"final acceptance evidence manifest created: {args.manifest}")
            return 0
        if args.command == "generate":
            data, errors = load_manifest(args.manifest)
            errors.extend(
                external_output_errors(args.output, ROOT, "generated acceptance matrix")
            )
            if errors or data is None:
                print("\n".join(errors), file=sys.stderr)
                return 1
            _write_external(
                args.output,
                render_matrix(data, sha256_file(args.manifest)),
                ROOT,
                "generated acceptance matrix",
            )
            print(f"acceptance matrix generated: {args.output}")
            return 0
        data, errors = matrix_file_errors(args.manifest, args.matrix)
        if args.require_ready and data is not None and not errors:
            errors.extend(release_candidate_errors(data))
        if errors:
            print("\n".join(errors), file=sys.stderr)
            return 1
        print("acceptance evidence matrix verified")
        return 0
    except (OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
