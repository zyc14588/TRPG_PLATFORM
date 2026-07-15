#!/usr/bin/env python3
"""Prove the reviewed P02 public package-boundary bypasses stay uncallable."""

from __future__ import annotations

import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "scripts/ci/fixtures/p02-boundary"

CASES = {
    "canonical_store_custody.rs": "field `canonical_custody` of struct `ApiApplication` is private",
    "runtime_append.rs": "method `append` is private",
    "event_envelope_forge.rs": "fields `recorded_payload` and `integrity_hash` of struct `EventEnvelope` are private",
    "caller_authority_root.rs": "this function takes 1 argument but 2 arguments were supplied",
    "runtime_commit_without_auth.rs": "this function takes 6 arguments but 4 arguments were supplied",
    "runtime_replay_untrusted_scope.rs": "expected `&ReplayAuthorization`, found `&PrincipalScope`",
    "synthetic_formal_permit.rs": "no associated function or constant named `record_authorized_commit`",
    "caller_supplied_dice.rs": "expected `&ServerDiceRoll`, found `&DiceRollOutcome`",
    "agent_generic_store.rs": "no `EventStore` in the root",
    "caller_selected_canonical_event.rs": "function `append_coc7_event` is private",
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
api-server = {{ path = {str(ROOT / 'apps/api-server')!r} }}
trpg-agent-runtime = {{ path = {str(ROOT / 'crates/trpg-agent-runtime')!r} }}
trpg-runtime = {{ path = {str(ROOT / 'crates/trpg-runtime')!r} }}
trpg-ruleset-coc7 = {{ path = {str(ROOT / 'crates/trpg-ruleset-coc7')!r} }}
trpg-security-governance = {{ path = {str(ROOT / 'crates/trpg-security-governance')!r} }}
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
