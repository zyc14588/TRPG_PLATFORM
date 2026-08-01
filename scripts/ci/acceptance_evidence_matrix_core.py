"""Schema, rendering, and construction helpers for V1 acceptance evidence."""

from __future__ import annotations

import hashlib
import json
import re
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any

from repo_truth_core import ROOT, base_commit, git_tree_sha, worktree_diff_sha256
from repo_truth_evidence import validate_evidence


SCHEMA_VERSION = "v1"
GENERATOR_VERSION = "acceptance-evidence-matrix-v1"
DEFINITION_PATH = Path("V1_ACCEPTANCE_EVIDENCE_MATRIX.md")
STATUSES = {"PASS", "FAIL", "BLOCKED", "NOT_RUN"}
REPAIR_BATCHES = {
    "RF01": "F-AR01-001",
    "RF02": "F-AR01-002",
    "RF03": "F-AR01-003",
    "RF04": "F-AR01-005",
}
REPAIR_BATCH_COMMANDS = {
    batch_id: "bash scripts/ci/test-all.sh" for batch_id in REPAIR_BATCHES
}
REPAIR_BATCH_MINIMUM_NEGATIVE_TESTS = {
    "RF01": 5,
    "RF02": 4,
    "RF03": 5,
    "RF04": 4,
}
ROW_DEPENDENCIES = {
    2: ("RF02", "RF03"),
    7: ("RF04",),
    15: ("RF01", "RF02"),
    16: ("RF03",),
}
MANIFEST_KEYS = {
    "schema_version",
    "generator_version",
    "candidate_commit",
    "candidate_tree",
    "definition_sha256",
    "source_evidence",
    "repair_batches",
    "rows",
}
SOURCE_KEYS = {"path", "sha256"}
BATCH_KEYS = {"id", "finding", "status", "evidence"}
ROW_KEYS = {"id", "status", "command", "exit_code", "evidence", "notes"}
EMPTY_WORKTREE_DIFF_SHA256 = hashlib.sha256(b"").hexdigest()
EvidenceCache = dict[str, tuple[dict[str, Any] | None, list[str]]]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_manifest_text(data: dict[str, Any]) -> str:
    return json.dumps(data, indent=2, sort_keys=True, ensure_ascii=False) + "\n"


def acceptance_contracts(root: Path = ROOT) -> dict[int, tuple[str, str]]:
    contracts: dict[int, tuple[str, str]] = {}
    path = root / DEFINITION_PATH
    for line in path.read_text(encoding="utf-8").splitlines():
        match = re.match(
            r"^\|\s*(\d+)\s*\|\s*([^|]+?)\s*\|\s*[^|]+?\s*\|\s*([^|]+?)\s*\|",
            line,
        )
        if match:
            row_id = int(match.group(1))
            if row_id in contracts:
                raise ValueError(
                    "canonical V1 acceptance definition must contain each row exactly once"
                )
            contracts[row_id] = (match.group(2).strip(), match.group(3).strip())
    if set(contracts) != set(range(1, 18)):
        raise ValueError("canonical V1 acceptance definition must contain rows 1 through 17")
    return contracts


def acceptance_definitions(root: Path = ROOT) -> dict[int, str]:
    return {row_id: contract[0] for row_id, contract in acceptance_contracts(root).items()}


def acceptance_commands(root: Path = ROOT) -> dict[int, str]:
    return {row_id: contract[1] for row_id, contract in acceptance_contracts(root).items()}


def path_is_within(path: Path, parent: Path) -> bool:
    try:
        path.relative_to(parent)
    except ValueError:
        return False
    return True


def external_output_errors(path: Path, root: Path, label: str) -> list[str]:
    errors: list[str] = []
    if path_is_within(path.resolve(), root.resolve()):
        errors.append(f"{label} must be outside the repository")
    if path.is_symlink():
        errors.append(f"{label} must not be a symlink")
    return errors


def external_input_errors(path: Path, root: Path, label: str) -> list[str]:
    errors = external_output_errors(path, root, label)
    if not path.is_file():
        errors.append(f"{label} is not a readable regular file")
    return errors


