#!/usr/bin/env python3
"""Reject human-maintained source files longer than the project limit."""

from __future__ import annotations

import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Iterable


ROOT = Path(__file__).resolve().parents[2]
MAX_SOURCE_LINES = 400
SOURCE_SUFFIXES = frozenset(
    {
        ".bash",
        ".c",
        ".cc",
        ".cjs",
        ".cpp",
        ".cs",
        ".css",
        ".go",
        ".h",
        ".hpp",
        ".html",
        ".java",
        ".js",
        ".jsx",
        ".kt",
        ".kts",
        ".mjs",
        ".php",
        ".ps1",
        ".py",
        ".rb",
        ".rego",
        ".rs",
        ".scss",
        ".sh",
        ".svelte",
        ".swift",
        ".ts",
        ".tsx",
        ".vue",
    }
)
EXCLUDED_DIRECTORIES = frozenset(
    {
        ".git",
        ".venv",
        "artifacts",
        "build",
        "dist",
        "docs",
        "fixtures",
        "generated",
        "migrations",
        "node_modules",
        "source-archive",
        "target",
        "vendor",
        "venv",
    }
)


@dataclass(frozen=True, order=True)
class SourceSizeViolation:
    path: PurePosixPath
    line_count: int


def is_human_maintained_source(path: PurePosixPath) -> bool:
    return (
        path.suffix.lower() in SOURCE_SUFFIXES
        and not any(part in EXCLUDED_DIRECTORIES for part in path.parts[:-1])
    )


def git_worktree_files(root: Path) -> list[PurePosixPath]:
    result = subprocess.run(
        [
            "git",
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
    )
    return sorted(
        PurePosixPath(os.fsdecode(raw_path))
        for raw_path in result.stdout.split(b"\0")
        if raw_path
    )


def physical_line_count(path: Path) -> int:
    return len(path.read_bytes().splitlines())


def source_size_violations(
    root: Path,
    paths: Iterable[PurePosixPath] | None = None,
    *,
    limit: int = MAX_SOURCE_LINES,
) -> list[SourceSizeViolation]:
    candidates = git_worktree_files(root) if paths is None else sorted(paths)
    violations = []
    for relative_path in candidates:
        if not is_human_maintained_source(relative_path):
            continue
        line_count = physical_line_count(root / relative_path)
        if line_count > limit:
            violations.append(SourceSizeViolation(relative_path, line_count))
    return violations


def checked_source_count(root: Path) -> int:
    return sum(is_human_maintained_source(path) for path in git_worktree_files(root))


def main() -> int:
    if sys.argv[1:] != ["--check"]:
        print("usage: check_source_file_size.py --check", file=sys.stderr)
        return 2
    try:
        violations = source_size_violations(ROOT)
        source_count = checked_source_count(ROOT)
    except (OSError, subprocess.CalledProcessError) as error:
        print(f"source-size check failed: {error}", file=sys.stderr)
        return 1
    if violations:
        for violation in violations:
            print(
                f"{violation.path}: {violation.line_count} lines "
                f"(limit {MAX_SOURCE_LINES})",
                file=sys.stderr,
            )
        return 1
    print(
        f"source-size contract verified: {source_count} files, "
        f"maximum {MAX_SOURCE_LINES} lines"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
