#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path
from unittest.mock import patch

from manifest import manifest_source_errors, render
from release_readiness import (
    REQUIRED_RELEASE_TEST_CASES,
    assess,
    readiness_report_errors,
    release_evidence_errors,
    release_junit_errors,
)
from repo_truth import (
    ROOT,
    canonical_json_sha256,
    compose_services,
    false_skip_markers,
    git_modes,
    sha256_file,
    validate_evidence,
)
from validate_workflows import validate as validate_workflows
from verify_evidence_schema import schema_errors
from verify_manifest import HASHED_ROW, manifest_count_errors
from verify_test_inventory import (
    _blank_rust_non_code,
    integration_test_silent_env_successes,
    inventory,
)


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

    def test_release_readiness_recognizes_product_entries_but_keeps_later_blockers(self) -> None:
        report = assess(ROOT)
        self.assertEqual(report["status"], "BLOCKED")
        ids = {blocker["id"] for blocker in report["blockers"]}
        self.assertNotIn("AUD-002", ids)
        self.assertNotIn("AUD-006", ids)
        self.assertNotIn("AUD-001", ids)
        self.assertNotIn("MISSING_PRODUCT_BINARY", ids)
        self.assertNotIn("MISSING_WEB_ENTRYPOINT", ids)
        self.assertNotIn("MISSING_WEB_SCRIPT", ids)
        self.assertNotIn("NO_PRODUCT_DOCKERFILE", ids)
        self.assertNotIn("PLACEHOLDER_SERVICE", ids)
        self.assertNotIn("MUTABLE_PRODUCT_IMAGE", ids)
        self.assertIn("MISSING_CURRENT_EVIDENCE", ids)
        self.assertIn("MISSING_PRODUCTION_SECURITY_EVIDENCE", ids)
        self.assertEqual(readiness_report_errors(report), [])
        del report["base_commit"]
        self.assertIn("release readiness base_commit mismatch", readiness_report_errors(report))

    def test_release_junit_recurses_and_rejects_error_or_skip_as_passing(self) -> None:
        root = ET.Element("testsuites")
        nested = ET.SubElement(root, "testsuites")
        suite = ET.SubElement(nested, "testsuite")
        cases = {
            name: ET.SubElement(suite, "testcase", name=name)
            for name in REQUIRED_RELEASE_TEST_CASES
        }
        self.assertEqual(release_junit_errors(root), [])

        errored_name, skipped_name = sorted(REQUIRED_RELEASE_TEST_CASES)[:2]
        ET.SubElement(cases[errored_name], "error")
        ET.SubElement(cases[skipped_name], "skipped")
        errors = release_junit_errors(root)
        self.assertIn(
            f"release evidence is missing required passing test: {errored_name}",
            errors,
        )
        self.assertIn(
            f"release evidence is missing required passing test: {skipped_name}",
            errors,
        )
        self.assertIn("release evidence must not contain ignored tests", errors)

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
        content = render()
        paths = [
            line.split("`", 2)[1]
            for line in content.splitlines()
            if line.startswith("| `")
        ]
        self.assertEqual(paths, sorted(git_modes()))
        self.assertEqual(manifest_count_errors(content), [])

        repository_header = next(
            line for line in content.splitlines() if line.startswith("Repository files: ")
        )
        tampered_header = content.replace(
            repository_header, "Repository files: 0", 1
        )
        self.assertTrue(
            any(
                "repository file count does not match manifest path rows" in error
                for error in manifest_count_errors(tampered_header)
            )
        )

        hashed_row = next(
            line for line in content.splitlines() if HASHED_ROW.fullmatch(line)
        )
        missing_row = content.replace(hashed_row + "\n", "", 1)
        missing_row_errors = manifest_count_errors(missing_row)
        self.assertTrue(
            any(
                "repository file count does not match manifest path rows" in error
                for error in missing_row_errors
            )
        )
        self.assertTrue(
            any(
                "hashed file count does not match hashed path rows" in error
                for error in missing_row_errors
            )
        )

    def test_manifest_rejects_unstaged_and_untracked_source_changes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "--quiet"], cwd=root, check=True)
            tracked = root / "tracked.txt"
            tracked.write_text("committed shape\n", encoding="utf-8")
            subprocess.run(["git", "add", "tracked.txt"], cwd=root, check=True)
            tracked.write_text("unstaged shape\n", encoding="utf-8")
            (root / "untracked.txt").write_text("not indexed\n", encoding="utf-8")
            self.assertEqual(
                manifest_source_errors(root),
                ["tracked.txt", "untracked.txt"],
            )

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
            payload["environment_sha256"] = canonical_json_sha256(
                {
                    "tool_versions": payload["tool_versions"],
                    "environment": payload["environment"],
                }
            )
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

    def test_evidence_generator_rejects_deceptive_skip_output(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report = root / "false-skip.json"
            shim_dir = root / "bin"
            shim_dir.mkdir()
            for name, version in (
                ("node", "v24.17.0"),
                ("npm", "11.9.0"),
                ("pnpm", "11.9.0"),
            ):
                shim = shim_dir / name
                shim.write_text(
                    f"#!/usr/bin/env sh\nprintf '%s\\n' '{version}'\n",
                    encoding="utf-8",
                )
                shim.chmod(0o755)
            environment = dict(os.environ)
            environment["PATH"] = str(shim_dir) + os.pathsep + environment["PATH"]
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
                        "print('test integration::requires_service ... ok'); "
                        "print('SKIPPING: service configuration is absent'); "
                        "print('not executed: dependency is absent')"
                    ),
                ],
                cwd=ROOT,
                env=environment,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 86)
            payload = json.loads(report.read_text(encoding="utf-8"))
            self.assertEqual(payload["status"], "FAIL")
            self.assertEqual(payload["exit_code"], 86)
            self.assertIn(
                "deceptive skip marker",
                report.with_suffix(".log").read_text(encoding="utf-8"),
            )
            self.assertEqual(validate_evidence(payload, artifact_base=report.parent), [])

    def test_false_skip_markers_include_bare_status_without_crossing_lines(self) -> None:
        output = "\n".join(
            (
                "SKIPPED",
                "not run",
                "NOT_EXECUTED",
                "skipping: service configuration is absent",
                "not",
                "executed: split across lines must not be joined",
                "ordinary test output",
            )
        )
        self.assertEqual(
            false_skip_markers(output),
            [
                "SKIPPED",
                "not run",
                "NOT_EXECUTED",
                "skipping: service configuration is absent",
            ],
        )

    def test_inventory_rejects_environment_driven_early_return(self) -> None:
        path = ROOT / "crates/trpg-identity/tests/p00-false-skip-negative.rs"
        path.write_text(
            """
#[test]
fn false_green() {
    let Ok(_value) = std::env::var("REQUIRED_SERVICE") else {
        return;
    };
}
""",
            encoding="utf-8",
        )
        try:
            _, errors = inventory()
            self.assertIn(
                "integration test can silently pass after missing environment configuration: "
                "crates/trpg-identity/tests/p00-false-skip-negative.rs (false_green)",
                errors,
            )
        finally:
            path.unlink()

    def test_inventory_rejects_result_success_after_missing_environment(self) -> None:
        source = """
#[tokio::test]
async fn false_green_result() -> Result<(), Box<dyn std::error::Error>> {
    let service = match env::var("REQUIRED_SERVICE") {
        Ok(service) => service,
        Err(_) => return Ok(()),
    };
    use_service(service).await?;
    Ok(())
}
"""
        self.assertEqual(
            integration_test_silent_env_successes(source),
            ["false_green_result"],
        )

    def test_inventory_ignores_comments_strings_and_unrelated_functions(self) -> None:
        source = r'''
fn helper() {
    let _service = std::env::var("HELPER_ONLY");
}

#[test]
fn legitimate_test() {
    let message = "std::env::var(\"FAKE\") then return;";
    // let Ok(_) = env::var("COMMENT_ONLY") else { return; };
    assert!(std::env::var("REQUIRED_SERVICE").expect("required").len() > 0);
}

fn unrelated_return() {
    return;
}
'''
        self.assertEqual(integration_test_silent_env_successes(source), [])
        literal_source = r"""fn borrow<'a>(value: &'a str) -> &'a str {
    let brace = '{';
    let escaped = '\u{7b}';
    let raw = r###"{ env::var(\"FAKE\") }"###;
    value
}
"""
        sanitized = _blank_rust_non_code(literal_source)
        self.assertIn("fn borrow<'a>(value: &'a str) -> &'a str {", sanitized)
        self.assertIn("value", sanitized)
        self.assertNotIn("env::var", sanitized)
        self.assertEqual(literal_source.count("\n"), sanitized.count("\n"))

    def test_inventory_does_not_cross_preceding_sibling_block(self) -> None:
        source = """
#[test]
fn legitimate_after_sibling_block() {
    if unrelated_probe() {
        return;
    }
    let required = std::env::var("REQUIRED_SERVICE").expect("required");
    use_service(required);
}
"""
        self.assertEqual(integration_test_silent_env_successes(source), [])

    def test_evidence_binds_environment_service_versions_and_real_test_details(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory) / "bound.json"
            environment = dict(os.environ)
            environment["P00_EVIDENCE_TEST_SCOPE"] = "dedicated-fixture"
            service_probe = json.dumps(
                ["python_runtime", sys.executable, "--version"]
            )
            result = subprocess.run(
                [
                    sys.executable,
                    "scripts/ci/generate_evidence.py",
                    "--report",
                    str(report),
                    "--artifact",
                    "MANIFEST.md",
                    "--environment-key",
                    "P00_EVIDENCE_TEST_SCOPE",
                    "--service-version-command",
                    service_probe,
                    "--",
                    sys.executable,
                    "-c",
                    (
                        "print('test evidence::passes ... ok'); "
                        "print('test evidence::skipped ... ignored')"
                    ),
                ],
                cwd=ROOT,
                env=environment,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            payload = json.loads(report.read_text(encoding="utf-8"))
            self.assertEqual(validate_evidence(payload, artifact_base=report.parent), [])
            self.assertRegex(
                payload["environment"]["variables"]["P00_EVIDENCE_TEST_SCOPE"],
                r"^sha256:[0-9a-f]{64}$",
            )
            self.assertIn(
                "Python", payload["environment"]["service_versions"]["python_runtime"]["output"]
            )
            junit = ET.parse(report.with_suffix(".junit.xml")).getroot()
            self.assertEqual(
                [case.get("name") for case in junit.findall("testcase")],
                ["evidence::passes", "evidence::skipped"],
            )
            self.assertEqual(junit.get("tests"), "2")
            self.assertEqual(junit.get("skipped"), "1")

            payload.update(
                github_run_id="123",
                github_run_attempt="1",
                workflow="test",
                job="test",
            )
            live = {
                "GITHUB_REPOSITORY": payload["repository"],
                "GITHUB_SHA": payload["github_sha"],
                "GITHUB_RUN_ID": "123",
                "GITHUB_RUN_ATTEMPT": "1",
                "GITHUB_WORKFLOW": "test",
                "GITHUB_JOB": "test",
                "RUNNER_OS": payload["runner_os"],
                "P00_EVIDENCE_TEST_SCOPE": "dedicated-fixture",
            }
            with patch.dict("os.environ", live, clear=False):
                self.assertEqual(
                    validate_evidence(
                        payload, artifact_base=report.parent, live_context=True
                    ),
                    [],
                )
            live["P00_EVIDENCE_TEST_SCOPE"] = "different-environment"
            with patch.dict("os.environ", live, clear=False):
                self.assertIn(
                    "environment variable digest mismatch: P00_EVIDENCE_TEST_SCOPE",
                    validate_evidence(
                        payload, artifact_base=report.parent, live_context=True
                    ),
                )

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
