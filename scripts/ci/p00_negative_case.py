#!/usr/bin/env python3
from __future__ import annotations

import argparse
import difflib
import hashlib
import json
import subprocess
import sys
from pathlib import Path

from repo_truth import ROOT


CASES = (
    "CASE-01_broken_test_assertion",
    "CASE-02_missing_referenced_script",
    "CASE-03_missing_required_evidence",
    "CASE-04_manifest_hash_tamper",
    "CASE-05_readiness_provenance_removed",
)


def run(command: list[str]) -> tuple[int, str]:
    result = subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        encoding="utf-8",
        errors="replace",
        capture_output=True,
    )
    output = (
        f"$ {' '.join(command)}\n[stdout]\n{result.stdout}[stderr]\n{result.stderr}"
        f"[exit_code]\n{result.returncode}\n"
    )
    return result.returncode, output


def text_diff(name: str, before: str, after: str) -> str:
    return "".join(
        difflib.unified_diff(
            before.splitlines(keepends=True),
            after.splitlines(keepends=True),
            fromfile=f"a/{name}",
            tofile=f"b/{name}",
        )
    )


def evidence_case(output_dir: Path) -> tuple[list[str], list[str], str, list[Path], str]:
    report = output_dir / "fault-evidence.json"
    baseline = [
        sys.executable,
        "scripts/ci/generate_evidence.py",
        "--report",
        str(report),
        "--artifact",
        "MANIFEST.md",
        "--",
        sys.executable,
        "-c",
        "pass",
    ]
    baseline_code, baseline_output = run(baseline)
    if baseline_code:
        raise RuntimeError(baseline_output)
    verify = [sys.executable, "scripts/ci/verify_evidence_schema.py", str(report)]
    verify_code, verify_output = run(verify)
    if verify_code:
        raise RuntimeError(verify_output)
    before = report.read_text(encoding="utf-8")
    payload = json.loads(before)
    del payload["generated_at_utc"]
    after = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    report.write_text(after, encoding="utf-8", newline="\n")
    cleanup = [report, report.with_suffix(".log"), report.with_suffix(".junit.xml"), report.with_suffix(".sarif")]
    return verify, verify, baseline_output + verify_output, cleanup, text_diff(report.name, before, after)


def readiness_case(output_dir: Path) -> tuple[list[str], list[str], str, list[Path], str]:
    report = output_dir / "fault-readiness.json"
    generate = [sys.executable, "scripts/ci/release_readiness.py", "--report", str(report)]
    generate_code, generate_output = run(generate)
    if generate_code:
        raise RuntimeError(generate_output)
    verify = [sys.executable, "scripts/ci/release_readiness.py", "--verify-report", str(report)]
    verify_code, verify_output = run(verify)
    if verify_code:
        raise RuntimeError(verify_output)
    before = report.read_text(encoding="utf-8")
    payload = json.loads(before)
    del payload["base_commit"]
    after = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    report.write_text(after, encoding="utf-8", newline="\n")
    return verify, verify, generate_output + verify_output, [report], text_diff(report.name, before, after)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--case", choices=CASES, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    output_dir = args.output_dir.resolve()
    if output_dir.is_relative_to(ROOT.resolve()):
        parser.error("--output-dir must be outside the repository")
    output_dir.mkdir(parents=True, exist_ok=True)

    restore: tuple[Path, bytes] | None = None
    cleanup: list[Path] = []
    baseline_output = ""
    diff = ""
    if args.case == CASES[0]:
        path = ROOT / "scripts/ci/test_repo_truth.py"
        before = path.read_bytes()
        needle = b'self.assertEqual(report["status"], "BLOCKED")'
        if before.count(needle) != 1:
            raise SystemExit("expected one readiness assertion to inject")
        baseline = validation = [sys.executable, "scripts/ci/test_repo_truth.py"]
        restore = (path, before)
        injected = before.replace(needle, b'self.assertEqual(report["status"], "READY")')
        baseline_code, baseline_output = run(baseline)
        if baseline_code:
            raise SystemExit(baseline_output)
        path.write_bytes(injected)
    elif args.case == CASES[1]:
        path = ROOT / "scripts/ci/test-all.sh"
        before = path.read_bytes()
        baseline = validation = [sys.executable, "scripts/ci/validate_workflows.py"]
        restore = (path, before)
        baseline_code, baseline_output = run(baseline)
        if baseline_code:
            raise SystemExit(baseline_output)
        path.unlink()
    elif args.case == CASES[2]:
        validation, _, baseline_output, cleanup, diff = evidence_case(output_dir)
    elif args.case == CASES[3]:
        path = ROOT / "MANIFEST.md"
        before = path.read_bytes()
        baseline = validation = [sys.executable, "scripts/ci/manifest.py", "--check"]
        restore = (path, before)
        baseline_code, baseline_output = run(baseline)
        if baseline_code:
            raise SystemExit(baseline_output)
        path.write_bytes(before + b"tampered\n")
    else:
        validation, _, baseline_output, cleanup, diff = readiness_case(output_dir)

    try:
        validation_code, validation_output = run(validation)
        if not diff:
            diff = subprocess.run(
                ["git", "diff", "--binary"], cwd=ROOT, check=True, capture_output=True
            ).stdout.decode("utf-8", errors="replace")
        (output_dir / "baseline-output.txt").write_text(baseline_output, encoding="utf-8", newline="\n")
        (output_dir / "validation-output.txt").write_text(validation_output, encoding="utf-8", newline="\n")
        (output_dir / "fault.diff").write_text(diff, encoding="utf-8", newline="\n")
        (output_dir / "exit-code.txt").write_text(f"{validation_code}\n", encoding="utf-8")
        if validation_code == 0:
            return 1
        hashes = []
        for name in ("baseline-output.txt", "validation-output.txt", "fault.diff", "exit-code.txt"):
            path = output_dir / name
            hashes.append(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {name}")
        (output_dir / "sha256.txt").write_text("\n".join(hashes) + "\n", encoding="utf-8")
        return 0
    finally:
        if restore:
            restore[0].write_bytes(restore[1])
        for path in cleanup:
            path.unlink(missing_ok=True)


if __name__ == "__main__":
    raise SystemExit(main())
