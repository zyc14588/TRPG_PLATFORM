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
from repo_truth_evidence import validate_evidence

"""Repository-level release truth aggregation and command entrypoint."""
def decision_values(root: Path = ROOT) -> dict[str, str]:
    path = root / "docs/governance/P00_REMOTE_CANONICALIZATION_DECISION.md"
    if not path.is_file():
        raise ValueError("missing docs/governance/P00_REMOTE_CANONICALIZATION_DECISION.md")
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