def command_evidence(
    name: str,
    sources: dict[str, Path],
    root: Path,
    cache: EvidenceCache,
) -> tuple[dict[str, Any] | None, list[str]]:
    if name in cache:
        return cache[name]
    path = sources.get(name)
    errors: list[str] = []
    payload: dict[str, Any] | None = None
    if path is None:
        errors.append("references unknown source evidence")
    elif path.suffix != ".json":
        errors.append("must be a JSON command evidence report")
    else:
        try:
            raw = path.read_text(encoding="utf-8")
            loaded = json.loads(raw)
        except (OSError, json.JSONDecodeError) as error:
            errors.append(str(error))
        else:
            if not isinstance(loaded, dict):
                errors.append("must contain a JSON object")
            else:
                payload = loaded
                canonical = json.dumps(
                    payload, indent=2, sort_keys=True, ensure_ascii=False
                ) + "\n"
                if raw != canonical:
                    errors.append("must be canonical JSON")
                errors.extend(validate_evidence(payload, root, path.parent))
    cache[name] = (payload, errors)
    return cache[name]


def command_matches(payload: dict[str, Any], expected: str) -> bool:
    if payload.get("command") == expected:
        return True
    argv = payload.get("command_argv")
    return argv in (["bash", "-lc", expected], ["bash", "-c", expected])


def passing_test_names(payload: dict[str, Any], artifact_base: Path) -> set[str]:
    command_artifacts = payload.get("command_artifact_sha256")
    if not isinstance(command_artifacts, dict):
        return set()
    junit_names = [name for name in command_artifacts if name.endswith(".junit.xml")]
    if len(junit_names) != 1:
        return set()
    try:
        suite = ET.parse(artifact_base / junit_names[0]).getroot()
    except (OSError, ET.ParseError):
        return set()
    return {
        case.get("name", "")
        for case in suite.iter("testcase")
        if case.get("name")
        and all(case.find(result) is None for result in ("failure", "error", "skipped"))
    }


def row_command_evidence_errors(
    row: dict[str, Any],
    name: str,
    sources: dict[str, Path],
    root: Path,
    expected_command: str,
    cache: EvidenceCache,
) -> list[str]:
    label = f"row {row['id']} evidence {name}"
    payload, evidence_errors = command_evidence(name, sources, root, cache)
    errors = [f"{label}: {error}" for error in evidence_errors]
    if payload is None or evidence_errors:
        return errors
    if payload.get("status") != row["status"]:
        errors.append(f"{label}: status does not match row status")
    if payload.get("exit_code") != row["exit_code"]:
        errors.append(f"{label}: exit_code does not match row exit_code")
    if not command_matches(payload, expected_command):
        errors.append(f"{label}: command does not execute the canonical row command")
    if row.get("status") == "PASS" and "cargo test" in expected_command:
        executed_tests = passing_test_names(payload, sources[name].parent)
        if not any(test_name != payload.get("command") for test_name in executed_tests):
            errors.append(f"{label}: canonical cargo test executed no passing test cases")
    return errors


def validate_batch_dependencies(
    batches: object,
    rows: dict[int, dict[str, Any]],
) -> list[str]:
    errors: list[str] = []
    if not isinstance(batches, list):
        return errors
    for batch in batches:
        if (
            not isinstance(batch, dict)
            or batch.get("id") not in REPAIR_BATCHES
            or batch.get("status") != "PASS"
        ):
            continue
        batch_id = batch["id"]
        dependent_rows = sorted(
            row_id
            for row_id, dependencies in ROW_DEPENDENCIES.items()
            if batch_id in dependencies
        )
        for row_id in dependent_rows:
            row = rows.get(row_id)
            if row is None or row.get("status") != "PASS":
                errors.append(f"{batch_id} cannot PASS while row {row_id} is not PASS")
    return errors


def incomplete_reasons(data: dict[str, Any]) -> list[str]:
    reasons = [
        f"{batch['id']}={batch['status']}"
        for batch in data["repair_batches"]
        if batch["status"] != "PASS"
    ]
    reasons.extend(
        f"row {row['id']}={row['status']}"
        for row in data["rows"]
        if row["status"] != "PASS"
    )
    return reasons


