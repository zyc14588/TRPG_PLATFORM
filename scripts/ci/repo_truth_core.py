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

"""Repository discovery, hashing, and tool-version primitives."""
ROOT = Path(__file__).resolve().parents[2]
MANIFEST_OUTPUTS = {
    "MANIFEST.md",
    "manifests/CURRENT_PACKAGE_MANIFEST.md",
    "manifests/SELF_CONTAINED_PACKAGE_MANIFEST.md",
}
PRODUCT_SERVICES = ("web", "api", "realtime", "agent-worker", "admin")
EVIDENCE_SCHEMA_VERSION = "p00-7"
EVIDENCE_GENERATOR_VERSION = "p00-7"
EVIDENCE_REQUIRED = (
    "base_commit",
    "tree_sha",
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


def git_tree_sha(root: Path = ROOT) -> str:
    return run("git", "rev-parse", "HEAD^{tree}", root=root).stdout.strip()


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
