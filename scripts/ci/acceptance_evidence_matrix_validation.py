"""Fail-closed validation for V1 acceptance evidence manifests and matrices."""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

from acceptance_evidence_matrix_core import (
    BATCH_KEYS,
    DEFINITION_PATH,
    GENERATOR_VERSION,
    MANIFEST_KEYS,
    REPAIR_BATCHES,
    REPAIR_BATCH_COMMANDS,
    REPAIR_BATCH_MINIMUM_NEGATIVE_TESTS,
    ROOT,
    ROW_DEPENDENCIES,
    ROW_KEYS,
    SCHEMA_VERSION,
    SOURCE_KEYS,
    STATUSES,
    EvidenceCache,
    acceptance_commands,
    acceptance_definitions,
    base_commit,
    canonical_manifest_text,
    command_evidence,
    command_matches,
    external_input_errors,
    git_tree_sha,
    passing_test_names,
    path_is_within,
    render_matrix,
    row_command_evidence_errors,
    sha256_file,
    validate_batch_dependencies,
)


def _source_paths(
    data: dict[str, Any], manifest_path: Path, root: Path, errors: list[str]
) -> dict[str, Path]:
    sources = data.get("source_evidence")
    if not isinstance(sources, list) or not sources:
        errors.append("source_evidence must be a non-empty array")
        return {}
    paths: dict[str, Path] = {}
    base = manifest_path.parent.resolve()
    for index, source in enumerate(sources):
        label = f"source_evidence[{index}]"
        if not isinstance(source, dict) or set(source) != SOURCE_KEYS:
            errors.append(f"{label} must contain only path and sha256")
            continue
        name = source.get("path")
        if not isinstance(name, str) or Path(name).name != name:
            errors.append(f"{label}.path must be a sibling file name")
            continue
        if name in paths:
            errors.append(f"duplicate source evidence path: {name}")
            continue
        candidate = manifest_path.parent / name
        if not path_is_within(candidate.resolve(), base):
            errors.append(f"source evidence escapes manifest directory: {name}")
        errors.extend(external_input_errors(candidate, root, f"source evidence {name}"))
        expected = source.get("sha256")
        if not re.fullmatch(r"[0-9a-f]{64}", str(expected)):
            errors.append(f"invalid source evidence hash: {name}")
        elif candidate.is_file() and not candidate.is_symlink():
            if sha256_file(candidate) != expected:
                errors.append(f"source evidence hash mismatch: {name}")
        paths[name] = candidate
    return paths


def _reference_errors(
    references: object, source_names: set[str], label: str, require_nonempty: bool
) -> list[str]:
    if not isinstance(references, list) or not all(
        isinstance(item, str) for item in references
    ):
        return [f"{label} must be a string array"]
    errors: list[str] = []
    if require_nonempty and not references:
        errors.append(f"{label} must not be empty")
    if len(references) != len(set(references)):
        errors.append(f"{label} contains duplicate evidence references")
    for name in references:
        if name not in source_names:
            errors.append(f"{label} references unknown evidence: {name}")
    return errors


def _batch_closure_errors(
    batch: dict[str, Any],
    sources: dict[str, Path],
    candidate_commit: str,
    candidate_tree: str,
    root: Path,
    cache: EvidenceCache,
) -> list[str]:
    if batch.get("status") != "PASS":
        return []
    errors: list[str] = []
    references = batch.get("evidence")
    if not isinstance(references, list):
        return errors
    for name in references:
        if not isinstance(name, str) or name not in sources:
            continue
        label = f"{batch['id']} evidence {name}"
        payload, evidence_errors = command_evidence(name, sources, root, cache)
        errors.extend(f"{label}: {error}" for error in evidence_errors)
        if payload is None or evidence_errors:
            continue
        finding_status = payload.get("finding_status")
        finding_passed = False
        if isinstance(finding_status, dict):
            finding_passed = finding_status.get(batch["finding"]) == "PASS"
        elif isinstance(finding_status, list):
            finding_passed = any(
                isinstance(item, dict)
                and item.get("finding_id") == batch["finding"]
                and item.get("status") == "PASS"
                for item in finding_status
            )
        if payload.get("batch_id") != batch["id"]:
            errors.append(f"{label}: batch_id mismatch")
        if payload.get("head_sha") != candidate_commit:
            errors.append(f"{label}: head_sha mismatch")
        if payload.get("tree_sha") != candidate_tree:
            errors.append(f"{label}: tree_sha mismatch")
        if not finding_passed:
            errors.append(f"{label}: finding_status does not close {batch['finding']}")
        if not command_matches(payload, REPAIR_BATCH_COMMANDS[batch["id"]]):
            errors.append(
                f"{label}: command does not execute the repair batch acceptance gate"
            )
        negative_tests = payload.get("negative_tests")
        if (
            not isinstance(negative_tests, list)
            or not negative_tests
            or not all(isinstance(item, str) and item.strip() for item in negative_tests)
            or len(negative_tests) != len(set(negative_tests))
        ):
            errors.append(f"{label}: negative_tests must be a non-empty unique string array")
        else:
            minimum = REPAIR_BATCH_MINIMUM_NEGATIVE_TESTS[batch["id"]]
            if len(negative_tests) < minimum:
                errors.append(
                    f"{label}: requires at least {minimum} executed negative tests"
                )
            executed_tests = passing_test_names(payload, sources[name].parent)
            for test_name in negative_tests:
                if test_name not in executed_tests:
                    errors.append(
                        f"{label}: negative test was not executed successfully: {test_name}"
                    )
        if payload.get("not_run") != []:
            errors.append(f"{label}: PASS requires an empty not_run array")
    return errors


