#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

from manifest import OUTPUTS, manifest_source_errors, render
from repo_truth import ROOT


HASHED_ROW = re.compile(
    r"^\| `(?P<path>[^`]+)` \| [0-9]+ \| `[0-9a-f]{64}` \| `[0-7]{6}` \|$"
)
SELF_REFERENCE_ROW = re.compile(
    r"^\| `(?P<path>[^`]+)` \| self \| `evidence-bound-self-reference` "
    r"\| `[0-7]{6}` \|$"
)


def manifest_count_errors(content: str) -> list[str]:
    lines = content.splitlines()
    errors: list[str] = []

    def declared_count(prefix: str) -> int | None:
        values = [line.removeprefix(prefix) for line in lines if line.startswith(prefix)]
        if len(values) != 1 or not values[0].isdigit():
            errors.append(f"manifest must declare exactly one numeric {prefix.rstrip()}")
            return None
        return int(values[0])

    repository_count = declared_count("Repository files: ")
    hashed_count = declared_count("Hashed files: ")
    table_rows = [line for line in lines if line.startswith("| `")]
    hashed_rows = [line for line in table_rows if HASHED_ROW.fullmatch(line)]
    self_reference_rows = [
        line for line in table_rows if SELF_REFERENCE_ROW.fullmatch(line)
    ]
    malformed_rows = [
        line
        for line in table_rows
        if line not in hashed_rows and line not in self_reference_rows
    ]
    if malformed_rows:
        errors.append(f"manifest has {len(malformed_rows)} malformed path rows")
    if repository_count is not None and repository_count != len(table_rows):
        errors.append(
            "repository file count does not match manifest path rows: "
            f"{repository_count} != {len(table_rows)}"
        )
    if hashed_count is not None and hashed_count != len(hashed_rows):
        errors.append(
            "hashed file count does not match hashed path rows: "
            f"{hashed_count} != {len(hashed_rows)}"
        )
    sentinel_paths = {
        match.group("path")
        for line in self_reference_rows
        if (match := SELF_REFERENCE_ROW.fullmatch(line)) is not None
    }
    if sentinel_paths != set(OUTPUTS):
        errors.append(
            "self-reference sentinel paths do not match manifest outputs: "
            f"{sorted(sentinel_paths)} != {sorted(OUTPUTS)}"
        )
    row_paths = []
    for line in table_rows:
        match = HASHED_ROW.fullmatch(line) or SELF_REFERENCE_ROW.fullmatch(line)
        if match is not None:
            row_paths.append(match.group("path"))
    if len(row_paths) != len(set(row_paths)):
        errors.append("manifest contains duplicate path rows")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path)
    args = parser.parse_args()
    source_errors = manifest_source_errors()
    if source_errors:
        print(
            "manifest drift: Git index excludes unstaged or untracked source paths: "
            + ", ".join(source_errors),
            file=sys.stderr,
        )
        return 1
    expected = render()
    errors = [f"generated manifest: {error}" for error in manifest_count_errors(expected)]
    expected_bytes = expected.encode("utf-8")
    paths = (args.manifest,) if args.manifest else tuple(ROOT / name for name in OUTPUTS)
    for path in paths:
        if path is None or not path.is_file():
            errors.append(f"missing manifest: {path}")
            continue
        content_bytes = path.read_bytes()
        try:
            content = content_bytes.decode("utf-8")
        except UnicodeDecodeError:
            errors.append(f"manifest is not UTF-8: {path}")
            continue
        errors.extend(f"{path}: {error}" for error in manifest_count_errors(content))
        if content_bytes != expected_bytes:
            errors.append(f"manifest drift: {path}")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    hashed = next(
        line.split(": ", 1)[1]
        for line in expected.splitlines()
        if line.startswith("Hashed files:")
    )
    print(f"manifest verified: {hashed} hashed files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
