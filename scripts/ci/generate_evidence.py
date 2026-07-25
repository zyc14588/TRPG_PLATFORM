#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shlex
import subprocess
import sys
import xml.etree.ElementTree as ET
from datetime import datetime, timezone
from pathlib import Path

from repo_truth import (
    EVIDENCE_GENERATOR_VERSION,
    ROOT,
    base_commit,
    current_tool_versions,
    evidence_environment_sha256,
    evidence_test_cases,
    false_skip_markers,
    git_modes,
    repository_artifact_path,
    repository_slug,
    sha256_file,
    service_version_record,
    worktree_diff_sha256,
)


def write_junit(
    path: Path, command: str, exit_code: int, stdout: str, stderr: str
) -> None:
    cases = evidence_test_cases(command, exit_code, stdout, stderr)
    failures = sum(status == "FAILED" for _, status in cases)
    skipped = sum(status == "ignored" for _, status in cases)
    suite = ET.Element(
        "testsuite",
        name="repository-evidence",
        tests=str(len(cases)),
        failures=str(failures),
        skipped=str(skipped),
    )
    for name, status in cases:
        case = ET.SubElement(
            suite,
            "testcase",
            classname="cargo.test" if name != command else "ci.command",
            name=name,
        )
        if status == "FAILED":
            ET.SubElement(case, "failure", message=f"command exited {exit_code}")
        elif status == "ignored":
            ET.SubElement(case, "skipped", message="test ignored by harness")
    ET.ElementTree(suite).write(path, encoding="utf-8", xml_declaration=True)