def manifest_errors(
    data: object, manifest_path: Path, root: Path = ROOT
) -> list[str]:
    errors = external_input_errors(manifest_path, root, "acceptance evidence manifest")
    if not isinstance(data, dict):
        return errors + ["acceptance evidence manifest must be a JSON object"]
    if set(data) != MANIFEST_KEYS:
        errors.append("acceptance evidence manifest fields do not match schema")
    if data.get("schema_version") != SCHEMA_VERSION:
        errors.append("acceptance evidence schema_version mismatch")
    if data.get("generator_version") != GENERATOR_VERSION:
        errors.append("acceptance evidence generator_version mismatch")

    current_commit = base_commit(root)
    current_tree = git_tree_sha(root)
    if data.get("candidate_commit") != current_commit:
        errors.append("acceptance matrix candidate commit does not equal HEAD")
    if data.get("candidate_tree") != current_tree:
        errors.append("acceptance matrix candidate tree does not equal HEAD tree")
    definition_path = root / DEFINITION_PATH
    commands: dict[int, str] = {}
    try:
        definition_sha256 = sha256_file(definition_path)
        acceptance_definitions(root)
        commands = acceptance_commands(root)
    except (OSError, ValueError) as error:
        errors.append(str(error))
    else:
        if data.get("definition_sha256") != definition_sha256:
            errors.append("acceptance definition hash mismatch")

    sources = _source_paths(data, manifest_path, root, errors)
    source_names = set(sources)
    evidence_cache: EvidenceCache = {}
    batch_status = _validate_batches(
        data.get("repair_batches"),
        sources,
        source_names,
        current_commit,
        current_tree,
        root,
        evidence_cache,
        errors,
    )
    row_records = _validate_rows(
        data.get("rows"),
        sources,
        source_names,
        batch_status,
        commands,
        root,
        evidence_cache,
        errors,
    )
    errors.extend(validate_batch_dependencies(data.get("repair_batches"), row_records))
    return errors


def _validate_batches(
    batches: object,
    sources: dict[str, Path],
    source_names: set[str],
    current_commit: str,
    current_tree: str,
    root: Path,
    evidence_cache: EvidenceCache,
    errors: list[str],
) -> dict[str, str]:
    batch_status: dict[str, str] = {}
    if not isinstance(batches, list):
        errors.append("repair_batches must be an array")
        return batch_status
    seen: set[str] = set()
    for index, batch in enumerate(batches):
        label = f"repair_batches[{index}]"
        if not isinstance(batch, dict) or set(batch) != BATCH_KEYS:
            errors.append(f"{label} fields do not match schema")
            continue
        batch_id = batch.get("id")
        if batch_id not in REPAIR_BATCHES or batch_id in seen:
            errors.append(f"invalid or duplicate repair batch: {batch_id}")
            continue
        seen.add(batch_id)
        if batch.get("finding") != REPAIR_BATCHES[batch_id]:
            errors.append(f"{batch_id} finding mismatch")
        status = batch.get("status")
        if status not in STATUSES:
            errors.append(f"{batch_id} has invalid status")
        else:
            batch_status[batch_id] = status
        errors.extend(
            _reference_errors(
                batch.get("evidence"), source_names, f"{batch_id}.evidence", status == "PASS"
            )
        )
        errors.extend(
            _batch_closure_errors(
                batch,
                sources,
                current_commit,
                current_tree,
                root,
                evidence_cache,
            )
        )
    if seen != set(REPAIR_BATCHES):
        errors.append("repair_batches must contain RF01 through RF04 exactly once")
    return batch_status


