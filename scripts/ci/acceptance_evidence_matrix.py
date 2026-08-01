#!/usr/bin/env python3
"""Generate and verify the V1 acceptance matrix from external evidence."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from acceptance_evidence_matrix_core import (
    ROOT,
    canonical_manifest_text,
    create_not_run_manifest,
    external_output_errors,
    incomplete_reasons,
    release_candidate_errors,
    render_matrix,
    sha256_file,
)
from acceptance_evidence_matrix_validation import (
    load_manifest,
    manifest_errors,
    matrix_file_errors,
)


def _write_external(path: Path, text: str, root: Path, label: str) -> None:
    errors = external_output_errors(path, root, label)
    if errors:
        raise ValueError("\n".join(errors))
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    create = subparsers.add_parser("create-not-run")
    create.add_argument("--manifest", type=Path, required=True)
    create.add_argument("--evidence", type=Path, action="append", required=True)

    generate = subparsers.add_parser("generate")
    generate.add_argument("--manifest", type=Path, required=True)
    generate.add_argument("--output", type=Path, required=True)

    validate = subparsers.add_parser("validate")
    validate.add_argument("--manifest", type=Path, required=True)
    validate.add_argument("--matrix", type=Path, required=True)
    validate.add_argument("--require-ready", action="store_true")
    args = parser.parse_args()

    try:
        if args.command == "create-not-run":
            data = create_not_run_manifest(args.evidence, args.manifest)
            _write_external(
                args.manifest,
                canonical_manifest_text(data),
                ROOT,
                "acceptance evidence manifest",
            )
            print(f"acceptance evidence manifest created: {args.manifest}")
            return 0
        if args.command == "generate":
            data, errors = load_manifest(args.manifest)
            errors.extend(
                external_output_errors(args.output, ROOT, "generated acceptance matrix")
            )
            if errors or data is None:
                print("\n".join(errors), file=sys.stderr)
                return 1
            _write_external(
                args.output,
                render_matrix(data, sha256_file(args.manifest)),
                ROOT,
                "generated acceptance matrix",
            )
            print(f"acceptance matrix generated: {args.output}")
            return 0
        data, errors = matrix_file_errors(args.manifest, args.matrix)
        if args.require_ready and data is not None and not errors:
            errors.extend(release_candidate_errors(data))
        if errors:
            print("\n".join(errors), file=sys.stderr)
            return 1
        print("acceptance evidence matrix verified")
        return 0
    except (OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
