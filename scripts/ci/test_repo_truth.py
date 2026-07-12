#!/usr/bin/env python3
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from manifest import render
from release_readiness import assess, release_evidence_errors
from repo_truth import (
    ROOT,
    canonical_json_sha256,
    compose_services,
    git_modes,
    sha256_file,
    validate_evidence,
)
from validate_workflows import validate as validate_workflows
from verify_evidence_schema import schema_errors
from verify_test_inventory import inventory


class RepositoryTruthNegativeTests(unittest.TestCase):
    def test_historical_pass_evidence_is_rejected(self) -> None:
        legacy = ROOT / "evidence/stages/S09/docker-compose-smoke.txt"
        with self.assertRaises(json.JSONDecodeError):
            json.loads(legacy.read_text(encoding="utf-8"))
        self.assertTrue(validate_evidence({}))

    def test_evidence_schema_drift_is_rejected(self) -> None:
        schema = json.loads(
            (ROOT / "scripts/ci/evidence.schema.json").read_text(encoding="utf-8")
        )
        self.assertEqual(schema_errors(schema), [])
        schema["statuses"] = ["PASS"]
        self.assertEqual(schema_errors(schema), ["evidence schema definition drift"])

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

    def test_manifest_path_set_matches_git(self) -> None:
        paths = [
            line.split("`", 2)[1]
            for line in render().splitlines()
            if line.startswith("| `")
        ]
        self.assertEqual(paths, sorted(git_modes()))

    def test_evidence_generator_executes_command_and_derives_failure(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory) / "negative.json"
            result = subprocess.run(
                [
                    sys.executable,
                    "scripts/ci/generate_evidence.py",
                    "--report",
                    str(report),
                    "--artifact",
                    "MANIFEST.md",
                    "--",
                    sys.executable,
                    "-c",
                    "raise SystemExit(7)",
                ],
                cwd=ROOT,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 7)
            payload = json.loads(report.read_text(encoding="utf-8"))
            actual_command = payload["command"]
            actual_argv = payload["command_argv"]
            self.assertEqual(payload["exit_code"], 7)
            self.assertEqual(payload["status"], "FAIL")
            self.assertEqual(validate_evidence(payload, artifact_base=report.parent), [])
            release_errors = release_evidence_errors(payload, ROOT, report.parent)
            self.assertIn("release evidence must record a passing command", release_errors)
            self.assertIn(
                "release evidence must execute bash scripts/ci/test-all.sh", release_errors
            )
            payload["status"] = "PASS"
            payload["exit_code"] = False
            self.assertIn(
                "exit_code must be an integer",
                validate_evidence(payload, artifact_base=report.parent),
            )
            payload["exit_code"] = 7
            self.assertIn(
                "status does not match exit_code",
                validate_evidence(payload, artifact_base=report.parent),
            )
            payload["status"] = "FAIL"
            payload["command"] = "cargo test"
            payload["command_argv"] = ["cargo", "test"]
            self.assertIn(
                "raw output command mismatch",
                validate_evidence(payload, artifact_base=report.parent),
            )
            payload["command"] = actual_command
            payload["command_argv"] = actual_argv
            digest = payload["artifact_sha256"].pop("MANIFEST.md")
            payload["artifact_sha256"][str((ROOT / "MANIFEST.md").resolve())] = digest
            self.assertTrue(
                any(
                    "artifact path must be repository-relative" in error
                    for error in validate_evidence(payload, artifact_base=report.parent)
                )
            )
            payload["artifact_sha256"] = {"MANIFEST.md": digest}
            extra_log = report.parent / "extra.log"
            extra_log.write_text("forged\n", encoding="utf-8")
            payload["generated_artifact_sha256"][extra_log.name] = sha256_file(extra_log)
            self.assertIn(
                "expected exactly one generated artifact: *.log",
                validate_evidence(payload, artifact_base=report.parent),
            )
            payload["generated_artifact_sha256"].pop(extra_log.name)
            payload["tool_versions"]["pnpm"] = "NOT_VERIFIED"
            payload["environment_sha256"] = canonical_json_sha256(payload["tool_versions"])
            self.assertIn(
                "tool version not verified: pnpm",
                validate_evidence(payload, artifact_base=report.parent),
            )

    def test_evidence_generator_rejects_command_worktree_mutation(self) -> None:
        relative = "p00-evidence-mutation.tmp"
        mutation = ROOT / relative
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory) / "mutation.json"
            try:
                result = subprocess.run(
                    [
                        sys.executable,
                        "scripts/ci/generate_evidence.py",
                        "--report",
                        str(report),
                        "--artifact",
                        "MANIFEST.md",
                        "--",
                        sys.executable,
                        "-c",
                        (
                            "from pathlib import Path; "
                            f"Path({str(mutation)!r}).write_text('mutation', encoding='utf-8')"
                        ),
                    ],
                    cwd=ROOT,
                    capture_output=True,
                    text=True,
                )
                self.assertEqual(result.returncode, 86)
                payload = json.loads(report.read_text(encoding="utf-8"))
                self.assertEqual(payload["status"], "FAIL")
                self.assertEqual(payload["exit_code"], 86)
                self.assertIn("worktree changed", report.with_suffix(".log").read_text())
            finally:
                mutation.unlink(missing_ok=True)

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
