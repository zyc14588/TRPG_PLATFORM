#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import sys
from pathlib import Path

from repo_truth import MANIFEST_OUTPUTS, ROOT, git_blob_bytes, git_modes


OUTPUTS = tuple(sorted(MANIFEST_OUTPUTS))


def render(root: Path = ROOT) -> str:
    modes = git_modes(root)
    all_files = sorted(modes)
    files = [path for path in all_files if path not in MANIFEST_OUTPUTS]
    blobs = git_blob_bytes(root)
    lines = [
        "# Repository Source Manifest v1",
        "",
        f"Repository files: {len(all_files)}",
        f"Hashed files: {len(files)}",
        "",
        "All tracked paths are listed. The three generated outputs use a self-reference sentinel instead of an impossible self-hash.",
        "CI evidence binds the manifest artifact hash to `base_commit`, and `verify_manifest.py` requires all three outputs to be byte-identical.",
        "",
        "| path | size_bytes | sha256 | git_mode |",
        "|---|---:|---|---|",
    ]
    for name in all_files:
        if name in MANIFEST_OUTPUTS:
            lines.append(
                f"| `{name}` | self | `evidence-bound-self-reference` | `{modes[name]}` |"
            )
            continue
        content = blobs[name] if name in blobs else (root / name).read_bytes()
        digest = hashlib.sha256(content).hexdigest()
        lines.append(f"| `{name}` | {len(content)} | `{digest}` | `{modes.get(name, '100644')}` |")
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    args = parser.parse_args()
    content = render()
    if args.write:
        for name in OUTPUTS:
            path = ROOT / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8", newline="\n")
    elif args.check:
        errors = [name for name in OUTPUTS if not (ROOT / name).is_file() or (ROOT / name).read_text(encoding="utf-8") != content]
        if errors:
            print("manifest drift: " + ", ".join(errors), file=sys.stderr)
            return 1
        print(f"manifest verified: {len(content.splitlines())} lines")
    else:
        print(content, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
