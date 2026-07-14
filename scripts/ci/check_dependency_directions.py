#!/usr/bin/env python3
"""Enforce the P01 Cargo ports-and-adapters dependency boundary."""

from __future__ import annotations

import argparse
import sys
import tomllib
from pathlib import Path


ALLOWED_NORMAL_DEPENDENCIES: dict[str, set[str]] = {
    "trpg-contracts": set(),
    "trpg-shared-kernel": {"trpg-contracts"},
    "trpg-identity": {"trpg-shared-kernel"},
    "trpg-domain-core": {"trpg-contracts", "trpg-shared-kernel"},
    "trpg-runtime": {"trpg-contracts", "trpg-shared-kernel"},
    "trpg-agent-runtime": {
        "trpg-contracts",
        "trpg-identity",
        "trpg-shared-kernel",
    },
    "trpg-ruleset-coc7": {"trpg-contracts", "trpg-shared-kernel"},
    "trpg-data-eventing": {"trpg-contracts", "trpg-shared-kernel"},
    "trpg-api": {"trpg-contracts", "trpg-shared-kernel"},
    "trpg-platform": {"trpg-contracts", "trpg-shared-kernel"},
    "trpg-security-governance": {"trpg-contracts", "trpg-shared-kernel"},
    "trpg-ops": {"trpg-contracts", "trpg-shared-kernel"},
    "trpg-extension-sdk": {"trpg-contracts", "trpg-shared-kernel"},
    "trpg-test-support": {
        "trpg-contracts",
        "trpg-identity",
        "trpg-shared-kernel",
    },
    "trpg-testing": {
        "trpg-agent-runtime",
        "trpg-contracts",
        "trpg-shared-kernel",
        "trpg-test-support",
    },
    "api-server": {
        "trpg-api",
        "trpg-contracts",
        "trpg-identity",
        "trpg-shared-kernel",
    },
    "realtime-server": {"trpg-api", "trpg-contracts"},
    "agent-worker": {"trpg-agent-runtime", "trpg-contracts"},
    "admin-server": {"trpg-contracts", "trpg-platform"},
    "migration-runner": {"trpg-contracts", "trpg-data-eventing"},
}

PRODUCT_BINARIES = {
    "api-server",
    "realtime-server",
    "agent-worker",
    "admin-server",
    "migration-runner",
}

PRODUCTION_DEPENDENCY_SECTIONS = ("dependencies", "build-dependencies")


def load_manifest(path: Path) -> dict:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def resolve_dependency_name(
    alias: str, specification: object, workspace_dependencies: dict
) -> str:
    if not isinstance(specification, dict):
        return alias
    package_name = specification.get("package")
    if isinstance(package_name, str):
        return package_name
    if specification.get("workspace") is True:
        workspace_specification = workspace_dependencies.get(alias)
        if workspace_specification is not None:
            return resolve_dependency_name(
                alias, workspace_specification, workspace_dependencies
            )
    return alias


def dependency_tables(manifest: dict, sections: tuple[str, ...]):
    for section in sections:
        yield section, manifest.get(section, {})
    for target_name, target_configuration in manifest.get("target", {}).items():
        if not isinstance(target_configuration, dict):
            continue
        for section in sections:
            yield (
                f"target.{target_name}.{section}",
                target_configuration.get(section, {}),
            )


def validate_workspace(root: Path) -> list[str]:
    root_manifest = load_manifest(root / "Cargo.toml")
    member_paths = root_manifest.get("workspace", {}).get("members", [])
    workspace_dependencies = root_manifest.get("workspace", {}).get("dependencies", {})
    manifests: dict[str, tuple[Path, dict]] = {}
    errors: list[str] = []

    for member in member_paths:
        manifest_path = root / member / "Cargo.toml"
        if not manifest_path.is_file():
            errors.append(f"workspace member has no manifest: {member}")
            continue
        manifest = load_manifest(manifest_path)
        package_name = manifest.get("package", {}).get("name")
        if not package_name:
            errors.append(f"workspace member has no package name: {member}")
            continue
        if package_name in manifests:
            errors.append(f"duplicate workspace package name: {package_name}")
            continue
        manifests[package_name] = (manifest_path, manifest)

    workspace_packages = set(manifests)
    unknown = workspace_packages - ALLOWED_NORMAL_DEPENDENCIES.keys()
    if unknown:
        errors.append(f"packages missing dependency policy: {sorted(unknown)}")

    missing_binaries = PRODUCT_BINARIES - workspace_packages
    if missing_binaries:
        errors.append(f"missing product binaries: {sorted(missing_binaries)}")

    for package_name, (manifest_path, manifest) in manifests.items():
        allowed = ALLOWED_NORMAL_DEPENDENCIES.get(package_name, set())
        for section, dependencies in dependency_tables(
            manifest, PRODUCTION_DEPENDENCY_SECTIONS
        ):
            for alias, specification in dependencies.items():
                dependency_name = resolve_dependency_name(
                    alias, specification, workspace_dependencies
                )
                if (
                    dependency_name in workspace_packages
                    and dependency_name not in allowed
                ):
                    errors.append(
                        f"forbidden dependency: {package_name} -> {dependency_name} "
                        f"({manifest_path.relative_to(root)} [{section}] via {alias})"
                    )

        for _, dev_dependencies in dependency_tables(manifest, ("dev-dependencies",)):
            for alias, specification in dev_dependencies.items():
                dependency_name = resolve_dependency_name(
                    alias, specification, workspace_dependencies
                )
                if dependency_name in PRODUCT_BINARIES:
                    errors.append(
                        f"tests must not depend on product binary: "
                        f"{package_name} -> {dependency_name}"
                    )

        if package_name in PRODUCT_BINARIES:
            main_path = manifest_path.parent / "src" / "main.rs"
            if not main_path.is_file():
                errors.append(f"product binary has no main.rs: {package_name}")

    for package_name, (_, manifest) in manifests.items():
        if package_name in {"trpg-testing", "trpg-test-support"}:
            continue
        for section, dependencies in dependency_tables(
            manifest, PRODUCTION_DEPENDENCY_SECTIONS
        ):
            for alias, specification in dependencies.items():
                dependency_name = resolve_dependency_name(
                    alias, specification, workspace_dependencies
                )
                if dependency_name == "trpg-test-support":
                    errors.append(
                        f"test-support is a production dependency of {package_name} "
                        f"([{section}] via {alias})"
                    )

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    errors = validate_workspace(args.root.resolve())
    if errors:
        for error in errors:
            print(f"DEPENDENCY_DIRECTION_ERROR: {error}", file=sys.stderr)
        return 1
    print("dependency directions: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