def _validate_rows(
    rows: object,
    sources: dict[str, Path],
    source_names: set[str],
    batch_status: dict[str, str],
    commands: dict[int, str],
    root: Path,
    evidence_cache: EvidenceCache,
    errors: list[str],
) -> dict[int, dict[str, Any]]:
    row_records: dict[int, dict[str, Any]] = {}
    if not isinstance(rows, list):
        errors.append("rows must be an array")
        return row_records
    seen_rows: set[int] = set()
    for index, row in enumerate(rows):
        label = f"rows[{index}]"
        if not isinstance(row, dict) or set(row) != ROW_KEYS:
            errors.append(f"{label} fields do not match schema")
            continue
        row_id = row.get("id")
        if type(row_id) is not int or row_id not in range(1, 18) or row_id in seen_rows:
            errors.append(f"invalid or duplicate acceptance row: {row_id}")
            continue
        seen_rows.add(row_id)
        row_records[row_id] = row
        status = row.get("status")
        if status not in STATUSES:
            errors.append(f"row {row_id} has invalid status")
        command = row.get("command")
        if not isinstance(command, str) or not command:
            errors.append(f"row {row_id} command must be a non-empty string")
        exit_code = row.get("exit_code")
        if status == "PASS" and (type(exit_code) is not int or exit_code != 0):
            errors.append(f"row {row_id} PASS requires exit_code 0")
        elif status == "FAIL" and (type(exit_code) is not int or exit_code == 0):
            errors.append(f"row {row_id} FAIL requires a non-zero exit_code")
        elif status in {"BLOCKED", "NOT_RUN"} and exit_code is not None:
            errors.append(f"row {row_id} {status} requires a null exit_code")
        if status in {"PASS", "FAIL"} and command == "NOT_RUN":
            errors.append(f"row {row_id} {status} requires an executed command")
        if status in {"BLOCKED", "NOT_RUN"} and command != "NOT_RUN":
            errors.append(f"row {row_id} {status} command must be NOT_RUN")
        references = row.get("evidence")
        errors.extend(
            _reference_errors(references, source_names, f"row {row_id}.evidence", True)
        )
        if status in {"PASS", "FAIL"}:
            expected_command = commands.get(row_id)
            if expected_command is not None and command != expected_command:
                errors.append(
                    f"row {row_id} command does not match canonical V1 acceptance command"
                )
            if isinstance(references, list) and expected_command is not None:
                for name in references:
                    if isinstance(name, str) and name in source_names:
                        errors.extend(
                            row_command_evidence_errors(
                                row,
                                name,
                                sources,
                                root,
                                expected_command,
                                evidence_cache,
                            )
                        )
        if not isinstance(row.get("notes"), str) or not row.get("notes", "").strip():
            errors.append(f"row {row_id} notes must be non-empty")
        if status == "PASS":
            for batch_id in ROW_DEPENDENCIES.get(row_id, ()):
                if batch_status.get(batch_id) != "PASS":
                    errors.append(f"row {row_id} cannot PASS while {batch_id} is not closed")
    if seen_rows != set(range(1, 18)):
        errors.append("acceptance matrix must contain rows 1 through 17 exactly once")
    return row_records


def load_manifest(
    manifest_path: Path, root: Path = ROOT
) -> tuple[dict[str, Any] | None, list[str]]:
    try:
        raw = manifest_path.read_text(encoding="utf-8")
        data = json.loads(raw)
    except (OSError, json.JSONDecodeError) as error:
        return None, external_input_errors(
            manifest_path, root, "acceptance evidence manifest"
        ) + [str(error)]
    errors = manifest_errors(data, manifest_path, root)
    if isinstance(data, dict) and raw != canonical_manifest_text(data):
        errors.append("acceptance evidence manifest is not canonical JSON")
    return data if isinstance(data, dict) else None, errors


def matrix_file_errors(
    manifest_path: Path, matrix_path: Path, root: Path = ROOT
) -> tuple[dict[str, Any] | None, list[str]]:
    data, errors = load_manifest(manifest_path, root)
    errors.extend(external_input_errors(matrix_path, root, "generated acceptance matrix"))
    if data is None or errors:
        return data, errors
    expected = render_matrix(data, sha256_file(manifest_path), root)
    try:
        actual = matrix_path.read_text(encoding="utf-8")
    except OSError as error:
        errors.append(str(error))
    else:
        if actual != expected:
            errors.append("generated acceptance matrix does not match evidence manifest")
    return data, errors
