#!/usr/bin/env python3
"""Negative contract tests for the P01 production source boundary gate."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("check_product_boundaries.py")


class ProductBoundaryGateTests(unittest.TestCase):
    def test_production_fixture_factory_is_rejected(self) -> None:
        result = self.run_gate(
            {
                "crates/trpg-shared-kernel/src/lib.rs": (
                    "fn fixture() { CommandEnvelope::governed(\"fixed\"); }\n"
                )
            }
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("fixed fixture command factory", result.stderr)

    def test_identical_production_modules_are_rejected(self) -> None:
        result = self.run_gate(
            {
                "crates/trpg-domain-core/src/first.rs": "pub fn value() -> u8 { 7 }\n",
                "crates/trpg-domain-core/src/second.rs": "pub fn value() -> u8 { 7 }\n",
            }
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("byte-identical production Rust modules", result.stderr)

    def test_batch_and_stage_fixture_metadata_are_rejected(self) -> None:
        result = self.run_gate(
            {
                "crates/trpg-api/src/lib.rs": (
                    'pub const STAGE_FIXTURE_ID: &str = "BATCH-999";\n'
                )
            }
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("construction prompt metadata", result.stderr)

    def test_unit_service_shell_is_rejected(self) -> None:
        result = self.run_gate(
            {"crates/trpg-api/src/lib.rs": "pub struct EmptyService;\n"}
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("empty Service/Repository", result.stderr)

    def test_unlisted_public_impl_module_is_rejected(self) -> None:
        result = self.run_gate(
            {
                "Cargo.toml": "[workspace]\nmembers = []\n",
                "crates/example/src/lib.rs": "pub mod surprise_impl;\n",
            }
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "unlisted public compatibility module: example:surprise_impl",
            result.stderr,
        )

    def run_gate(self, files: dict[str, str]) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            for relative_path, content in files.items():
                path = root / relative_path
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content, encoding="utf-8")
            return subprocess.run(
                [sys.executable, str(SCRIPT), "--root", str(root)],
                check=False,
                capture_output=True,
                text=True,
            )


if __name__ == "__main__":
    unittest.main()
