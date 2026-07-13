#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

from repo_truth import ROOT, cargo_targets, decision_values


ACTION = re.compile(r"uses:\s*([^\s@]+)@([^\s#]+)")
SCRIPT = re.compile(r"(?:\./)?(scripts/[A-Za-z0-9_./-]+\.(?:py|sh|ps1|mjs))")
TEST_TARGET = re.compile(r"--test\s+([A-Za-z0-9_-]+)")
PINNED_GATE_TOKENS = (
    "actionlint_1.7.7_linux_amd64.tar.gz",
    "023070a287cd8cccd71515fedc843f1985bf96c436b7effaecce67290e7e0757",
    "shellcheck-v0.10.0.linux.x86_64.tar.xz",
    "6c881ab0698e4e6ea235245f22832860544f17ba386442fe7e9d629f8cbedf87",
    "opa_linux_amd64_static",
    "9903e5125ac281104f2c4b7371d10cc3b74a98933743fcbfc174f9bf0ab20de8",
)
PINNED_GATE_DOWNLOADS = (
    (
        "https://github.com/rhysd/actionlint/releases/download/v1.7.7/actionlint_1.7.7_linux_amd64.tar.gz",
        "023070a287cd8cccd71515fedc843f1985bf96c436b7effaecce67290e7e0757",
    ),
    (
        "https://github.com/koalaman/shellcheck/releases/download/v0.10.0/shellcheck-v0.10.0.linux.x86_64.tar.xz",
        "6c881ab0698e4e6ea235245f22832860544f17ba386442fe7e9d629f8cbedf87",
    ),
    (
        "https://openpolicyagent.org/downloads/v1.18.2/opa_linux_amd64_static",
        "9903e5125ac281104f2c4b7371d10cc3b74a98933743fcbfc174f9bf0ab20de8",
    ),
)


def validate(root: Path = ROOT) -> list[str]:
    workflows = sorted((root / ".github/workflows").glob("*.yml"))
    errors = []
    if not workflows:
        return ["no .github/workflows/*.yml files"]
    try:
        decision = decision_values(root)
        canonical_branch = decision["CANONICAL_BRANCH"]
        non_canonical_branch = decision.get("NON_CANONICAL_BRANCH")
    except (KeyError, ValueError) as error:
        return [f"canonical branch decision unavailable: {error}"]
    gate = (root / "scripts/ci/test-all.sh").read_text(encoding="utf-8")
    for token in PINNED_GATE_TOKENS:
        if token not in gate:
            errors.append(f"scripts/ci/test-all.sh: missing pinned tool token {token}")
    gate_lines = gate.splitlines()
    curl_lines = [index for index, line in enumerate(gate_lines) if line.startswith("curl ")]
    if len(curl_lines) != len(PINNED_GATE_DOWNLOADS):
        errors.append("scripts/ci/test-all.sh: unexpected downloaded tool count")
    for url, digest in PINNED_GATE_DOWNLOADS:
        matches = [index for index in curl_lines if url in gate_lines[index]]
        if len(matches) != 1:
            errors.append(f"scripts/ci/test-all.sh: expected one pinned download for {url}")
        elif matches[0] + 1 >= len(gate_lines) or not all(
            token in gate_lines[matches[0] + 1] for token in (digest, "sha256sum -c -")
        ):
            errors.append(f"scripts/ci/test-all.sh: checksum is not adjacent to download {url}")
    targets = cargo_targets(root, "test")
    for path in workflows:
        text = path.read_text(encoding="utf-8")
        relative = path.relative_to(root).as_posix()
        for token in ("permissions:", "timeout-minutes:", "concurrency:", "cancel-in-progress: true"):
            if token not in text:
                errors.append(f"{relative}: missing {token}")
        for forbidden in ("continue-on-error", "--if-present"):
            if forbidden in text:
                errors.append(f"{relative}: forbidden {forbidden}")
        if "go install" in text:
            errors.append(f"{relative}: build-time tool installation is not reproducible")
        if "curl " in text and "sha256sum -c" not in text:
            errors.append(f"{relative}: downloaded tools require SHA-256 verification")
        if "generate_evidence.py" in text:
            for forged in ("--status", "--exit-code"):
                if forged in text:
                    errors.append(f"{relative}: evidence cannot accept caller-supplied {forged}")
            if "actions/upload-artifact@" not in text:
                errors.append(f"{relative}: generated evidence must be uploaded")
        if "actions/upload-artifact@" in text:
            for token in (
                "if: always()",
                "github.sha",
                "github.run_id",
                "github.run_attempt",
                "if-no-files-found: error",
                "retention-days:",
                "evidence-manifest.json",
            ):
                if token not in text:
                    errors.append(f"{relative}: evidence upload missing {token}")
        if "runs-on: ubuntu-24.04" not in text or "ubuntu-latest" in text:
            errors.append(f"{relative}: runner image must be pinned to ubuntu-24.04")
        if "\t" in text:
            errors.append(f"{relative}: tabs are not valid indentation")
        for action, reference in ACTION.findall(text):
            if not re.fullmatch(r"[0-9a-f]{40}", reference):
                errors.append(f"{relative}: mutable action reference {action}@{reference}")
        for script in SCRIPT.findall(text):
            if not (root / script).is_file():
                errors.append(f"{relative}: missing script {script}")
            if script.endswith(".sh") and not re.search(
                rf"(?m)bash(?:\s+-n)?\s+(?:\./)?{re.escape(script)}", text
            ):
                errors.append(f"{relative}: shell script must use explicit bash: {script}")
        for target in TEST_TARGET.findall(text):
            if target not in targets:
                errors.append(f"{relative}: missing Cargo test target {target}")
        for event, block in re.findall(
            r"(?ms)^  (push|pull_request):\s*\n((?:    .*\n?)*)", text
        ):
            if canonical_branch not in block:
                errors.append(f"{relative}: {event} policy must name {canonical_branch}")
            if non_canonical_branch and non_canonical_branch in block:
                errors.append(f"{relative}: {event} policy includes non-canonical {non_canonical_branch}")
    required_checks = {
        "ci.yml": ("name: workspace-ci", "  workspace:"),
        "contracts.yml": ("name: repository-truth", "  truth:"),
        "golden-scenarios.yml": ("name: golden-scenarios", "  golden:"),
    }
    for name, tokens in required_checks.items():
        path = root / ".github" / "workflows" / name
        if not path.is_file():
            errors.append(f"missing required-check workflow: {name}")
            continue
        text = path.read_text(encoding="utf-8")
        for token in tokens:
            if token not in text:
                errors.append(f"{name}: missing required-check identity {token.strip()}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args()
    errors = validate(args.root.resolve())
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("workflow static validation passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
