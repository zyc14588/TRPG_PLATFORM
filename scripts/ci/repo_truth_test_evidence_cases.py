from __future__ import annotations

from repo_truth_test_support import *


class EvidenceArtifactCases:
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
            self.assertEqual(
                set(payload["command_artifact_sha256"]),
                {
                    report.with_suffix(".log").name,
                    report.with_suffix(".junit.xml").name,
                    report.with_suffix(".sarif").name,
                },
            )
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

    def test_aggregate_evidence_separates_command_artifacts_from_hashed_attachments(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            artifact_root = Path(directory)
            child = artifact_root / "child.json"
            child_result = subprocess.run(
                [
                    sys.executable,
                    "scripts/ci/generate_evidence.py",
                    "--report",
                    str(child),
                    "--artifact",
                    "MANIFEST.md",
                    "--",
                    sys.executable,
                    "-c",
                    "print('test child::passes ... ok')",
                ],
                cwd=ROOT,
                capture_output=True,
                text=True,
            )
            self.assertEqual(child_result.returncode, 0, child_result.stderr)

            aggregate = artifact_root / "aggregate.json"
            command = [
                sys.executable,
                "scripts/ci/generate_evidence.py",
                "--report",
                str(aggregate),
                "--artifact",
                "MANIFEST.md",
            ]
            for path in (
                child,
                child.with_suffix(".log"),
                child.with_suffix(".junit.xml"),
                child.with_suffix(".sarif"),
            ):
                command.extend(("--generated-artifact", str(path)))
            command.extend(
                (
                    "--",
                    sys.executable,
                    "-c",
                    "print('test aggregate::passes ... ok')",
                )
            )
            aggregate_result = subprocess.run(
                command,
                cwd=ROOT,
                capture_output=True,
                text=True,
            )
            self.assertEqual(aggregate_result.returncode, 0, aggregate_result.stderr)

            payload = json.loads(aggregate.read_text(encoding="utf-8"))
            self.assertEqual(validate_evidence(payload, artifact_base=artifact_root), [])
            self.assertEqual(len(payload["command_artifact_sha256"]), 3)
            self.assertEqual(len(payload["generated_artifact_sha256"]), 7)
            self.assertEqual(
                len(
                    [
                        name
                        for name in payload["generated_artifact_sha256"]
                        if name.endswith(".log")
                    ]
                ),
                2,
            )

            primary_log = aggregate.with_suffix(".log").name
            child_log = child.with_suffix(".log").name
            primary_hash = payload["command_artifact_sha256"].pop(primary_log)
            payload["command_artifact_sha256"][child_log] = payload[
                "generated_artifact_sha256"
            ][child_log]
            self.assertIn(
                "expected exactly one raw output bound to command and exit_code",
                validate_evidence(payload, artifact_base=artifact_root),
            )
            payload["command_artifact_sha256"].pop(child_log)
            payload["command_artifact_sha256"][primary_log] = "0" * 64
            self.assertIn(
                f"command artifact hash mismatch: {primary_log}",
                validate_evidence(payload, artifact_base=artifact_root),
            )
            payload["command_artifact_sha256"][primary_log] = primary_hash
            self.assertEqual(validate_evidence(payload, artifact_base=artifact_root), [])

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
