#!/usr/bin/env python3
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from manifest import render
from release_readiness import assess, readiness_report_errors, release_evidence_errors
from repo_truth import (
    ROOT,
    canonical_json_sha256,
    compose_services,
    dependency_graph_errors,
    git_modes,
    production_source_errors,
    sha256_file,
    validate_evidence,
    workspace_dependency_errors,
)
from validate_workflows import validate as validate_workflows
from verify_evidence_schema import schema_errors
from verify_test_inventory import inventory


class RepositoryTruthNegativeTests(unittest.TestCase):
    def test_workspace_dependency_graph_matches_declared_layers(self) -> None:
        self.assertEqual(workspace_dependency_errors(), [])

    def test_outward_and_test_dependencies_are_rejected(self) -> None:
        metadata = {
            "packages": [
                {
                    "name": "trpg-contracts",
                    "dependencies": [{"name": "trpg-shared-kernel"}],
                },
                {
                    "name": "trpg-shared-kernel",
                    "dependencies": [{"name": "trpg-testing"}],
                },
                {"name": "trpg-testing", "dependencies": []},
            ]
        }
        errors = dependency_graph_errors(metadata)
        self.assertTrue(any("points outward" in error for error in errors))
        self.assertTrue(any("depends on test package" in error for error in errors))

    def test_production_dev_dependency_on_test_support_is_allowed(self) -> None:
        metadata = {
            "packages": [
                {
                    "name": "trpg-shared-kernel",
                    "dependencies": [{"name": "trpg-test-support", "kind": "dev"}],
                },
                {
                    "name": "trpg-test-support",
                    "dependencies": [{"name": "trpg-shared-kernel", "kind": None}],
                },
                {
                    "name": "trpg-testing",
                    "dependencies": [{"name": "trpg-test-support", "kind": None}],
                },
            ]
        }
        self.assertEqual(dependency_graph_errors(metadata), [])

    def test_production_normal_dependency_on_test_support_is_rejected(self) -> None:
        metadata = {
            "packages": [
                {
                    "name": "api-server",
                    "dependencies": [{"name": "trpg-shared-kernel", "kind": None}],
                },
                {
                    "name": "trpg-shared-kernel",
                    "dependencies": [{"name": "trpg-test-support", "kind": None}],
                },
                {"name": "trpg-test-support", "dependencies": []},
            ]
        }
        errors = dependency_graph_errors(metadata)
        self.assertTrue(any("outside dev-only test support" in error for error in errors))
        self.assertTrue(any("service/app normal dependency graph" in error for error in errors))

    def test_production_build_dependency_on_test_support_is_rejected(self) -> None:
        metadata = {
            "packages": [
                {
                    "name": "trpg-shared-kernel",
                    "dependencies": [{"name": "trpg-test-support", "kind": "build"}],
                },
                {"name": "trpg-test-support", "dependencies": []},
            ]
        }
        self.assertTrue(
            any(
                "outside dev-only test support" in error
                for error in dependency_graph_errors(metadata)
            )
        )

    def test_production_fixture_factories_are_rejected_but_test_support_is_excluded(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            production = root / "crates/trpg-shared-kernel/src/lib.rs"
            test_support = root / "crates/trpg-test-support/src/lib.rs"
            production.parent.mkdir(parents=True)
            test_support.parent.mkdir(parents=True)
            production.write_text(
                'CommandEnvelope::governed("command_001", "idem_001");\n', encoding="utf-8"
            )
            test_support.write_text(
                'CommandEnvelope::governed("command_001", "idem_001");\n', encoding="utf-8"
            )
            errors = production_source_errors(root)
        self.assertEqual(len(errors), 3)
        self.assertTrue(all("trpg-shared-kernel" in error for error in errors))

    def test_construction_metadata_and_duplicate_production_modules_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            first = root / "crates/trpg-runtime/src/first.rs"
            second = root / "apps/api-server/src/second.rs"
            first.parent.mkdir(parents=True)
            second.parent.mkdir(parents=True)
            source = 'pub const PROMPT_ID: &str = "CODEX-0000";\n'
            first.write_text(source, encoding="utf-8")
            second.write_text(source, encoding="utf-8")
            errors = production_source_errors(root)

        self.assertTrue(any("PROMPT_ID" in error for error in errors))
        self.assertTrue(any("CODEX-" in error for error in errors))
        self.assertTrue(any("byte-identical production modules" in error for error in errors))

    def test_documentary_construction_metadata_is_rejected_in_production_source(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            production = root / "crates/trpg-shared-kernel/src/lib.rs"
            production.parent.mkdir(parents=True)
            production.write_text(
                "\n".join(
                    (
                        "source_file: source,",
                        "test_file: test,",
                        "points_to_bootstrap_prompt",
                        "BootstrapPrompt",
                        "BatchPlan",
                        "per_file_prompts",
                        "FoundationDocument",
                        "SourceBundleGuide",
                        "NormalizedExecutionMap",
                        "SafeOutputMap",
                        "TokenRewriteTable",
                        "current_foundation_document_set",
                        "points_to_top_level_design",
                        "points_to_normalized_maps",
                        "states_historical_inputs_are_provenance_only",
                        "crate::workspace_and_governance::GovernanceContract {",
                    )
                ),
                encoding="utf-8",
            )
            errors = production_source_errors(root)

        for token in (
            "source_file:",
            "test_file:",
            "points_to_bootstrap_prompt",
            "BootstrapPrompt",
            "BatchPlan",
            "per_file_prompts",
            "FoundationDocument",
            "SourceBundleGuide",
            "NormalizedExecutionMap",
            "SafeOutputMap",
            "TokenRewriteTable",
            "current_foundation_document_set",
            "points_to_top_level_design",
            "points_to_normalized_maps",
            "states_historical_inputs_are_provenance_only",
        ):
            self.assertTrue(any(error.endswith(token) for error in errors), token)
        self.assertTrue(
            any("bypasses the canonical constructor" in error for error in errors)
        )

    def test_workspace_dependency_cycle_is_rejected(self) -> None:
        metadata = {
            "packages": [
                {
                    "name": "trpg-shared-kernel",
                    "dependencies": [{"name": "trpg-domain-core"}],
                },
                {
                    "name": "trpg-domain-core",
                    "dependencies": [{"name": "trpg-shared-kernel"}],
                },
            ]
        }
        self.assertTrue(
            any("dependency cycle" in error for error in dependency_graph_errors(metadata))
        )

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

    def test_release_readiness_recognizes_product_runtime_and_blocks_on_deployment(self) -> None:
        report = assess(ROOT)
        self.assertEqual(report["status"], "BLOCKED")
        ids = {blocker["id"] for blocker in report["blockers"]}
        self.assertTrue({"AUD-002", "AUD-006"}.issubset(ids))
        self.assertNotIn("AUD-001", ids)
        self.assertNotIn("NO_PRODUCT_BINARY", ids)
        self.assertIn("NO_PRODUCT_DOCKERFILE", ids)
        self.assertIn("PLACEHOLDER_SERVICE", ids)
        self.assertEqual(readiness_report_errors(report), [])
        del report["base_commit"]
        self.assertIn("release readiness base_commit mismatch", readiness_report_errors(report))

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
            self.assertEqual(payload["semantic_status"], "FAIL")
            self.assertEqual(payload["github_sha"], payload["base_commit"])
            self.assertEqual(set(payload["report_files"]), set(payload["generated_artifact_sha256"]))
            self.assertEqual(validate_evidence(payload, artifact_base=report.parent), [])
            live = {
                "GITHUB_REPOSITORY": payload["repository"],
                "GITHUB_SHA": payload["github_sha"],
                "GITHUB_RUN_ID": "123",
                "GITHUB_RUN_ATTEMPT": "1",
                "GITHUB_WORKFLOW": "test",
                "GITHUB_JOB": "test",
                "RUNNER_OS": payload["runner_os"],
            }
            payload.update(
                github_run_id="123",
                github_run_attempt="1",
                workflow="test",
                job="test",
            )
            with patch.dict("os.environ", live, clear=False):
                self.assertEqual(
                    validate_evidence(payload, artifact_base=report.parent, live_context=True), []
                )
                payload["github_run_id"] = "124"
                self.assertIn(
                    "github_run_id does not match live GitHub context",
                    validate_evidence(payload, artifact_base=report.parent, live_context=True),
                )
            payload["github_run_id"] = "LOCAL"
            payload["github_run_attempt"] = "LOCAL"
            payload["workflow"] = "local"
            payload["job"] = "local"
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
                "expected exactly one raw output bound to command and exit_code",
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
                "report_files do not match generated artifacts",
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
