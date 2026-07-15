#!/usr/bin/env python3
"""Prove the reviewed P02 public package-boundary bypasses stay uncallable."""

from __future__ import annotations

import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "scripts/ci/fixtures/p02-boundary"

CASES = {
    "runtime_append.rs": "method `append` is private",
    "event_envelope_forge.rs": "field `integrity_hash` of struct `EventEnvelope` is private",
    "caller_authority_root.rs": "this function takes 1 argument but 2 arguments were supplied",
}


def manifest() -> str:
    return f"""\
[package]
name = "p02-boundary-regression"
version = "0.0.0"
edition = "2021"
publish = false

[workspace]

[dependencies]
trpg-agent-runtime = {{ path = {str(ROOT / 'crates/trpg-agent-runtime')!r} }}
trpg-runtime = {{ path = {str(ROOT / 'crates/trpg-runtime')!r} }}
trpg-shared-kernel = {{ path = {str(ROOT / 'crates/trpg-shared-kernel')!r} }}
trpg-test-support = {{ path = {str(ROOT / 'crates/trpg-contracts/test-support')!r} }}
"""


def cargo_check(project: Path, source: Path) -> subprocess.CompletedProcess[str]:
    (project / "src/main.rs").write_text(source.read_text(encoding="utf-8"), encoding="utf-8")
    return subprocess.run(
        ["cargo", "check", "--offline", "--quiet"],
        cwd=project,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="p02-boundary-") as temporary:
        project = Path(temporary)
        (project / "src").mkdir()
        (project / "Cargo.toml").write_text(manifest(), encoding="utf-8")
        for fixture, expected_error in CASES.items():
            result = cargo_check(project, FIXTURES / fixture)
            if result.returncode == 0:
                raise RuntimeError(f"P02 bypass unexpectedly compiled: {fixture}")
            if expected_error not in result.stderr:
                raise RuntimeError(
                    f"{fixture} failed for the wrong reason; expected {expected_error!r}:\n"
                    f"{result.stderr}"
                )
        control = cargo_check(project, FIXTURES / "control.rs")
        if control.returncode != 0:
            raise RuntimeError(f"P02 compile-fail harness control failed:\n{control.stderr}")
    print(f"P02 package-boundary regressions rejected: {len(CASES)}/{len(CASES)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
