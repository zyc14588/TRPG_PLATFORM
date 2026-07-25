#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

from repo_truth import ROOT, cargo_targets, git_files, git_modes
from validate_workflows import validate as validate_workflows


RAW_STRING_START = re.compile(r'(?:br|r)(?P<hashes>#{0,255})"')
CHARACTER_LITERAL = re.compile(
    r"""(?:b)?'(?:[^\\'\n]|\\(?:[nrt0\\'"]|x[0-9A-Fa-f]{2}|u\{[0-9A-Fa-f_]{1,6}\}))'"""
)


def _blank_rust_non_code(source: str) -> str:
    """Blank comments and literals while preserving byte offsets and newlines."""
    output = list(source)
    index = 0
    length = len(source)

    def blank(start: int, end: int) -> None:
        for position in range(start, end):
            if output[position] != "\n":
                output[position] = " "

    while index < length:
        if source.startswith("//", index):
            end = source.find("\n", index + 2)
            end = length if end == -1 else end
            blank(index, end)
            index = end
            continue
        if source.startswith("/*", index):
            depth = 1
            end = index + 2
            while end < length and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            blank(index, end)
            index = end
            continue

        raw = RAW_STRING_START.match(source, index)
        if raw:
            terminator = '"' + raw.group("hashes")
            content_start = raw.end()
            end_marker = source.find(terminator, content_start)
            end = length if end_marker == -1 else end_marker + len(terminator)
            blank(index, end)
            index = end
            continue

        prefix_length = 2 if source.startswith('b"', index) else 1
        if source[index] == '"' or prefix_length == 2:
            end = index + prefix_length
            escaped = False
            while end < length:
                character = source[end]
                end += 1
                if escaped:
                    escaped = False
                elif character == "\\":
                    escaped = True
                elif character == '"':
                    break
            blank(index, end)
            index = end
            continue

        character_match = CHARACTER_LITERAL.match(source, index)
        if character_match:
            end = character_match.end()
            blank(index, end)
            index = end
            continue
        index += 1
    return "".join(output)


