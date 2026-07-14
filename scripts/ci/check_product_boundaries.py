#!/usr/bin/env python3
"""Reject construction metadata, fixture APIs, and duplicate production modules."""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
from collections import defaultdict
from pathlib import Path


CONSTRUCTION_METADATA = re.compile(
    r"CODEX-[0-9]|\bPROMPT_ID\b|\bprompt_id\b|"
    r"(?i:(?:^|[^A-Za-z0-9])batch[_-]?[0-9]{3}(?:[^A-Za-z0-9]|$))|"
    r"(?i:(?:^|[^A-Za-z0-9])stage_fixture(?:[^A-Za-z0-9]|$))|"
    r"ExpectedFixtureContract\b|"
    r"evidence/batches/"
)
PRODUCTION_FIXTURE_FACTORY = re.compile(
    r"CommandEnvelope\s*::\s*governed|"
    r"pub\s+fn\s+[A-Za-z_][A-Za-z0-9_]*fixture[A-Za-z0-9_]*\s*\("
)
UNIT_SERVICE = re.compile(
    r"pub\s+struct\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*(?:Service|Repository))\s*;"
)

PUBLIC_IMPL_COMPATIBILITY_MODULES = {
    "trpg-agent-runtime:agent_runtime_impl",
    "trpg-agent-runtime:evaluation_golden_scenario_impl",
    "trpg-agent-runtime:memory_rag_impl",
    "trpg-agent-runtime:model_provider_local_cloud_impl",
    "trpg-agent-runtime:rag_snapshot_impl",
    "trpg-data-eventing:cache_redis_impl",
    "trpg-data-eventing:event_bus_nats_impl",
    "trpg-data-eventing:persistence_postgresql_impl",
    "trpg-domain-core:authority_contract_impl",
    "trpg-domain-core:character_combat_san_chase_impl",
    "trpg-domain-core:command_cqrs_impl",
    "trpg-domain-core:domain_model_impl",
    "trpg-domain-core:event_sourcing_projection_impl",
    "trpg-domain-core:investigation_clue_npc_time_impl",
    "trpg-domain-core:rule_runtime_coc7_impl",
    "trpg-domain-core:visibility_fact_provenance_impl",
    "trpg-ops:upgrade_rollback_impl",
    "trpg-platform:api_contracts_impl",
    "trpg-platform:deployment_ops_impl",
    "trpg-platform:observability_impl",
    "trpg-platform:plugin_sdk_impl",
    "trpg-platform:policy_authz_impl",
    "trpg-platform:reliability_performance_impl",
    "trpg-runtime:capability_layer_impl",
    "trpg-runtime:pending_decision_impl",
    "trpg-runtime:realtime_room_sync_impl",
    "trpg-runtime:saga_transaction_impl",
    "trpg-runtime:scheduler_service_impl",
    "trpg-runtime:session_runtime_impl",
    "trpg-runtime:workflow_engine_impl",
    "trpg-shared-kernel:cargo_workspace_impl",
    "trpg-shared-kernel:constitution_impl",
    "trpg-shared-kernel:document_set_impl",
    "trpg-shared-kernel:open_source_reference_matrix_impl",
    "trpg-shared-kernel:system_context_impl",
    "trpg-shared-kernel:technology_selection_rust_impl",
    "trpg-testing:golden_scenarios_ci_impl",
    "trpg-testing:test_strategy_impl",
}


def production_sources(root: Path) -> list[Path]:
    sources: list[Path] = []
    for source_root in sorted((root / "crates").glob("*/src")):
        if source_root.parent.name == "trpg-testing":
            continue
        sources.extend(source_root.rglob("*.rs"))
    for source_root in sorted((root / "apps").glob("*/src")):
        sources.extend(source_root.rglob("*.rs"))
    return sorted(path for path in sources if path.is_file())


def validate_product_boundaries(root: Path) -> list[str]:
    errors: list[str] = []
    hashes: dict[str, list[Path]] = defaultdict(list)

    for path in production_sources(root):
        content = path.read_text(encoding="utf-8")
        relative_path = path.relative_to(root)
        for pattern, message in (
            (CONSTRUCTION_METADATA, "construction prompt metadata in production source"),
            (PRODUCTION_FIXTURE_FACTORY, "fixed fixture command factory in production source"),
        ):
            match = pattern.search(content)
            if match:
                line = content.count("\n", 0, match.start()) + 1
                errors.append(f"{message}: {relative_path}:{line}")

        for match in UNIT_SERVICE.finditer(content):
            service_name = match.group("name")
            behavior = re.search(
                rf"impl\s+{re.escape(service_name)}\s*\{{.*?pub\s+fn\s+",
                content,
                re.DOTALL,
            )
            if behavior is None:
                line = content.count("\n", 0, match.start()) + 1
                errors.append(
                    f"empty Service/Repository unit struct: {relative_path}:{line}"
                )

        if content.strip():
            digest = hashlib.sha256(content.encode("utf-8")).hexdigest()
            hashes[digest].append(relative_path)

    for duplicate_paths in hashes.values():
        if len(duplicate_paths) > 1:
            rendered = ", ".join(str(path) for path in duplicate_paths)
            errors.append(f"byte-identical production Rust modules: {rendered}")

    if (root / "Cargo.toml").is_file():
        discovered_modules: set[str] = set()
        for lib_path in sorted((root / "crates").glob("*/src/lib.rs")):
            crate_name = lib_path.parents[1].name
            content = lib_path.read_text(encoding="utf-8")
            for module_name in re.findall(
                r"^\s*pub\s+mod\s+([A-Za-z_][A-Za-z0-9_]*_impl)\b",
                content,
                re.MULTILINE,
            ):
                discovered_modules.add(f"{crate_name}:{module_name}")
        for module in sorted(discovered_modules - PUBLIC_IMPL_COMPATIBILITY_MODULES):
            errors.append(f"unlisted public compatibility module: {module}")
        for module in sorted(PUBLIC_IMPL_COMPATIBILITY_MODULES - discovered_modules):
            errors.append(f"missing public compatibility module: {module}")

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    errors = validate_product_boundaries(args.root.resolve())
    if errors:
        for error in errors:
            print(f"PRODUCT_BOUNDARY_ERROR: {error}", file=sys.stderr)
        return 1
    print("product source boundaries: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
