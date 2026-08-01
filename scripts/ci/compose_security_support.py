#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
from pathlib import Path

from repo_truth import ROOT


PINNED_INFRASTRUCTURE_SERVICES = {
    "openfga-migrate",
    "openfga",
    "opa",
    "policy-bootstrap",
    "postgres",
    "postgres-witness",
    "witness-role-bootstrap",
    "redis",
    "nats",
    "minio",
    "minio-init",
    "reverse-proxy",
}
REQUIRED_PRODUCT_SERVICES = {
    "web",
    "api",
    "realtime",
    "agent-worker",
    "admin",
    "migration-runner",
}
DATABASE_CLIENT_SERVICES = {
    "api",
    "realtime",
    "agent-worker",
    "migration-runner",
}
WITNESS_DATABASE_SECRET_BY_SERVICE = {
    "api": "witness_append_database_url",
    "realtime": "witness_read_database_url",
    # The Agent Job worker is a formal canonical writer. It receives only the
    # immutable append role, never witness owner or mutable table privileges.
    "agent-worker": "witness_append_database_url",
    "migration-runner": "witness_owner_database_url",
}
REQUIRED_RUNTIME_SMOKE_FRAGMENTS = (
    '"${compose_command[@]}" build api web',
    "up --detach --no-build --wait --wait-timeout 600",
    "/api/health/ready",
    "/realtime/health/ready",
    "/admin/health/ready",
    "docker secret create",
    "docker service update",
    "MinIO certificate fingerprint did not change after rotation",
    "MinIO accepted an unrelated private CA",
    "MinIO TLS accepted the wrong hostname",
    "ar02_live_s3_version_erasure_closes_recoverable_history",
    "object-storage service identity crossed the bucket boundary",
    "object-storage service identity wrote outside subjects/",
    "--write-out '%{http_code} %{redirect_url}'",
    '[[ "$plaintext_proxy_status" != 308\\ https://* ]]',
    '"${compose_command[@]}" run --rm witness-role-bootstrap',
    "GRANT trpg_witness_owner TO trpg_witness_append_login",
    "ALTER ROLE trpg_witness_append_login CREATEDB CREATEROLE BYPASSRLS",
    'expected_append_privileges="trpg_witness_append_login|t|f|f|t|f|t|t|f|f|f|f|t|f|f|f|f|f|f"',
    'expected_read_privileges="trpg_witness_read_login|t|f|f|t|f|t|f|f|f|f|f|f|t|f|f|f|f|f"',
    "has_table_privilege(current_user, 'public.external_audit_witness', 'TRUNCATE')",
    'witness_query trpg_witness_read_login "$postgres_witness_read_password"',
    '"$postgres_witness_append_password" drop-table',
    '"$postgres_witness_append_password" create-table',
    'trpg_witness_read_login "$postgres_witness_read_password" insert',
    "production security smoke: full product graph, witness least privilege",
)


def section(text: str, name: str) -> str:
    match = re.search(rf"(?m)^{re.escape(name)}:\s*$", text)
    if match is None:
        return ""
    end = re.search(r"(?m)^[a-zA-Z0-9_-]+:\s*$", text[match.end() :])
    return text[match.end() : match.end() + end.start()] if end else text[match.end() :]


def mapping_blocks(text: str) -> dict[str, str]:
    matches = list(re.finditer(r"(?m)^  ([a-zA-Z0-9_-]+):\s*$", text))
    return {
        match.group(1): text[
            match.end() : matches[index + 1].start()
            if index + 1 < len(matches)
            else len(text)
        ]
        for index, match in enumerate(matches)
    }


def shell_bundle(root: Path, relative: str) -> str:
    """Read a Bash entrypoint and its ordered same-stem phase directory."""
    entrypoint = root / relative
    phase_paths = sorted(entrypoint.with_suffix("").glob("*.sh"))
    return "\n".join(
        [
            entrypoint.read_text(encoding="utf-8"),
            *(path.read_text(encoding="utf-8") for path in phase_paths),
        ]
    )


def runtime_secret_staging_errors(root: Path) -> list[str]:
    entrypoint = (root / "config/container/trpg-entrypoint.sh").read_text(
        encoding="utf-8"
    )
    service_process_smoke = shell_bundle(root, "scripts/ci/service-process-smoke.sh")
    security_manifest = (
        root / "crates/trpg-security-governance/Cargo.toml"
    ).read_text(encoding="utf-8")
    found: list[str] = []
    if "minio_tls_ca_certificate" not in entrypoint or "SSL_CERT_FILE" not in entrypoint:
        found.append("runtime does not trust the mounted MinIO CA without disabling TLS")
    for fragment in (
        "P05_MINIO_CA_CERT_PATH",
        "TRPG_OBJECT_STORAGE_CA_CERT_PATH=$object_storage_ca_cert_path",
        'SSL_CERT_FILE=${SSL_CERT_FILE:-$object_storage_ca_cert_path}',
    ):
        if fragment not in service_process_smoke:
            found.append(
                "service process smoke does not bind the private MinIO CA: "
                f"{fragment}"
            )
    for fragment in (
        "private_secret_mount=/tmp/trpg-mounted-secrets",
        "install -d -o trpg -g trpg -m 0700",
        "install -o trpg -g trpg -m 0400",
        'export TRPG_SECRET_MOUNT="$private_secret_mount"',
    ):
        if fragment not in entrypoint:
            found.append(
                "runtime does not stage Compose/Docker secrets into a private "
                f"non-root mount: {fragment}"
            )
    if (
        "aws-sdk-s3" not in security_manifest
        or '"default-https-client"' not in security_manifest
    ):
        found.append(
            "object-store client cannot consume the mounted CA bundle with the "
            "version-aware SDK"
        )
    return found

"""Compose parsing primitives and security-policy constants."""
