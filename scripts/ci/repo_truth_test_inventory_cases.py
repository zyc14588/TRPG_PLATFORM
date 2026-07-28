from __future__ import annotations

from repo_truth_test_support import *


class InventoryAndBindingCases:
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
