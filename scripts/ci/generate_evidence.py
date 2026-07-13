#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import platform
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
    canonical_json_sha256,
    current_tool_versions,
    git_modes,
    repository_artifact_path,
    repository_slug,
    sha256_file,
    worktree_diff_sha256,
)


def write_junit(path: Path, command: str, exit_code: int) -> None:
    suite = ET.Element(
        "testsuite",
        name="repository-evidence",
        tests="1",
        failures="0" if exit_code == 0 else "1",
    )
    case = ET.SubElement(suite, "testcase", classname="ci.command", name=command)
    if exit_code:
        ET.SubElement(case, "failure", message=f"command exited {exit_code}")
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
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("a command is required after --")
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
        if resolved == args.report:
            parser.error("--generated-artifact cannot be the evidence manifest itself")
        generated_paths.append(resolved)

    commit_before = base_commit()
    diff_before = worktree_diff_sha256()
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
            text=True,
            encoding="utf-8",
            errors="replace",
            capture_output=True,
        )
        exit_code, stdout, stderr = result.returncode, result.stdout, result.stderr
    except OSError as error:
        exit_code, stdout, stderr = 127, "", str(error) + "\n"

    raw_path = args.report.with_suffix(".log")
    junit_path = args.report.with_suffix(".junit.xml")
    sarif_path = args.report.with_suffix(".sarif")
    integrity_errors = []
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
    if integrity_errors:
        stderr += "\n".join(f"[evidence-integrity] {error}" for error in integrity_errors) + "\n"
        if exit_code == 0:
            exit_code = 86

    raw_path.write_text(
        f"$ {command_text}\n[stdout]\n{stdout}[stderr]\n{stderr}[exit_code]\n{exit_code}\n",
        encoding="utf-8",
        newline="\n",
    )
    write_junit(junit_path, command_text, exit_code)
    write_sarif(sarif_path, command_text, exit_code)

    tool_versions = current_tool_versions()
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
        "environment_sha256": canonical_json_sha256(tool_versions),
        "command": command_text,
        "command_argv": command,
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
    sys.stdout.write(stdout)
    sys.stderr.write(stderr)
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
