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


ROOT = Path(__file__).resolve().parents[2]
MANIFEST_OUTPUTS = {
    "MANIFEST.md",
    "manifests/CURRENT_PACKAGE_MANIFEST.md",
    "manifests/SELF_CONTAINED_PACKAGE_MANIFEST.md",
}
PRODUCT_SERVICES = ("web", "api", "realtime", "agent-worker", "admin")
EVIDENCE_SCHEMA_VERSION = "p00-6"
EVIDENCE_GENERATOR_VERSION = "p00-6"
EVIDENCE_REQUIRED = (
    "base_commit",
    "worktree_diff_sha256",
    "generated_at_utc",
    "generator_version",
    "tool_versions",
    "environment",
    "environment_sha256",
    "command",
    "command_argv",
    "command_output",
    "exit_code",
    "artifact_sha256",
    "command_artifact_sha256",
    "generated_artifact_sha256",
    "report_files",
    "repository",
    "github_sha",
    "github_run_id",
    "github_run_attempt",
    "workflow",
    "job",
    "runner_os",
    "semantic_status",
    "status",
)
EVIDENCE_STATUSES = ("PASS", "FAIL")
OPENFGA_VERSION_WITH_LOG_TIMESTAMP = re.compile(
    r"^\d{4}/\d{2}/\d{2} \d{2}:\d{2}:\d{2} "
    r"(OpenFGA version `[^`\r\n]+` build from `[^`\r\n]+` on `[^`\r\n]+`)$"
)


def run(*args: str, root: Path = ROOT, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args, cwd=root, check=check, text=True, encoding="utf-8", errors="replace", capture_output=True
    )


@lru_cache(maxsize=None)
def command_version(*command: str, root: Path = ROOT) -> str:
    try:
        result = subprocess.run(
            command,
            cwd=root,
            text=True,
            encoding="utf-8",
            errors="replace",
            capture_output=True,
        )
    except OSError:
        return "NOT_VERIFIED"
    return (result.stdout or result.stderr).splitlines()[0] if result.returncode == 0 else "NOT_VERIFIED"


def service_version_record(command: list[str], root: Path = ROOT) -> dict:
    try:
        result = subprocess.run(
            command,
            cwd=root,
            text=True,
            encoding="utf-8",
            errors="replace",
            capture_output=True,
        )
        exit_code = result.returncode
        output = stable_service_version_output(
            command, (result.stdout + result.stderr).strip()
        )
    except OSError as error:
        exit_code = 127
        output = str(error)
    return {
        "command": shlex.join(command),
        "command_argv": command,
        "exit_code": exit_code,
        "output": output,
    }


def stable_service_version_output(command: list[str], output: str) -> str:
    """Remove only a tool-owned nondeterministic prefix from version output."""
    if command[-2:] != ["/openfga", "version"]:
        return output
    match = OPENFGA_VERSION_WITH_LOG_TIMESTAMP.fullmatch(output)
    return match.group(1) if match is not None else output


def current_tool_versions(root: Path = ROOT) -> dict[str, str]:
    return {
        "platform": platform.platform(),
        "python": platform.python_version(),
        "rustc": command_version("rustc", "--version", root=root),
        "cargo": command_version("cargo", "--version", root=root),
        "node": command_version("node", "--version", root=root),
        "npm": command_version("npm.cmd" if os.name == "nt" else "npm", "--version", root=root),
        "pnpm": command_version(
            "pnpm.cmd" if os.name == "nt" else "pnpm", "--version", root=root
        ),
    }


def git_files(root: Path = ROOT) -> list[str]:
    result = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=root,
        check=True,
        capture_output=True,
    )
    return sorted(path.decode("utf-8") for path in result.stdout.split(b"\0") if path)


def git_mode(path: str, root: Path = ROOT) -> str:
    result = run("git", "ls-files", "-s", "--", path, root=root)
    if result.stdout.strip():
        return result.stdout.split()[0]
    return "100755" if os.name != "nt" and os.access(root / path, os.X_OK) else "100644"


def git_modes(root: Path = ROOT) -> dict[str, str]:
    result = subprocess.run(
        ["git", "ls-files", "-s", "-z"], cwd=root, check=True, capture_output=True
    ).stdout
    modes = {}
    for entry in result.split(b"\0"):
        if not entry:
            continue
        metadata, path = entry.split(b"\t", 1)
        modes[path.decode("utf-8")] = metadata.split(b" ", 1)[0].decode("ascii")
    return modes


