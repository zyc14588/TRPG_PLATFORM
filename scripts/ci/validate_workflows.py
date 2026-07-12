#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

from repo_truth import ROOT, cargo_targets


ACTION = re.compile(r"uses:\s*([^\s@]+)@([^\s#]+)")
SCRIPT = re.compile(r"(?:\./)?(scripts/[A-Za-z0-9_./-]+\.(?:py|sh|ps1|mjs))")
TEST_TARGET = re.compile(r"--test\s+([A-Za-z0-9_-]+)")


def validate(root: Path = ROOT) -> list[str]:
    workflows = sorted((root / ".github/workflows").glob("*.yml"))
    errors = []
    if not workflows:
        return ["no .github/workflows/*.yml files"]
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
        if "pull_request:" in text or re.search(r"(?m)^\s*push:\s*$", text):
            if not all(branch in text for branch in ("master", "main")):
                errors.append(f"{relative}: push/PR policy must name master and main")
        template = root / "ci-cd/workflows-extractable" / f"target-{path.name}.md"
        if template.is_file():
            source = template.read_text(encoding="utf-8")
            match = re.search(r"```yaml\s*(.*?)\s*```", source, re.S)
            if not match or match.group(1).strip() != text.strip():
                errors.append(f"{relative}: canonical extractable template drift")
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
