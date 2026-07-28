#!/usr/bin/env python3
"""Shared, dependency-free repository truth helpers for P00A gates."""

from __future__ import annotations

import hashlib
import json
import os
import platform
import re
import shlex
import subprocess
import sys
import xml.etree.ElementTree as ET
from datetime import datetime
from functools import lru_cache
from pathlib import Path

from repo_truth_core import *

"""Validation rules for immutable repository evidence."""
def validate_evidence(
    data: dict,
    root: Path = ROOT,
    artifact_base: Path | None = None,
    live_context: bool = False,
) -> list[str]:
    required = set(EVIDENCE_REQUIRED)
    errors = [f"missing field: {name}" for name in sorted(required - data.keys())]
    if data.get("generator_version") != EVIDENCE_GENERATOR_VERSION:
        errors.append("generator_version mismatch")
    try:
        generated_at = datetime.fromisoformat(str(data.get("generated_at_utc")))
        if generated_at.tzinfo is None:
            raise ValueError
    except ValueError:
        errors.append("generated_at_utc must be an ISO-8601 timestamp with timezone")
    if data.get("status") not in EVIDENCE_STATUSES:
        errors.append("invalid status")
    exit_code = data.get("exit_code")
    if type(exit_code) is not int:
        errors.append("exit_code must be an integer")
    elif (data.get("status") == "PASS") != (exit_code == 0):
        errors.append("status does not match exit_code")
    if data.get("semantic_status") != data.get("status"):
        errors.append("semantic_status does not match derived status")
    if not isinstance(data.get("command"), str) or not data.get("command"):
        errors.append("command must be a non-empty string")
    argv = data.get("command_argv")
    if not isinstance(argv, list) or not argv or not all(isinstance(item, str) for item in argv):
        errors.append("command_argv must be a non-empty string array")
    elif any(any(control in item for control in ("\0", "\n", "\r")) for item in argv):
        errors.append("command_argv must not contain control characters")
    elif data.get("command") != shlex.join(argv):
        errors.append("command does not match command_argv")
    if data.get("base_commit") != base_commit(root):
        errors.append("base_commit mismatch")
    if data.get("github_sha") != data.get("base_commit"):
        errors.append("github_sha does not match base_commit")
    try:
        expected_repository = repository_slug(root)
    except ValueError as error:
        errors.append(str(error))
    else:
        if data.get("repository") != expected_repository:
            errors.append("repository does not match origin")
    for name in ("github_run_id", "github_run_attempt"):
        if not re.fullmatch(r"LOCAL|[1-9][0-9]*", str(data.get(name, ""))):
            errors.append(f"invalid {name}")
    for name in ("workflow", "job", "runner_os"):
        if not isinstance(data.get(name), str) or not data.get(name):
            errors.append(f"{name} must be a non-empty string")
    if live_context:
        live_fields = {
            "repository": "GITHUB_REPOSITORY",
            "github_sha": "GITHUB_SHA",
            "github_run_id": "GITHUB_RUN_ID",
            "github_run_attempt": "GITHUB_RUN_ATTEMPT",
            "workflow": "GITHUB_WORKFLOW",
            "job": "GITHUB_JOB",
            "runner_os": "RUNNER_OS",
        }
        for field, environment_name in live_fields.items():
            expected = os.environ.get(environment_name)
            if not expected:
                errors.append(f"missing live context: {environment_name}")
            elif str(data.get(field)) != expected:
                errors.append(f"{field} does not match live GitHub context")
    if not re.fullmatch(r"[0-9a-f]{64}", str(data.get("worktree_diff_sha256", ""))):
        errors.append("invalid worktree_diff_sha256")
    elif data.get("worktree_diff_sha256") != worktree_diff_sha256(root):
        errors.append("worktree_diff_sha256 mismatch")
    tool_versions = data.get("tool_versions")
    if not isinstance(tool_versions, dict) or not tool_versions:
        errors.append("tool_versions must be a non-empty object")
    else:
        for name in ("platform", "python", "rustc", "cargo", "node", "npm", "pnpm"):
            if not tool_versions.get(name) or tool_versions[name] == "NOT_VERIFIED":
                errors.append(f"tool version not verified: {name}")
        if live_context:
            actual_versions = current_tool_versions(root)
            for name, version in tool_versions.items():
                if version != actual_versions.get(name):
                    errors.append(f"tool version does not match current environment: {name}")
        rust_match = re.search(
            r'(?m)^channel\s*=\s*"([^"]+)"',
            (root / "rust-toolchain.toml").read_text(encoding="utf-8"),
        )
        node_pin = (root / ".nvmrc").read_text(encoding="utf-8").strip()
        package_manager = json.loads((root / "package.json").read_text(encoding="utf-8"))[
            "packageManager"
        ]
        pnpm_pin = package_manager.removeprefix("pnpm@")
        if rust_match:
            for name in ("rustc", "cargo"):
                if not re.match(
                    rf"^{name} {re.escape(rust_match.group(1))}(?:\s|$)",
                    str(tool_versions.get(name, "")),
                ):
                    errors.append(f"{name} version does not match rust-toolchain.toml")
        python_pin_path = root / ".python-version"
        if python_pin_path.is_file() and tool_versions.get("python") != python_pin_path.read_text(
            encoding="utf-8"
        ).strip():
            errors.append("python version does not match .python-version")
        if str(tool_versions.get("node", "")).removeprefix("v") != node_pin:
            errors.append("node version does not match .nvmrc")
        if str(tool_versions.get("pnpm", "")) != pnpm_pin:
            errors.append("pnpm version does not match packageManager")
    environment = data.get("environment")
    if not isinstance(environment, dict) or set(environment) != {
        "variables",
        "service_versions",
    }:
        errors.append("environment must contain variables and service_versions")
    else:
        variables = environment.get("variables")
        if not isinstance(variables, dict):
            errors.append("environment variables must be an object")
        else:
            for name, digest in variables.items():
                if not isinstance(name, str) or not re.fullmatch(
                    r"[A-Z_][A-Z0-9_]{0,127}", name
                ):
                    errors.append(f"invalid environment variable name: {name}")
                elif not re.fullmatch(r"sha256:[0-9a-f]{64}", str(digest)):
                    errors.append(f"invalid environment variable digest: {name}")
                elif live_context:
                    current = os.environ.get(name)
                    if current is None:
                        errors.append(f"missing bound environment variable: {name}")
                    elif digest != f"sha256:{hashlib.sha256(current.encode('utf-8')).hexdigest()}":
                        errors.append(f"environment variable digest mismatch: {name}")
        service_versions = environment.get("service_versions")
        if not isinstance(service_versions, dict):
            errors.append("service_versions must be an object")
        else:
            for name, record in service_versions.items():
                if not isinstance(name, str) or not re.fullmatch(
                    r"[a-z0-9][a-z0-9_.-]{0,63}", name
                ):
                    errors.append(f"invalid service version name: {name}")
                    continue
                if not isinstance(record, dict) or set(record) != {
                    "command",
                    "command_argv",
                    "exit_code",
                    "output",
                }:
                    errors.append(f"invalid service version record: {name}")
                    continue
                service_argv = record.get("command_argv")
                if (
                    not isinstance(service_argv, list)
                    or not service_argv
                    or not all(isinstance(item, str) and item for item in service_argv)
                    or any(
                        any(control in item for control in ("\0", "\n", "\r"))
                        for item in service_argv
                    )
                ):
                    errors.append(f"invalid service version command_argv: {name}")
                    continue
                if record.get("command") != shlex.join(service_argv):
                    errors.append(f"service version command mismatch: {name}")
                if type(record.get("exit_code")) is not int:
                    errors.append(f"service version exit_code must be an integer: {name}")
                if not isinstance(record.get("output"), str):
                    errors.append(f"service version output must be a string: {name}")
                if data.get("status") == "PASS" and (
                    record.get("exit_code") != 0 or not record.get("output", "").strip()
                ):
                    errors.append(f"service version was not verified: {name}")
                if live_context and record != service_version_record(service_argv, root):
                    errors.append(f"service version does not match current environment: {name}")
    if isinstance(tool_versions, dict) and isinstance(environment, dict):
        if data.get("environment_sha256") != evidence_environment_sha256(
            tool_versions, environment
        ):
            errors.append("environment_sha256 mismatch")
    command_output = data.get("command_output")
    if not isinstance(command_output, dict) or set(command_output) != {
        "stdout_bytes",
        "stdout_sha256",
        "stderr_bytes",
        "stderr_sha256",
    }:
        errors.append("command_output metadata is invalid")
    else:
        for stream in ("stdout", "stderr"):
            if type(command_output.get(f"{stream}_bytes")) is not int or command_output[
                f"{stream}_bytes"
            ] < 0:
                errors.append(f"invalid {stream} byte count")
            if not re.fullmatch(
                r"[0-9a-f]{64}", str(command_output.get(f"{stream}_sha256", ""))
            ):
                errors.append(f"invalid {stream} digest")
    artifacts = data.get("artifact_sha256")
    if not isinstance(artifacts, dict) or not artifacts:
        errors.append("artifact_sha256 must be a non-empty object")
    else:
        tracked = git_modes(root)
        for name, expected in artifacts.items():
            try:
                path = repository_artifact_path(name, root)
            except (TypeError, ValueError) as error:
                errors.append(str(error))
                continue
            if name not in tracked:
                errors.append(f"artifact is not tracked: {name}")
            elif not re.fullmatch(r"[0-9a-f]{64}", str(expected)):
                errors.append(f"invalid artifact hash: {name}")
            elif not path.is_file() or sha256_file(path) != expected:
                errors.append(f"artifact hash mismatch: {name}")
    generated = data.get("generated_artifact_sha256")
    if not isinstance(generated, dict) or not generated:
        errors.append("generated_artifact_sha256 must be a non-empty object")
    else:
        for name, expected in generated.items():
            candidate = artifact_base / name if artifact_base is not None else None
            if Path(name).name != name or artifact_base is None:
                errors.append(f"invalid generated artifact path: {name}")
            elif candidate.is_symlink():
                errors.append(f"generated artifact must not be a symlink: {name}")
            elif not candidate.resolve().is_relative_to(artifact_base.resolve()):
                errors.append(f"generated artifact escapes artifact directory: {name}")
            elif not re.fullmatch(r"[0-9a-f]{64}", str(expected)):
                errors.append(f"invalid generated artifact hash: {name}")
            elif not candidate.is_file() or sha256_file(candidate) != expected:
                errors.append(f"generated artifact hash mismatch: {name}")

    command_artifacts = data.get("command_artifact_sha256")
    if not isinstance(command_artifacts, dict) or not command_artifacts:
        errors.append("command_artifact_sha256 must be a non-empty object")
    else:
        command_by_suffix = {
            suffix: [name for name in command_artifacts if name.endswith(suffix)]
            for suffix in (".log", ".junit.xml", ".sarif")
        }
        if len(command_artifacts) != 3:
            errors.append("command_artifact_sha256 must contain exactly three artifacts")
        for suffix, names in command_by_suffix.items():
            if len(names) != 1:
                errors.append(f"expected exactly one command artifact: *{suffix}")
        if isinstance(generated, dict):
            for name, expected in command_artifacts.items():
                if name not in generated:
                    errors.append(f"command artifact is not a generated artifact: {name}")
                elif generated[name] != expected:
                    errors.append(f"command artifact hash mismatch: {name}")
        if artifact_base is not None and all(
            len(names) == 1 for names in command_by_suffix.values()
        ):
            bound_outputs = []
            for name in command_by_suffix[".log"]:
                raw_path = artifact_base / name
                if raw_path.is_file():
                    parsed = parse_bound_raw_output(
                        raw_path.read_bytes(), str(data.get("command", ""))
                    )
                    if parsed is not None and parsed[2] == data.get("exit_code"):
                        bound_outputs.append(parsed)
            if len(bound_outputs) != 1:
                errors.append("expected exactly one raw output bound to command and exit_code")
            else:
                stdout_bytes, stderr_bytes, _ = bound_outputs[0]
                expected_output = {
                    "stdout_bytes": len(stdout_bytes),
                    "stdout_sha256": hashlib.sha256(stdout_bytes).hexdigest(),
                    "stderr_bytes": len(stderr_bytes),
                    "stderr_sha256": hashlib.sha256(stderr_bytes).hexdigest(),
                }
                if command_output != expected_output:
                    errors.append("command_output does not match bound raw output")
                if exit_code == 0 and false_skip_markers(
                    stdout_bytes.decode("utf-8", errors="replace"),
                    stderr_bytes.decode("utf-8", errors="replace"),
                ):
                    errors.append("passing evidence contains a deceptive skip marker")
                junit_path = artifact_base / command_by_suffix[".junit.xml"][0]
                try:
                    suite = ET.parse(junit_path).getroot()
                except (OSError, ET.ParseError):
                    errors.append("generated JUnit is not valid XML")
                else:
                    expected_cases = evidence_test_cases(
                        str(data.get("command", "")),
                        exit_code if type(exit_code) is int else -1,
                        stdout_bytes.decode("utf-8", errors="replace"),
                        stderr_bytes.decode("utf-8", errors="replace"),
                    )
                    actual_cases = []
                    for case in suite.findall("testcase"):
                        status = (
                            "FAILED"
                            if case.find("failure") is not None
                            else "ignored"
                            if case.find("skipped") is not None
                            else "ok"
                        )
                        actual_cases.append((case.get("name", ""), status))
                    if actual_cases != expected_cases:
                        errors.append("JUnit test details do not match bound raw output")
                    expected_failures = sum(
                        status == "FAILED" for _, status in expected_cases
                    )
                    expected_skipped = sum(
                        status == "ignored" for _, status in expected_cases
                    )
                    if suite.get("tests") != str(len(expected_cases)):
                        errors.append("JUnit test count does not match bound raw output")
                    if suite.get("failures") != str(expected_failures):
                        errors.append("JUnit failure count does not match bound raw output")
                    if suite.get("skipped", "0") != str(expected_skipped):
                        errors.append("JUnit skipped count does not match bound raw output")
    reports = data.get("report_files")
    if not isinstance(reports, dict) or not reports:
        errors.append("report_files must be a non-empty object")
    elif isinstance(generated, dict):
        if set(reports) != set(generated):
            errors.append("report_files do not match generated artifacts")
        for name, metadata in reports.items():
            if not isinstance(metadata, dict):
                errors.append(f"invalid report metadata: {name}")
                continue
            if metadata.get("path") != name:
                errors.append(f"report path mismatch: {name}")
            candidate = artifact_base / name if artifact_base is not None else None
            if candidate is None or not candidate.is_file():
                errors.append(f"missing report file: {name}")
            elif metadata.get("size_bytes") != candidate.stat().st_size:
                errors.append(f"report size mismatch: {name}")
            if metadata.get("sha256") != generated.get(name):
                errors.append(f"report hash mismatch: {name}")
    return errors