def git_blob_bytes(root: Path = ROOT) -> dict[str, bytes]:
    index = subprocess.run(
        ["git", "ls-files", "-s", "-z"], cwd=root, check=True, capture_output=True
    ).stdout
    paths = {}
    for entry in index.split(b"\0"):
        if not entry:
            continue
        metadata, path = entry.split(b"\t", 1)
        _, object_id, stage = metadata.split()
        if stage != b"0":
            raise RuntimeError("manifest generation requires an unconflicted Git index")
        paths[path.decode("utf-8")] = object_id.decode("ascii")

    object_ids = sorted(set(paths.values()))
    output = subprocess.run(
        ["git", "cat-file", "--batch"],
        cwd=root,
        check=True,
        input=("\n".join(object_ids) + "\n").encode("ascii"),
        capture_output=True,
    ).stdout
    blobs = {}
    offset = 0
    for object_id in object_ids:
        header_end = output.index(b"\n", offset)
        _, object_type, size = output[offset:header_end].split()
        if object_type != b"blob":
            raise RuntimeError(f"manifest object is not a blob: {object_id}")
        offset = header_end + 1
        size = int(size)
        blobs[object_id] = output[offset : offset + size]
        offset += size + 1
    return {path: blobs[object_id] for path, object_id in paths.items()}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_json_sha256(value: object) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def evidence_environment_sha256(tool_versions: dict, environment: dict) -> str:
    return canonical_json_sha256(
        {"tool_versions": tool_versions, "environment": environment}
    )


def cargo_test_cases(*outputs: str) -> list[tuple[str, str]]:
    cases = []
    pattern = re.compile(r"(?m)^test (.+?) \.\.\. (ok|FAILED|ignored)$")
    for output in outputs:
        cases.extend((match.group(1), match.group(2)) for match in pattern.finditer(output))
    return cases


def false_skip_markers(*outputs: str) -> list[str]:
    pattern = re.compile(
        r"(?im)^[ \t]*(?:skip(?:ped|ping)?|not[ \t_-]+(?:run|executed))"
        r"(?:[ \t]*(?::|-)[ \t]*.*)?[ \t]*$"
    )
    return [
        match.group(0).strip()
        for output in outputs
        for match in pattern.finditer(output)
    ]


def evidence_test_cases(
    command: str, exit_code: int, stdout: str, stderr: str
) -> list[tuple[str, str]]:
    cases = cargo_test_cases(stdout, stderr)
    if not cases:
        return [(command, "ok" if exit_code == 0 else "FAILED")]
    if exit_code != 0 and not any(status == "FAILED" for _, status in cases):
        cases.append((command, "FAILED"))
    return cases


def parse_bound_raw_output(
    raw: bytes, command: str
) -> tuple[bytes, bytes, int] | None:
    prefix = f"$ {command}\n[stdout bytes=".encode("utf-8")
    if not raw.startswith(prefix):
        return None
    cursor = len(prefix)
    header_end = raw.find(b"]\n", cursor)
    if header_end < 0:
        return None
    stdout_header = raw[cursor:header_end].decode("ascii", errors="replace")
    stdout_match = re.fullmatch(r"([0-9]+) sha256=([0-9a-f]{64})", stdout_header)
    if stdout_match is None:
        return None
    cursor = header_end + 2
    stdout_length = int(stdout_match.group(1))
    stdout = raw[cursor : cursor + stdout_length]
    if len(stdout) != stdout_length or hashlib.sha256(stdout).hexdigest() != stdout_match.group(2):
        return None
    cursor += stdout_length

    stderr_prefix = b"\n[stderr bytes="
    if raw[cursor : cursor + len(stderr_prefix)] != stderr_prefix:
        return None
    cursor += len(stderr_prefix)
    header_end = raw.find(b"]\n", cursor)
    if header_end < 0:
        return None
    stderr_header = raw[cursor:header_end].decode("ascii", errors="replace")
    stderr_match = re.fullmatch(r"([0-9]+) sha256=([0-9a-f]{64})", stderr_header)
    if stderr_match is None:
        return None
    cursor = header_end + 2
    stderr_length = int(stderr_match.group(1))
    stderr = raw[cursor : cursor + stderr_length]
    if len(stderr) != stderr_length or hashlib.sha256(stderr).hexdigest() != stderr_match.group(2):
        return None
    cursor += stderr_length

    exit_prefix = b"\n[exit_code]\n"
    if raw[cursor : cursor + len(exit_prefix)] != exit_prefix:
        return None
    try:
        exit_code = int(raw[cursor + len(exit_prefix) :].decode("ascii").strip())
    except ValueError:
        return None
    if raw[cursor + len(exit_prefix) :] != f"{exit_code}\n".encode("ascii"):
        return None
    return stdout, stderr, exit_code


def repository_artifact_path(name: str, root: Path = ROOT) -> Path:
    relative = Path(name)
    if relative.is_absolute() or ".." in relative.parts or relative.as_posix() != name:
        raise ValueError(f"artifact path must be repository-relative: {name}")
    path = (root / relative).resolve()
    try:
        path.relative_to(root.resolve())
    except ValueError as error:
        raise ValueError(f"artifact path escapes repository: {name}") from error
    return path


