#!/usr/bin/env python3
from __future__ import annotations

import copy
import json
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from acceptance_evidence_matrix import (
    ROOT,
    canonical_manifest_text,
    create_not_run_manifest,
    load_manifest,
    manifest_errors,
    matrix_file_errors,
    release_candidate_errors,
    render_matrix,
    sha256_file,
)
from acceptance_evidence_matrix_core import (
    acceptance_commands,
    row_command_evidence_errors,
)
from release_readiness import assess
from repo_truth import evidence_environment_sha256


class AcceptanceEvidenceMatrixTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.artifact_root = Path(self.temporary_directory.name)
        self.source = self.artifact_root / "release-input.json"
        self.source.write_text('{"status":"PASS"}\n', encoding="utf-8")
        self.manifest = self.artifact_root / "acceptance-evidence-manifest.json"
        self.matrix = self.artifact_root / "V1_ACCEPTANCE_EVIDENCE_MATRIX_FILLED.md"
        self.data = create_not_run_manifest([self.source], self.manifest)
        self._write_manifest(self.data)
        self.matrix.write_text(
            render_matrix(self.data, sha256_file(self.manifest)), encoding="utf-8"
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def _write_manifest(self, data: dict) -> None:
        self.manifest.write_text(canonical_manifest_text(data), encoding="utf-8")

    def _add_json_source(self, name: str, payload: dict) -> None:
        path = self.artifact_root / name
        path.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        self.data["source_evidence"].append(
            {"path": name, "sha256": sha256_file(path)}
        )
        self.data["source_evidence"].sort(key=lambda item: item["path"])

    def _machine_command_evidence(self, name: str) -> tuple[Path, dict]:
        report = self.artifact_root / name
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
                "print('test acceptance::machine_evidence ... ok')",
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        payload = json.loads(report.read_text(encoding="utf-8"))
        channel_match = re.search(
            r'(?m)^channel\s*=\s*"([^"]+)"',
            (ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"),
        )
        self.assertIsNotNone(channel_match)
        channel = channel_match.group(1)
        package = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))
        payload["tool_versions"].update(
            python=(ROOT / ".python-version").read_text(encoding="utf-8").strip(),
            node="v" + (ROOT / ".nvmrc").read_text(encoding="utf-8").strip(),
            pnpm=package["packageManager"].removeprefix("pnpm@"),
            rustc=f"rustc {channel}",
            cargo=f"cargo {channel}",
        )
        payload["environment_sha256"] = evidence_environment_sha256(
            payload["tool_versions"], payload["environment"]
        )
        report.write_text(
            json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
        return report, payload

    def test_generator_and_validator_bind_current_candidate_and_all_rows(self) -> None:
        loaded, manifest_found = load_manifest(self.manifest)
        self.assertEqual(manifest_found, [])
        self.assertEqual(loaded, self.data)
        validated, matrix_found = matrix_file_errors(self.manifest, self.matrix)
        self.assertEqual(matrix_found, [])
        self.assertEqual(validated, self.data)
        self.assertEqual([row["id"] for row in self.data["rows"]], list(range(1, 18)))
        self.assertEqual(self.data["rows"][14]["status"], "BLOCKED")

    def test_machine_verified_command_evidence_can_support_a_pass_row(self) -> None:
        report, payload = self._machine_command_evidence("verified.json")
        data = create_not_run_manifest([report], self.manifest)
        row = data["rows"][3]
        row.update(
            status="PASS",
            command=payload["command"],
            exit_code=0,
            evidence=[report.name],
            notes="machine-verified command evidence",
        )
        self._write_manifest(data)
        commands = acceptance_commands()
        commands[4] = payload["command"]
        with patch(
            "acceptance_evidence_matrix_validation.acceptance_commands",
            return_value=commands,
        ):
            _, found = load_manifest(self.manifest)
        self.assertEqual(found, [])

    def test_zero_test_cargo_evidence_cannot_support_a_pass_row(self) -> None:
        expected = "cargo test -p example definitely_missing"
        row = {"id": 4, "status": "PASS", "exit_code": 0}
        payload = {
            "command": expected,
            "command_argv": expected.split(),
            "status": "PASS",
            "exit_code": 0,
            "command_artifact_sha256": {"zero.junit.xml": "0" * 64},
        }
        with patch(
            "acceptance_evidence_matrix_core.command_evidence",
            return_value=(payload, []),
        ), patch(
            "acceptance_evidence_matrix_core.passing_test_names",
            return_value={expected},
        ):
            found = row_command_evidence_errors(
                row,
                "zero.json",
                {"zero.json": self.source},
                ROOT,
                expected,
                {},
            )
        self.assertIn(
            "row 4 evidence zero.json: canonical cargo test executed no passing test cases",
            found,
        )

    def test_valid_evidence_for_another_command_cannot_be_reused(self) -> None:
        report, _ = self._machine_command_evidence("wrong-command.json")
        data = create_not_run_manifest([report], self.manifest)
        data["rows"][3].update(
            status="PASS",
            command=acceptance_commands()[4],
            exit_code=0,
            evidence=[report.name],
            notes="wrong command evidence",
        )
        self._write_manifest(data)
        _, found = load_manifest(self.manifest)
        self.assertIn(
            "row 4 evidence wrong-command.json: command does not execute the canonical row command",
            found,
        )

    def test_machine_verified_dependent_row_can_close_repair_batch(self) -> None:
        report, payload = self._machine_command_evidence("rf04-closure.json")
        payload.update(
            batch_id="RF04",
            head_sha=payload["base_commit"],
            tree_sha=payload["tree_sha"],
            finding_status={"F-AR01-005": "PASS"},
            negative_tests=["acceptance::machine_evidence"],
            not_run=[],
        )
        report.write_text(
            json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
        data = create_not_run_manifest([report], self.manifest)
        data["rows"][6].update(
            status="PASS",
            command=payload["command"],
            exit_code=0,
            evidence=[report.name],
            notes="machine-verified RF04 dependent row",
        )
        rf04 = next(
            batch for batch in data["repair_batches"] if batch["id"] == "RF04"
        )
        rf04.update(status="PASS", evidence=[report.name])
        self._write_manifest(data)
        commands = acceptance_commands()
        commands[7] = payload["command"]
        with patch(
            "acceptance_evidence_matrix_validation.acceptance_commands",
            return_value=commands,
        ):
            _, rejected = load_manifest(self.manifest)
        self.assertIn(
            "RF04 evidence rf04-closure.json: command does not execute the repair batch acceptance gate",
            rejected,
        )
        self.assertIn(
            "RF04 evidence rf04-closure.json: requires at least 4 executed negative tests",
            rejected,
        )
        with patch(
            "acceptance_evidence_matrix_validation.acceptance_commands",
            return_value=commands,
        ), patch.dict(
            "acceptance_evidence_matrix_validation.REPAIR_BATCH_COMMANDS",
            {"RF04": payload["command"]},
        ), patch.dict(
            "acceptance_evidence_matrix_validation.REPAIR_BATCH_MINIMUM_NEGATIVE_TESTS",
            {"RF04": 1},
        ):
            _, found = load_manifest(self.manifest)
        self.assertEqual(found, [])

    def test_old_candidate_commit_is_rejected(self) -> None:
        stale = copy.deepcopy(self.data)
        stale["candidate_commit"] = "68f1c1772993b00dd9c6390c237fa44d9ec14c7e"
        self._write_manifest(stale)
        _, found = load_manifest(self.manifest)
        self.assertIn("acceptance matrix candidate commit does not equal HEAD", found)

    def test_wrong_candidate_tree_is_rejected(self) -> None:
        stale = copy.deepcopy(self.data)
        stale["candidate_tree"] = "0" * 40
        self._write_manifest(stale)
        _, found = load_manifest(self.manifest)
        self.assertIn("acceptance matrix candidate tree does not equal HEAD tree", found)

    def test_missing_row_is_rejected(self) -> None:
        missing = copy.deepcopy(self.data)
        del missing["rows"][8]
        self._write_manifest(missing)
        _, found = load_manifest(self.manifest)
        self.assertIn(
            "acceptance matrix must contain rows 1 through 17 exactly once", found
        )

    def test_modified_evidence_hash_is_rejected(self) -> None:
        tampered = copy.deepcopy(self.data)
        tampered["source_evidence"][0]["sha256"] = "0" * 64
        self._write_manifest(tampered)
        _, found = load_manifest(self.manifest)
        self.assertIn("source evidence hash mismatch: release-input.json", found)

    def test_blocked_row_cannot_be_changed_to_pass(self) -> None:
        forged = copy.deepcopy(self.data)
        row = forged["rows"][14]
        row.update(status="PASS", command="python3 forged.py", exit_code=0)
        self._write_manifest(forged)
        _, found = load_manifest(self.manifest)
        self.assertIn("row 15 cannot PASS while RF01 is not closed", found)
        self.assertIn("row 15 cannot PASS while RF02 is not closed", found)

    def test_manual_pass_requires_machine_verified_command_evidence(self) -> None:
        forged = copy.deepcopy(self.data)
        forged["rows"][0].update(
            status="PASS",
            command="true",
            exit_code=0,
            notes="manually asserted PASS",
        )
        self._write_manifest(forged)
        _, found = load_manifest(self.manifest)
        self.assertTrue(
            any(
                error.startswith("row 1 evidence release-input.json:")
                for error in found
            ),
            found,
        )

    def test_fully_self_asserted_ready_manifest_is_rejected(self) -> None:
        for batch in self.data["repair_batches"]:
            name = f"{batch['id']}.json"
            self._add_json_source(
                name,
                {
                    "batch_id": batch["id"],
                    "head_sha": self.data["candidate_commit"],
                    "tree_sha": self.data["candidate_tree"],
                    "finding_status": {batch["finding"]: "PASS"},
                },
            )
            batch.update(status="PASS", evidence=[name])
        for row in self.data["rows"]:
            row.update(
                status="PASS",
                command="true",
                exit_code=0,
                evidence=[self.source.name],
                notes="manually asserted PASS",
            )
        self._write_manifest(self.data)
        _, found = load_manifest(self.manifest)
        self.assertTrue(
            any(
                error.startswith("row 1 evidence release-input.json:")
                for error in found
            ),
            found,
        )

    def test_generated_matrix_edit_is_rejected(self) -> None:
        original = self.matrix.read_text(encoding="utf-8")
        self.assertIn("| 15 |", original)
        self.matrix.write_text(original.replace("BLOCKED", "PASS", 1), encoding="utf-8")
        _, found = matrix_file_errors(self.manifest, self.matrix)
        self.assertIn(
            "generated acceptance matrix does not match evidence manifest", found
        )

    def test_repository_local_manifest_is_rejected(self) -> None:
        found = manifest_errors(self.data, ROOT / "MANIFEST.md")
        self.assertIn("acceptance evidence manifest must be outside the repository", found)

    def test_release_readiness_accepts_structure_but_blocks_incomplete_rows(self) -> None:
        report = assess(
            ROOT,
            acceptance_manifest=self.manifest,
            acceptance_matrix=self.matrix,
        )
        blocker_ids = {blocker["id"] for blocker in report["blockers"]}
        self.assertNotIn("INVALID_ACCEPTANCE_EVIDENCE_MATRIX", blocker_ids)
        self.assertNotIn("MISSING_ACCEPTANCE_EVIDENCE_MATRIX", blocker_ids)
        self.assertIn("INCOMPLETE_ACCEPTANCE_EVIDENCE_MATRIX", blocker_ids)

    def test_noncanonical_manifest_is_rejected(self) -> None:
        self.manifest.write_text(json.dumps(self.data), encoding="utf-8")
        _, found = load_manifest(self.manifest)
        self.assertIn("acceptance evidence manifest is not canonical JSON", found)

    def test_require_ready_rejects_a_dirty_candidate_worktree(self) -> None:
        with patch(
            "acceptance_evidence_matrix_core.worktree_diff_sha256",
            return_value="1" * 64,
        ):
            found = release_candidate_errors(self.data)
        self.assertIn("release candidate worktree must be clean", found)


if __name__ == "__main__":
    unittest.main()
