#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import platform
import subprocess
from datetime import datetime, timezone
from pathlib import Path

from repo_truth import ROOT, base_commit, sha256_file, worktree_diff_sha256


def version(*command: str) -> str:
    try:
        result = subprocess.run(command, cwd=ROOT, text=True, encoding="utf-8", errors="replace", capture_output=True)
    except FileNotFoundError:
        return "NOT_VERIFIED"
    return (result.stdout or result.stderr).splitlines()[0] if result.returncode == 0 else "NOT_VERIFIED"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--command", required=True)
    parser.add_argument("--exit-code", type=int, required=True)
    parser.add_argument("--status", choices=("PASS", "FAIL", "BLOCKED", "NOT_VERIFIED"), required=True)
    parser.add_argument("--artifact", action="append", default=[])
    args = parser.parse_args()
    artifacts = {}
    for name in args.artifact:
        path = ROOT / name
        if not path.is_file():
            raise SystemExit(f"missing artifact: {name}")
        artifacts[name] = sha256_file(path)
    evidence = {
        "base_commit": base_commit(),
        "worktree_diff_sha256": worktree_diff_sha256(),
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "generator_version": "p00a-1",
        "tool_versions": {
            "platform": platform.platform(),
            "rustc": version("rustc", "--version"),
            "cargo": version("cargo", "--version"),
            "node": version("node", "--version"),
            "pnpm": version("pnpm.cmd" if os.name == "nt" else "pnpm", "--version"),
        },
        "command": args.command,
        "exit_code": args.exit_code,
        "artifact_sha256": artifacts,
        "status": args.status,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