def base_commit(root: Path = ROOT) -> str:
    return run("git", "rev-parse", "HEAD", root=root).stdout.strip()


def repository_slug(root: Path = ROOT) -> str:
    remote = run("git", "remote", "get-url", "origin", root=root).stdout.strip()
    match = re.search(r"github\.com[/:]([^/]+/[^/]+?)(?:\.git)?$", remote)
    if not match:
        raise ValueError(f"origin is not a GitHub repository: {remote}")
    return match.group(1)


def worktree_diff_sha256(root: Path = ROOT) -> str:
    digest = hashlib.sha256()
    diff = subprocess.run(
        ["git", "diff", "--binary", "HEAD"], cwd=root, check=True, capture_output=True
    ).stdout
    digest.update(diff)
    tracked = set(run("git", "ls-files", root=root).stdout.splitlines())
    for path in (path for path in git_files(root) if path not in tracked):
        digest.update(path.encode("utf-8") + b"\0")
        digest.update((root / path).read_bytes())
    return digest.hexdigest()


def cargo_metadata(root: Path = ROOT) -> dict:
    return json.loads(
        run("cargo", "metadata", "--no-deps", "--format-version", "1", root=root).stdout
    )


def cargo_targets(root: Path = ROOT, kind: str | None = None) -> set[str]:
    targets = set()
    for package in cargo_metadata(root)["packages"]:
        for target in package["targets"]:
            if kind is None or kind in target["kind"]:
                targets.add(target["name"])
    return targets


def compose_services(
    path: Path, _seen: set[Path] | None = None
) -> dict[str, dict[str, object]]:
    path = path.resolve()
    seen = set() if _seen is None else _seen
    if path in seen:
        return {}
    seen.add(path)
    text = path.read_text(encoding="utf-8")
    services: dict[str, dict[str, object]] = {}
    for include in re.finditer(
        r"(?m)^\s*-\s+path:\s*['\"]?([^'\"\s#]+)['\"]?\s*$", text
    ):
        services.update(compose_services(path.parent / include.group(1), seen))
    matches = list(re.finditer(r"(?m)^  ([a-zA-Z0-9_-]+):\s*$", text))
    for index, match in enumerate(matches):
        end = matches[index + 1].start() if index + 1 < len(matches) else len(text)
        block = text[match.end() : end]
        image_match = re.search(r"(?m)^    image:\s*([^\s#]+)", block)
        has_build = bool(re.search(r"(?m)^    build:\s*", block))
        labelled = bool(
            re.search(r"coc_ai_trpg\.placeholder:\s*['\"]?true['\"]?", block, re.I)
        )
        static_nginx = bool(image_match and image_match.group(1).startswith("nginx")) and bool(
            re.search(r"printf|status\\?\"?:?\\?\"?ok|not_implemented", block, re.I)
        )
        services[match.group(1)] = {
            "image": image_match.group(1) if image_match else None,
            "build": has_build,
            "placeholder": labelled or static_nginx,
            "block": block,
        }
    return services


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


def decision_values(root: Path = ROOT) -> dict[str, str]:
    path = root / "P00_REMOTE_CANONICALIZATION_DECISION.md"
    if not path.is_file():
        raise ValueError("missing P00_REMOTE_CANONICALIZATION_DECISION.md")
    return dict(
        match.groups()
        for match in re.finditer(r"(?m)^([A-Z0-9_]+)\s*=\s*(.+?)\s*$", path.read_text(encoding="utf-8"))
    )


def repository_truth_errors(root: Path = ROOT) -> list[str]:
    errors = []
    try:
        decision = decision_values(root)
        if decision.get("OWNER_APPROVAL") != "APPROVED":
            errors.append("canonical repository owner approval is not APPROVED")
        if decision.get("CANONICAL_REPOSITORY") != repository_slug(root):
            errors.append("canonical repository does not match origin")
        branch = (
            os.environ.get("GITHUB_BASE_REF")
            or os.environ.get("GITHUB_REF_NAME")
            or run("git", "branch", "--show-current", root=root).stdout.strip()
        )
        if decision.get("CANONICAL_BRANCH") != branch:
            errors.append("canonical branch does not match current branch")
    except (OSError, subprocess.SubprocessError, ValueError) as error:
        errors.append(str(error))
    diff = subprocess.run(
        ["git", "diff", "--check"], cwd=root, text=True, encoding="utf-8", capture_output=True
    )
    if diff.returncode:
        errors.append(diff.stdout.strip() or diff.stderr.strip() or "git diff --check failed")
    if run("git", "status", "--porcelain=v1", root=root).stdout.strip():
        errors.append("worktree is not clean")
    return errors


def main() -> int:
    if sys.argv[1:] != ["--check"]:
        print("usage: repo_truth.py --check", file=sys.stderr)
        return 2
    errors = repository_truth_errors()
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"repository truth verified: {repository_slug()} {base_commit()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
