#!/usr/bin/env python3
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from manifest import render
from release_readiness import assess
from repo_truth import ROOT, compose_services, validate_evidence
from validate_workflows import validate as validate_workflows
from verify_test_inventory import inventory


class RepositoryTruthNegativeTests(unittest.TestCase):
    def test_historical_pass_evidence_is_rejected(self) -> None:
        legacy = ROOT / "evidence/stages/S09/docker-compose-smoke.txt"
        with self.assertRaises(json.JSONDecodeError):
            json.loads(legacy.read_text(encoding="utf-8"))
        self.assertTrue(validate_evidence({}))

    def test_static_http_200_placeholder_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            compose = Path(directory) / "compose.yml"
            compose.write_text(
                'services:\n  api:\n    image: nginx:alpine\n    command: ["sh", "-c", "printf \'{\\"status\\":\\"ok\\"}\'"]\n',
                encoding="utf-8",
            )
            self.assertTrue(compose_services(compose)["api"]["placeholder"])

    def test_release_readiness_is_blocked_without_product_runtime(self) -> None:
        report = assess(ROOT)
        self.assertEqual(report["status"], "BLOCKED")
        ids = {blocker["id"] for blocker in report["blockers"]}
        self.assertIn("NO_PRODUCT_BINARY", ids)
        self.assertIn("NO_PRODUCT_DOCKERFILE", ids)
        self.assertIn("PLACEHOLDER_SERVICE", ids)

    def test_missing_workflow_script_is_rejected_and_restored(self) -> None:
        path = ROOT / ".github/workflows/p00a-negative.yml"
        path.write_text(
            """name: negative
on:\n  workflow_dispatch:\npermissions:\n  contents: read
concurrency:\n  group: negative\n  cancel-in-progress: true
jobs:\n  negative:\n    runs-on: ubuntu-latest\n    timeout-minutes: 1
    steps:\n      - run: python3 scripts/ci/does-not-exist.py
""",
            encoding="utf-8",
        )
        try:
            self.assertTrue(any("does-not-exist.py" in error for error in validate_workflows()))
        finally:
            path.unlink()

    def test_modified_manifest_is_rejected(self) -> None:
        expected = render()
        with tempfile.NamedTemporaryFile("w", encoding="utf-8", delete=False) as handle:
            handle.write(expected + "tampered\n")
            path = Path(handle.name)
        try:
            result = subprocess.run(
                [sys.executable, "scripts/ci/verify_manifest.py", "--manifest", str(path)],
                cwd=ROOT,
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("manifest drift", result.stderr)
        finally:
            path.unlink()

    def test_unreferenced_fixture_is_rejected_and_restored(self) -> None:
        relative = "fixtures/" + "p00a-unreferenced-" + "negative.json.md"
        path = ROOT / relative
        path.write_text("```json\n{}\n```\n", encoding="utf-8")
        try:
            _, errors = inventory()
            self.assertIn(f"orphan fixture: {relative}", errors)
        finally:
            path.unlink()


if __name__ == "__main__":
    unittest.main()
