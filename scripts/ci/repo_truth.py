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
EVIDENCE_SCHEMA_VERSION = "p00-3"
EVIDENCE_GENERATOR_VERSION = "p00-3"
EVIDENCE_REQUIRED = (
    "base_commit",
    "worktree_diff_sha256",
    "generated_at_utc",
    "generator_version",
    "tool_versions",
    "environment_sha256",
    "command",
    "command_argv",
    "exit_code",
    "artifact_sha256",
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


def compose_services(path: Path) -> dict[str, dict[str, object]]:
    text = path.read_text(encoding="utf-8")
    services: dict[str, dict[str, object]] = {}
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
    elif data.get("environment_sha256") != canonical_json_sha256(tool_versions):
        errors.append("environment_sha256 mismatch")
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
        generated_by_suffix = {
            suffix: [name for name in generated if name.endswith(suffix)]
            for suffix in (".log", ".junit.xml", ".sarif")
        }
        for suffix, names in generated_by_suffix.items():
            if not names:
                errors.append(f"missing generated artifact: *{suffix}")
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
        if artifact_base is not None:
            bound_logs = []
            for name in generated_by_suffix[".log"]:
                raw_path = artifact_base / name
                if raw_path.is_file():
                    raw = raw_path.read_text(encoding="utf-8")
                    if raw.startswith(f"$ {data.get('command', '')}\n") and raw.endswith(
                        f"[exit_code]\n{data.get('exit_code')}\n"
                    ):
                        bound_logs.append(name)
            if len(bound_logs) != 1:
                errors.append("expected exactly one raw output bound to command and exit_code")
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
