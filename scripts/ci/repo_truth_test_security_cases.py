from __future__ import annotations

from repo_truth_test_support import *


class SecurityAndManifestCases:
    def test_runtime_witness_owner_credential_injection_is_rejected(self) -> None:
        compose_path = ROOT / "compose.yml"
        original_read_text = Path.read_text
        original_compose = original_read_text(compose_path, encoding="utf-8")
        tampered_compose = original_compose.replace(
            "TRPG_WITNESS_DATABASE_URL_SECRET_ID: witness_append_database_url",
            "TRPG_WITNESS_DATABASE_URL_SECRET_ID: witness_owner_database_url",
            1,
        ).replace(
            "- source: witness_append_database_url\n"
            "        target: witness_append_database_url.v1",
            "- source: witness_owner_database_url\n"
            "        target: witness_owner_database_url.v1",
            1,
        )
        self.assertNotEqual(tampered_compose, original_compose)

        def tampered_read_text(
            path: Path, *args: object, **kwargs: object
        ) -> str:
            if path == compose_path:
                return tampered_compose
            return original_read_text(path, *args, **kwargs)

        with patch.object(Path, "read_text", tampered_read_text):
            found = compose_security_errors(ROOT)
        self.assertIn(
            "api does not select its least-privilege witness URL: "
            "witness_append_database_url",
            found,
        )
        self.assertIn(
            "api also receives forbidden witness URL: witness_owner_database_url",
            found,
        )

    def test_mutable_witness_runtime_grant_is_rejected(self) -> None:
        migration_path = (
            ROOT
            / "migrations/witness/20260726000100_restrict_witness_runtime_privileges.up.sql"
        )
        original_read_text = Path.read_text
        original_migration = original_read_text(migration_path, encoding="utf-8")
        tampered_migration = original_migration.replace(
            "GRANT SELECT, INSERT ON TABLE external_audit_witness",
            "GRANT SELECT, INSERT, UPDATE ON TABLE external_audit_witness",
            1,
        )
        self.assertNotEqual(tampered_migration, original_migration)

        def tampered_read_text(
            path: Path, *args: object, **kwargs: object
        ) -> str:
            if path == migration_path:
                return tampered_migration
            return original_read_text(path, *args, **kwargs)

        with patch.object(Path, "read_text", tampered_read_text):
            found = compose_security_errors(ROOT)
        self.assertIn(
            "witness runtime migration grants mutable/owner privileges",
            found,
        )

    def test_openfga_version_normalization_preserves_real_version_drift(self) -> None:
        command = ["docker", "exec", "trpg-openfga", "/openfga", "version"]
        first = (
            "2026/07/25 17:35:13 OpenFGA version `v1.15.1` build from "
            "`1db35fb8b33d7512666e49e6b75cbd2cca8c4694` on "
            "`2026-05-06T20:12:25Z`"
        )
        second = first.replace("17:35:13", "17:50:11")
        expected = first.split(" ", 2)[2]
        self.assertEqual(stable_service_version_output(command, first), expected)
        self.assertEqual(stable_service_version_output(command, second), expected)

        changed_version = second.replace("v1.15.1", "v1.15.2")
        self.assertNotEqual(
            stable_service_version_output(command, changed_version), expected
        )
        malformed = "prefix " + expected
        self.assertEqual(stable_service_version_output(command, malformed), malformed)
        self.assertEqual(
            stable_service_version_output(["openfga", "version"], first), first
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