def _matching_brace(source: str, opening: int) -> int | None:
    depth = 0
    for index in range(opening, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return index
    return None


def _test_function_bodies(source: str) -> list[tuple[str, str]]:
    sanitized = _blank_rust_non_code(source)
    attribute = re.compile(
        r"#\s*\[\s*(?:tokio\s*::\s*)?test(?:\s*\([^\]]*\))?\s*\]"
    )
    function = re.compile(
        r"\s*(?:#\s*\[[^\]]*\]\s*)*"
        r"(?:pub(?:\s*\([^)]*\))?\s+)?(?:async\s+)?"
        r"fn\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)[^{;]*\{"
    )
    bodies: list[tuple[str, str]] = []
    for annotation in attribute.finditer(sanitized):
        declaration = function.match(sanitized, annotation.end())
        if declaration is None:
            continue
        opening = declaration.end() - 1
        closing = _matching_brace(sanitized, opening)
        if closing is not None:
            bodies.append(
                (declaration.group("name"), sanitized[opening + 1 : closing])
            )
    return bodies


def _environment_control_statement(body: str, call_start: int) -> str:
    start = call_start
    expected_openings: list[str] = []
    while start > 0:
        start -= 1
        character = body[start]
        if character == ")":
            expected_openings.append("(")
        elif character == "]":
            expected_openings.append("[")
        elif character == "}":
            # A top-level closing block belongs to the preceding sibling
            # statement. Only cross it when an unmatched parenthesized or
            # bracketed expression already proves that the block is nested in
            # the current statement.
            if expected_openings:
                expected_openings.append("{")
            else:
                start += 1
                break
        elif character in "({[":
            if expected_openings and expected_openings[-1] == character:
                expected_openings.pop()
            else:
                start += 1
                break
        elif not expected_openings and character == ";":
            start += 1
            break

    curly = round_bracket = square = 0
    index = start
    saw_control_block = False
    while index < len(body):
        character = body[index]
        if character == "{":
            curly += 1
            saw_control_block = True
        elif character == "}":
            curly -= 1
            if curly < 0:
                return body[start:index]
            if curly == 0 and saw_control_block:
                tail = body[index + 1 :]
                continuation = re.match(r"\s*(?:else\b|[.;?])", tail)
                if continuation is None:
                    return body[start : index + 1]
        elif character == "(":
            round_bracket += 1
        elif character == ")":
            round_bracket -= 1
        elif character == "[":
            square += 1
        elif character == "]":
            square -= 1
        elif (
            character == ";"
            and curly == 0
            and round_bracket == 0
            and square == 0
        ):
            return body[start : index + 1]
        index += 1
    return body[start:]


def integration_test_silent_env_successes(source: str) -> list[str]:
    """Return test names whose missing-env control path can report success."""
    silent_tests: list[str] = []
    env_call = re.compile(r"(?:std\s*::\s*)?env\s*::\s*var\s*\(")
    silent_return = re.compile(
        r"\breturn\s*(?:;|(?=[,}])|Ok\s*\(\s*\(\s*\)\s*\)\s*;?)"
    )
    for test_name, body in _test_function_bodies(source):
        for call in env_call.finditer(body):
            statement = _environment_control_statement(body, call.start())
            if silent_return.search(statement):
                silent_tests.append(test_name)
                break
    return silent_tests


def inventory(root: Path = ROOT) -> tuple[dict, list[str]]:
    files = git_files(root)
    fixtures = [path for path in files if path.startswith("fixtures/") and path != "fixtures/README.md"]
    reference_text = []
    reference_roots = ("crates/", "scripts/", "stages/", "policy/", ".github/")
    for name in files:
        if name in fixtures or not (name.startswith(reference_roots) or "/" not in name):
            continue
        path = root / name
        if path.is_file() and path.stat().st_size < 1_000_000:
            try:
                reference_text.append(path.read_text(encoding="utf-8"))
            except UnicodeDecodeError:
                pass
    corpus = "\n".join(reference_text)
    orphans = [name for name in fixtures if name not in corpus and Path(name).name not in corpus]
    allowlist_path = root / "scripts/ci/fixture_allowlist.json"
    allowed = {}
    errors = validate_workflows(root)
    if allowlist_path.is_file():
        entries = json.loads(allowlist_path.read_text(encoding="utf-8"))
        for entry in entries:
            if not all(entry.get(field) for field in ("path", "reason", "owner")):
                errors.append("fixture allowlist entries require path, reason, and owner")
            else:
                allowed[entry["path"]] = entry
    errors.extend(f"orphan fixture: {name}" for name in orphans if name not in allowed)
    errors.extend(f"stale fixture allowlist entry: {name}" for name in allowed if name not in orphans)
    for name in files:
        if (
            not name.startswith(("crates/", "apps/"))
            or "/tests/" not in name
            or not name.endswith(".rs")
        ):
            continue
        test_path = root / name
        if not test_path.is_file():
            errors.append(f"indexed integration test path is missing: {name}")
            continue
        source = test_path.read_text(encoding="utf-8")
        silent_tests = integration_test_silent_env_successes(source)
        if silent_tests:
            errors.append(
                "integration test can silently pass after missing environment configuration: "
                + name
                + " ("
                + ", ".join(silent_tests)
                + ")"
            )
    package = json.loads((root / "package.json").read_text(encoding="utf-8"))
    report = {
        "rust_test_targets": sorted(cargo_targets(root, "test")),
        "node_scripts": package.get("scripts", {}),
        "opa_tests": sorted(path for path in files if path.startswith("policy/opa/") and path.endswith("_test.rego")),
        "powershell": sorted(path for path in files if path.endswith(".ps1")),
        "shell": sorted(path for path in files if path.endswith(".sh")),
        "fixtures": fixtures,
        "orphan_fixtures": orphans,
        "workflows": sorted(path for path in files if path.startswith(".github/workflows/") and path.endswith(".yml")),
    }
    for key in ("rust_test_targets", "node_scripts", "opa_tests", "powershell", "shell", "fixtures", "workflows"):
        if not report[key]:
            errors.append(f"empty test inventory category: {key}")
    workflow_text = "\n".join((root / path).read_text(encoding="utf-8") for path in report["workflows"])
    ci_text = workflow_text + "\n" + (root / "scripts/ci/test-all.sh").read_text(encoding="utf-8")
    for script in report["powershell"]:
        if script not in ci_text:
            errors.append(f"PowerShell script is not referenced by CI: {script}")
    modes = git_modes(root)
    for script in (
        "scripts/ci/init-smoke.sh",
        "scripts/ci/test-all.sh",
        "scripts/ci/integration-services.sh",
        "scripts/ci/generate-integration-evidence.sh",
        "scripts/ci/production-security-smoke.sh",
        "scripts/backup_restore/smoke.sh",
        "scripts/projection_rebuild/verify.sh",
    ):
        if modes.get(script) != "100755":
            errors.append(f"CI shell script is not executable in Git: {script}")
    return report, errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    report, errors = inventory()
    payload = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(payload, encoding="utf-8")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(payload, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