def release_candidate_errors(
    data: dict[str, Any], root: Path = ROOT
) -> list[str]:
    errors = incomplete_reasons(data)
    if worktree_diff_sha256(root) != EMPTY_WORKTREE_DIFF_SHA256:
        errors.append("release candidate worktree must be clean")
    return errors


def _markdown_cell(value: object) -> str:
    return str(value).replace("|", "\\|").replace("\n", " ")


def render_matrix(
    data: dict[str, Any], manifest_sha256: str, root: Path = ROOT
) -> str:
    definitions = acceptance_definitions(root)
    sources = {item["path"]: item["sha256"] for item in data["source_evidence"]}
    lines = [
        "# V1 Acceptance Evidence Matrix (generated)",
        "",
        "> Generated from a machine-verifiable external evidence manifest. Do not edit.",
        "",
        f"- Candidate commit: `{data['candidate_commit']}`",
        f"- Candidate tree: `{data['candidate_tree']}`",
        f"- Definition SHA-256: `{data['definition_sha256']}`",
        f"- Evidence manifest SHA-256: `{manifest_sha256}`",
        "",
        "| # | V1 acceptance item | Status | Candidate commit | Command | Exit | Evidence SHA-256 | Notes |",
        "|---:|---|---|---|---|---:|---|---|",
    ]
    for row in data["rows"]:
        evidence = "<br>".join(
            f"`{name}`: `{sources[name]}`" for name in row["evidence"]
        )
        exit_code = "—" if row["exit_code"] is None else str(row["exit_code"])
        lines.append(
            "| {id} | {title} | {status} | `{commit}` | `{command}` | {exit_code} | {evidence} | {notes} |".format(
                id=row["id"],
                title=_markdown_cell(definitions[row["id"]]),
                status=row["status"],
                commit=data["candidate_commit"],
                command=_markdown_cell(row["command"]),
                exit_code=exit_code,
                evidence=evidence,
                notes=_markdown_cell(row["notes"]),
            )
        )
    return "\n".join(lines) + "\n"


def create_not_run_manifest(
    evidence_paths: list[Path], manifest_path: Path, root: Path = ROOT
) -> dict[str, Any]:
    errors = external_output_errors(manifest_path, root, "acceptance evidence manifest")
    if not evidence_paths:
        errors.append("at least one external source evidence file is required")
    sources = []
    seen: set[str] = set()
    for path in evidence_paths:
        errors.extend(external_input_errors(path, root, f"source evidence {path.name}"))
        if path.parent.resolve() != manifest_path.parent.resolve():
            errors.append(f"source evidence must be a sibling of the manifest: {path.name}")
        if path.name in seen:
            errors.append(f"duplicate source evidence path: {path.name}")
        elif path.is_file() and not path.is_symlink():
            seen.add(path.name)
            sources.append({"path": path.name, "sha256": sha256_file(path)})
    if errors:
        raise ValueError("\n".join(errors))
    evidence_names = sorted(seen)
    rows = []
    for row_id in range(1, 18):
        dependencies = ROW_DEPENDENCIES.get(row_id, ())
        status = "BLOCKED" if dependencies else "NOT_RUN"
        notes = (
            "Open repair batches: " + ", ".join(dependencies)
            if dependencies
            else "No row-specific release evidence was supplied."
        )
        rows.append(
            {
                "id": row_id,
                "status": status,
                "command": "NOT_RUN",
                "exit_code": None,
                "evidence": evidence_names,
                "notes": notes,
            }
        )
    return {
        "schema_version": SCHEMA_VERSION,
        "generator_version": GENERATOR_VERSION,
        "candidate_commit": base_commit(root),
        "candidate_tree": git_tree_sha(root),
        "definition_sha256": sha256_file(root / DEFINITION_PATH),
        "source_evidence": sorted(sources, key=lambda item: item["path"]),
        "repair_batches": [
            {"id": batch_id, "finding": finding, "status": "NOT_RUN", "evidence": []}
            for batch_id, finding in REPAIR_BATCHES.items()
        ],
        "rows": rows,
    }
