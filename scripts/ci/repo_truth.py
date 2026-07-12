#!/usr/bin/env python3
"""Shared, dependency-free repository truth helpers for P00A gates."""

from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MANIFEST_OUTPUTS = {
    "MANIFEST.md",
    "manifests/CURRENT_PACKAGE_MANIFEST.md",
    "manifests/SELF_CONTAINED_PACKAGE_MANIFEST.md",
}
PRODUCT_SERVICES = ("web", "api", "realtime", "agent-worker", "admin")


def run(*args: str, root: Path = ROOT, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args, cwd=root, check=check, text=True, encoding="utf-8", errors="replace", capture_output=True
    )


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


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def base_commit(root: Path = ROOT) -> str:
    return run("git", "rev-parse", "HEAD", root=root).stdout.strip()


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


def validate_evidence(data: dict, root: Path = ROOT) -> list[str]:
    required = {
        "base_commit",
        "worktree_diff_sha256",
        "generated_at_utc",
        "generator_version",
        "tool_versions",
        "command",
        "exit_code",
        "artifact_sha256",
        "status",
    }
    errors = [f"missing field: {name}" for name in sorted(required - data.keys())]
    if data.get("status") not in {"PASS", "FAIL", "BLOCKED", "NOT_VERIFIED"}:
        errors.append("invalid status")
    if data.get("base_commit") != base_commit(root):
        errors.append("base_commit mismatch")
    if not re.fullmatch(r"[0-9a-f]{64}", str(data.get("worktree_diff_sha256", ""))):
        errors.append("invalid worktree_diff_sha256")
    elif data.get("worktree_diff_sha256") != worktree_diff_sha256(root):
        errors.append("worktree_diff_sha256 mismatch")
    artifacts = data.get("artifact_sha256")
    if not isinstance(artifacts, dict):
        errors.append("artifact_sha256 must be an object")
    else:
        for name, expected in artifacts.items():
            path = root / name
            if not path.is_file() or sha256_file(path) != expected:
                errors.append(f"artifact hash mismatch: {name}")
    return errors
