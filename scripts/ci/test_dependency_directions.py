#!/usr/bin/env python3
"""Negative contract test for the Cargo dependency direction gate."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("check_dependency_directions.py")


class DependencyDirectionGateTests(unittest.TestCase):
    def test_shared_kernel_cannot_depend_on_api(self) -> None:
        result = self.run_gate(
            '\n[dependencies]\ntrpg-api = { path = "../trpg-api" }\n'
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "forbidden dependency: trpg-shared-kernel -> trpg-api",
            result.stderr,
        )

    def test_target_dependency_alias_cannot_bypass_policy(self) -> None:
        result = self.run_gate(
            "\n[target.'cfg(unix)'.dependencies]\n"
            'api_alias = { package = "trpg-api", path = "../trpg-api" }\n'
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "forbidden dependency: trpg-shared-kernel -> trpg-api",
            result.stderr,
        )
        self.assertIn("via api_alias", result.stderr)

    def test_build_dependency_alias_cannot_bypass_policy(self) -> None:
        result = self.run_gate(
            '\n[build-dependencies]\n'
            'api_alias = { package = "trpg-api", path = "../trpg-api" }\n'
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "forbidden dependency: trpg-shared-kernel -> trpg-api",
            result.stderr,
        )

    def run_gate(self, shared_kernel_dependencies: str) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            self.write(
                root / "Cargo.toml",
                """[workspace]
members = [
  "crates/trpg-contracts",
  "crates/trpg-shared-kernel",
  "crates/trpg-api",
  "apps/api-server",
  "apps/realtime-server",
  "apps/agent-worker",
  "apps/admin-server",
  "apps/migration-runner",
]
resolver = "2"
""",
            )
            packages = {
                "crates/trpg-contracts": ("trpg-contracts", ""),
                "crates/trpg-shared-kernel": (
                    "trpg-shared-kernel",
                    shared_kernel_dependencies,
                ),
                "crates/trpg-api": ("trpg-api", ""),
                "apps/api-server": ("api-server", ""),
                "apps/realtime-server": ("realtime-server", ""),
                "apps/agent-worker": ("agent-worker", ""),
                "apps/admin-server": ("admin-server", ""),
                "apps/migration-runner": ("migration-runner", ""),
            }
            for member, (name, extra) in packages.items():
                self.write(
                    root / member / "Cargo.toml",
                    f'[package]\nname = "{name}"\nversion = "0.1.0"\n'
                    f'edition = "2021"\n{extra}',
                )
                if member.startswith("apps/"):
                    self.write(root / member / "src" / "main.rs", "fn main() {}\n")

            return subprocess.run(
                [sys.executable, str(SCRIPT), "--root", str(root)],
                check=False,
                capture_output=True,
                text=True,
            )

    @staticmethod
    def write(path: Path, content: str) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")


if __name__ == "__main__":
    unittest.main()
