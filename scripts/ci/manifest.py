#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from repo_truth import MANIFEST_OUTPUTS, ROOT, git_blob_bytes, git_files, git_modes


OUTPUTS = tuple(sorted(MANIFEST_OUTPUTS))


def render(root: Path = ROOT) -> str:
    all_files = git_files(root)
    files = [path for path in all_files if path not in MANIFEST_OUTPUTS]
    blobs = git_blob_bytes(root)
    modes = git_modes(root)
    lines = [
        "# Repository Source Manifest v1",
        "",
        f"Repository files: {len(all_files)}",
        f"Hashed files: {len(files)}",
        "",
        "The three generated manifest outputs are excluded from their own hash set to avoid self-reference.",
        "Their paths remain fixed and `verify_manifest.py` requires all three outputs to be byte-identical.",
        "",
        "| path | size_bytes | sha256 | git_mode |",
        "|---|---:|---|---|",
    ]
    for name in files:
        content = blobs[name] if name in blobs else (root / name).read_bytes()
        digest = hashlib.sha256(content).hexdigest()
        lines.append(f"| `{name}` | {len(content)} | `{digest}` | `{modes.get(name, '100644')}` |")
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    content = render()
    if args.write:
        for name in OUTPUTS:
            path = ROOT / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8", newline="\n")
    else:
        print(content, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
