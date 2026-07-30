#!/usr/bin/env python3
"""Enforce the single time-bounded, unreachable RustSec exception."""

from __future__ import annotations

import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
AUDIT_CONFIG = REPOSITORY_ROOT / ".cargo" / "audit.toml"
EXPECTED_ADVISORY = "RUSTSEC-2023-0071"
EXPECTED_PACKAGE = "rsa@0.9.7"
REQUIRED_METADATA = ("owner", "approved-by", "expires", "evidence")
METADATA_PATTERN = re.compile(
    r"^# AR04-(owner|approved-by|expires|evidence):\s*(\S(?:.*\S)?)\s*$"
)


def fail(message: str) -> int:
    print(f"AR04_RUSTSEC_EXCEPTION=FAIL: {message}", file=sys.stderr)
    return 1


def load_metadata(config_text: str) -> dict[str, str]:
    metadata: dict[str, str] = {}
    for line in config_text.splitlines():
        match = METADATA_PATTERN.fullmatch(line)
        if match:
            metadata[match.group(1)] = match.group(2)
    return metadata


def main() -> int:
    try:
        config_text = AUDIT_CONFIG.read_text(encoding="utf-8")
    except OSError as error:
        return fail(f"cannot read {AUDIT_CONFIG.relative_to(REPOSITORY_ROOT)}: {error}")

    metadata = load_metadata(config_text)
    missing = [field for field in REQUIRED_METADATA if not metadata.get(field)]
    if missing:
        return fail(f"missing metadata: {', '.join(missing)}")

    ignored_advisories = re.findall(r'"(RUSTSEC-\d{4}-\d{4})"', config_text)
    if ignored_advisories != [EXPECTED_ADVISORY]:
        return fail(
            "audit config must ignore exactly "
            f"{EXPECTED_ADVISORY}, found {ignored_advisories}"
        )

    try:
        expires = datetime.strptime(metadata["expires"], "%Y-%m-%d").date()
    except ValueError:
        return fail("expires must use YYYY-MM-DD")

    today = datetime.now(timezone.utc).date()
    if today > expires:
        return fail(f"exception expired on {expires.isoformat()}")

    evidence_path = REPOSITORY_ROOT / metadata["evidence"]
    if not evidence_path.is_file():
        return fail(f"evidence file does not exist: {metadata['evidence']}")

    command = [
        "cargo",
        "tree",
        "-i",
        EXPECTED_PACKAGE,
        "--workspace",
        "--all-features",
        "--target",
        "all",
        "--locked",
    ]
    completed = subprocess.run(
        command,
        cwd=REPOSITORY_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        return fail(
            f"{EXPECTED_PACKAGE} could not be checked (exit "
            f"{completed.returncode}): {completed.stderr.strip()}"
        )
    if completed.stdout.strip():
        return fail(
            f"{EXPECTED_PACKAGE} is reachable and may not be ignored:\n"
            f"{completed.stdout.rstrip()}"
        )

    print(
        "AR04_RUSTSEC_EXCEPTION=PASS "
        f"advisory={EXPECTED_ADVISORY} expires={expires.isoformat()} "
        f"owner={metadata['owner']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