def write_sarif(path: Path, command: str, exit_code: int) -> None:
    results = []
    if exit_code:
        results.append(
            {
                "ruleId": "command-failed",
                "level": "error",
                "message": {"text": f"Command exited {exit_code}: {command}"},
            }
        )
    payload = {
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [
            {
                "tool": {
                    "driver": {
                        "name": "coc-ai-trpg-evidence",
                        "version": EVIDENCE_GENERATOR_VERSION,
                    }
                },
                "results": results,
            }
        ],
    }
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--artifact", action="append", required=True)
    parser.add_argument("--generated-artifact", action="append", type=Path, default=[])
    parser.add_argument("--environment-key", action="append", default=[])
    parser.add_argument("--service-version-command", action="append", default=[])
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("a command is required after --")
    if any(any(control in item for control in ("\0", "\n", "\r")) for item in command):
        parser.error("command arguments must not contain control characters")

    environment_variables = {}
    for name in args.environment_key:
        if not re.fullmatch(r"[A-Z_][A-Z0-9_]{0,127}", name):
            parser.error(f"invalid --environment-key: {name}")
        if name in environment_variables:
            parser.error(f"duplicate --environment-key: {name}")
        value = os.environ.get(name)
        if value is None:
            parser.error(f"missing --environment-key: {name}")
        environment_variables[name] = (
            "sha256:" + hashlib.sha256(value.encode("utf-8")).hexdigest()
        )

    service_commands = {}
    for encoded in args.service_version_command:
        try:
            specification = json.loads(encoded)
        except json.JSONDecodeError as error:
            parser.error(f"invalid --service-version-command JSON: {error}")
        if (
            not isinstance(specification, list)
            or len(specification) < 2
            or not all(isinstance(item, str) and item for item in specification)
        ):
            parser.error(
                "--service-version-command must be a JSON string array of name and argv"
            )
        name, *service_command = specification
        if not re.fullmatch(r"[a-z0-9][a-z0-9_.-]{0,63}", name):
            parser.error(f"invalid service version name: {name}")
        if name in service_commands:
            parser.error(f"duplicate service version name: {name}")
        if any(
            any(control in item for control in ("\0", "\n", "\r"))
            for item in service_command
        ):
            parser.error(f"service version command contains control characters: {name}")
        service_commands[name] = service_command
    args.report = args.report.resolve()
    if args.report.suffix.lower() != ".json":
        parser.error("--report must use a .json suffix")
    try:
        args.report.relative_to(ROOT.resolve())
    except ValueError:
        pass
    else:
        parser.error("--report must be outside the repository")
    artifact_root = args.report.parent.resolve()
    generated_paths = []
    for path in args.generated_artifact:
        resolved = path.resolve()
        try:
            resolved.relative_to(artifact_root)
        except ValueError:
            parser.error("--generated-artifact must stay inside the report directory")
        if resolved.parent != artifact_root:
            parser.error("--generated-artifact must be directly inside the report directory")
        if resolved == args.report:
            parser.error("--generated-artifact cannot be the evidence manifest itself")
        generated_paths.append(resolved)

    commit_before = base_commit()
    diff_before = worktree_diff_sha256()
    service_versions = {
        name: service_version_record(service_command)
        for name, service_command in service_commands.items()
    }
    service_integrity_errors = [
        f"service version was not verified: {name}"
        for name, record in service_versions.items()
        if record["exit_code"] != 0 or not record["output"].strip()
    ]
    artifacts = {}
    tracked = git_modes()
    for name in args.artifact:
        try:
            path = repository_artifact_path(name)
        except ValueError as error:
            raise SystemExit(str(error)) from error
        if name not in tracked:
            raise SystemExit(f"artifact is not tracked: {name}")
        if not path.is_file():
            raise SystemExit(f"missing artifact: {name}")
        artifacts[name] = sha256_file(path)

    args.report.parent.mkdir(parents=True, exist_ok=True)
    command_text = shlex.join(command)
    try:
        result = subprocess.run(
            command,
            cwd=ROOT,
            capture_output=True,
        )
        exit_code = result.returncode
        stdout_bytes, stderr_bytes = result.stdout, result.stderr
    except OSError as error:
        exit_code = 127
        stdout_bytes, stderr_bytes = b"", (str(error) + "\n").encode("utf-8")

    raw_path = args.report.with_suffix(".log")
    junit_path = args.report.with_suffix(".junit.xml")
    sarif_path = args.report.with_suffix(".sarif")
    integrity_errors = list(service_integrity_errors)
    try:
        commit_after = base_commit()
    except (OSError, subprocess.SubprocessError) as error:
        commit_after = None
        integrity_errors.append(f"cannot read base commit after evidence command: {error}")
    if commit_after != commit_before:
        integrity_errors.append("base commit changed while the evidence command ran")
    github_sha = os.environ.get("GITHUB_SHA", commit_before)
    if github_sha != commit_before:
        integrity_errors.append("GITHUB_SHA does not match the checked-out commit")
    try:
        diff_after = worktree_diff_sha256()
    except (OSError, subprocess.SubprocessError) as error:
        diff_after = None
        integrity_errors.append(f"cannot read worktree after evidence command: {error}")
    if diff_after != diff_before:
        integrity_errors.append("worktree changed while the evidence command ran")
    for name, expected in artifacts.items():
        path = repository_artifact_path(name)
        if not path.is_file() or sha256_file(path) != expected:
            integrity_errors.append(f"artifact changed while the evidence command ran: {name}")
    reserved_names = {raw_path.name, junit_path.name, sarif_path.name}
    for path in generated_paths:
        if not path.is_file():
            integrity_errors.append(f"missing generated artifact: {path.name}")
        elif path.name in reserved_names:
            integrity_errors.append(f"duplicate generated artifact name: {path.name}")
        else:
            reserved_names.add(path.name)
    skip_markers = false_skip_markers(
        stdout_bytes.decode("utf-8", errors="replace"),
        stderr_bytes.decode("utf-8", errors="replace"),
    )
    if skip_markers:
        integrity_errors.append(
            "command emitted a deceptive skip marker instead of a harness-visible ignored or failed test: "
            + "; ".join(skip_markers)
        )
    if integrity_errors:
        stderr_bytes += (
            "\n".join(f"[evidence-integrity] {error}" for error in integrity_errors) + "\n"
        ).encode("utf-8")
        if exit_code == 0:
            exit_code = 86

    stdout = stdout_bytes.decode("utf-8", errors="replace")
    stderr = stderr_bytes.decode("utf-8", errors="replace")
    command_output = {
        "stdout_bytes": len(stdout_bytes),
        "stdout_sha256": hashlib.sha256(stdout_bytes).hexdigest(),
        "stderr_bytes": len(stderr_bytes),
        "stderr_sha256": hashlib.sha256(stderr_bytes).hexdigest(),
    }
    raw_path.write_bytes(
        f"$ {command_text}\n[stdout bytes={len(stdout_bytes)} sha256={command_output['stdout_sha256']}]\n".encode(
            "utf-8"
        )
        + stdout_bytes
        + f"\n[stderr bytes={len(stderr_bytes)} sha256={command_output['stderr_sha256']}]\n".encode(
            "utf-8"
        )
        + stderr_bytes
        + f"\n[exit_code]\n{exit_code}\n".encode("ascii")
    )
    write_junit(junit_path, command_text, exit_code, stdout, stderr)
    write_sarif(sarif_path, command_text, exit_code)

    tool_versions = current_tool_versions()
    environment = {
        "variables": environment_variables,
        "service_versions": service_versions,
    }
    generated_files = [raw_path, junit_path, sarif_path]
    generated_files.extend(path for path in generated_paths if path.is_file())
    generated = {path.name: sha256_file(path) for path in generated_files if path.is_file()}
    report_files = {
        path.name: {
            "path": path.name,
            "size_bytes": path.stat().st_size,
            "sha256": generated[path.name],
        }
        for path in generated_files
        if path.is_file()
    }
    status = "PASS" if exit_code == 0 else "FAIL"
    evidence = {
        "base_commit": commit_before,
        "worktree_diff_sha256": diff_before,
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "generator_version": EVIDENCE_GENERATOR_VERSION,
        "tool_versions": tool_versions,
        "environment": environment,
        "environment_sha256": evidence_environment_sha256(tool_versions, environment),
        "command": command_text,
        "command_argv": command,
        "command_output": command_output,
        "exit_code": exit_code,
        "artifact_sha256": artifacts,
        "generated_artifact_sha256": generated,
        "report_files": report_files,
        "repository": os.environ.get("GITHUB_REPOSITORY", repository_slug()),
        "github_sha": github_sha,
        "github_run_id": os.environ.get("GITHUB_RUN_ID", "LOCAL"),
        "github_run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT", "LOCAL"),
        "workflow": os.environ.get("GITHUB_WORKFLOW", "local"),
        "job": os.environ.get("GITHUB_JOB", "local"),
        "runner_os": os.environ.get("RUNNER_OS", platform.system()),
        "semantic_status": status,
        "status": status,
    }
    args.report.write_text(
        json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n"
    )
    sys.stdout.buffer.write(stdout_bytes)
    sys.stderr.buffer.write(stderr_bytes)
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
