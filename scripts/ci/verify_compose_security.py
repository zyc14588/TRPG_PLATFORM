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


def errors(root: Path = ROOT) -> list[str]:
    compose_path = root / "compose.yml"
    override_path = root / "docker-compose.ci.yml"
    dockerfile_path = root / "Dockerfile"
    workflow_path = root / ".github/workflows/docker-compose-smoke.yml"
    smoke_path = root / "scripts/ci/production-security-smoke.sh"
    try:
        compose = compose_path.read_text(encoding="utf-8")
        override = override_path.read_text(encoding="utf-8")
        dockerfile = dockerfile_path.read_text(encoding="utf-8")
        workflow = workflow_path.read_text(encoding="utf-8")
        smoke = smoke_path.read_text(encoding="utf-8")
    except OSError as error:
        return [str(error)]

    found: list[str] = []
    if not re.search(r"(?m)^name:\s+coc-ai-trpg\s*$", compose):
        found.append("Compose project name must use the stable current-safe name")

    service_blocks = mapping_blocks(section(compose, "services"))
    network_blocks = mapping_blocks(section(compose, "networks"))
    for service in sorted(REQUIRED_PRODUCT_SERVICES):
        if service not in service_blocks:
            found.append(f"production Compose misses product service: {service}")
    for service in sorted(DATABASE_CLIENT_SERVICES):
        block = service_blocks.get(service, "")
        if not re.search(
            r"(?m)^\s+- postgres_ca_certificate\s*$",
            block,
        ):
            found.append(
                f"database client does not mount the PostgreSQL trust anchor: {service}"
            )
    backend_network = network_blocks.get("backend", "")
    if not re.search(r"(?m)^\s+internal:\s+true\s*$", backend_network):
        found.append("PostgreSQL backend network must remain internal")
    for service in ("postgres", "postgres-witness"):
        block = service_blocks.get(service, "")
        if not re.search(
            r"(?m)^\s{4}networks:\s*\n\s{6}- backend\s*$",
            block,
        ):
            found.append(
                f"{service} must be attached only to the internal backend network"
            )
        if re.search(r"(?m)^\s{4}ports:\s*$", block):
            found.append(f"production Compose must not publish {service} ports")
    for service in sorted(PINNED_INFRASTRUCTURE_SERVICES):
        block = service_blocks.get(service, "")
        image = re.search(r"(?m)^\s+image:\s*(\S+)", block)
        if image is None or "@sha256:" not in image.group(1):
            found.append(f"infrastructure image is not digest pinned: {service}")

    declared_stages: set[str] = set()
    for instruction in re.findall(r"(?mi)^FROM\s+(.+)$", dockerfile):
        tokens = instruction.split()
        image_index = next(
            (index for index, token in enumerate(tokens) if not token.startswith("--")),
            None,
        )
        if image_index is None:
            found.append("Dockerfile has an invalid FROM instruction")
            continue
        image = tokens[image_index]
        if image not in declared_stages and "@sha256:" not in image:
            found.append(f"Dockerfile base image is not digest pinned: {image}")
        remaining = tokens[image_index + 1 :]
        if len(remaining) == 2 and remaining[0].lower() == "as":
            declared_stages.add(remaining[1])

    production_secrets = mapping_blocks(section(compose, "secrets"))
    override_secrets = mapping_blocks(section(override, "secrets"))
    override_networks = mapping_blocks(section(override, "networks"))
    if not re.search(
        r"(?m)^\s+internal:\s+false\s*$",
        override_networks.get("backend", ""),
    ):
        found.append(
            "runtime override must expose its isolated backend for loopback TLS probes"
        )
    if not production_secrets:
        found.append("production Compose has no external secrets")
    for name, block in sorted(production_secrets.items()):
        if not re.search(r"(?m)^\s+external:\s+true\s*$", block):
            found.append(f"production secret is not external: {name}")
        if re.search(r"(?m)^\s+(?:file|environment):", block):
            found.append(f"production secret embeds a local source: {name}")
    if set(override_secrets) != set(production_secrets):
        missing = sorted(set(production_secrets) - set(override_secrets))
        extra = sorted(set(override_secrets) - set(production_secrets))
        if missing:
            found.append("runtime override misses secrets: " + ", ".join(missing))
        if extra:
            found.append("runtime override has unknown secrets: " + ", ".join(extra))
    for name, block in sorted(override_secrets.items()):
        if not re.search(r"(?m)^\s+external:\s+false\s*$", block):
            found.append(f"runtime secret override does not disable external lookup: {name}")
        if not re.search(
            rf"(?m)^\s+file:\s+\$\{{TRPG_COMPOSE_SECRET_DIRECTORY:[^}}]+}}/{re.escape(name)}\s*$",
            block,
        ):
            found.append(f"runtime secret override has an unexpected source: {name}")

    required_security_fragments = (
        "hostnossl all           all",
        "tls-auth-clients yes",
        "verify: true",
        'handshake_first: true',
        "ssl_min_protocol_version",
        "TRPG_REDIS_CLIENT_CERT_PATH",
        "TRPG_NATS_CLIENT_CERT_PATH",
        "MINIO_ROOT_PASSWORD_FILE",
    )
    security_corpus = "\n".join(
        [
            compose,
            (root / "config/postgres/pg_hba.conf").read_text(encoding="utf-8"),
            (root / "config/postgres/witness_pg_hba.conf").read_text(encoding="utf-8"),
            (root / "config/redis/redis.conf").read_text(encoding="utf-8"),
            (root / "config/nats/nats.conf").read_text(encoding="utf-8"),
        ]
    )
    for fragment in required_security_fragments:
        if fragment not in security_corpus:
            found.append(f"Compose security boundary is missing: {fragment}")
    primary_hba = (root / "config/postgres/pg_hba.conf").read_text(encoding="utf-8")
    witness_hba = (root / "config/postgres/witness_pg_hba.conf").read_text(
        encoding="utf-8"
    )
    for service, hba in (
        ("primary", primary_hba),
        ("witness", witness_hba),
    ):
        if not re.search(
            r"(?m)^local\s+all\s+all\s+scram-sha-256\s*$",
            hba,
        ):
            found.append(
                f"PostgreSQL {service} local socket authentication must use SCRAM"
            )
    if not re.search(
        r"(?m)^hostssl\s+coc_ai_trpg\s+trpg_database_owner\s+samenet\s+scram-sha-256\s*$",
        primary_hba,
    ):
        found.append("PostgreSQL owner is not limited to its database and backend network")
    if not re.search(
        r"(?m)^hostssl\s+coc_ai_trpg_witness\s+trpg_witness_owner\s+samenet\s+scram-sha-256\s*$",
        witness_hba,
    ):
        found.append("PostgreSQL witness owner is not limited to its database and backend network")
    if re.search(
        r"(?m)^(?:host|hostssl)\s+all\s+(?:trpg_database_owner|trpg_witness_owner)\s+(?:0\.0\.0\.0/0|::/0)",
        primary_hba + "\n" + witness_hba,
    ):
        found.append("PostgreSQL owner access still accepts unrestricted routed addresses")

    if "pull_request:" not in workflow:
        found.append("production security workflow does not run on pull requests")
    if "production-security-smoke.sh" not in workflow:
        found.append("production security workflow does not execute the runtime smoke")
    required_runtime_fragments = (
        '"${compose_command[@]}" build api web',
        "up --detach --no-build --wait --wait-timeout 600",
        "/api/health/ready",
        "/realtime/health/ready",
        "/admin/health/ready",
        "docker secret create",
        "docker service update",
        "MinIO certificate fingerprint did not change after rotation",
        "--write-out '%{http_code} %{redirect_url}'",
        '[[ "$plaintext_proxy_status" != 308\\ https://* ]]',
    )
    for fragment in required_runtime_fragments:
        if fragment not in smoke:
            found.append(f"production runtime smoke is incomplete: {fragment}")
    if "runtime-smoke-infrastructure-only" in smoke:
        found.append("production runtime smoke still uses a placeholder NATS credential")
    entrypoint = (root / "config/container/trpg-entrypoint.sh").read_text(
        encoding="utf-8"
    )
    security_manifest = (
        root / "crates/trpg-security-governance/Cargo.toml"
    ).read_text(encoding="utf-8")
    if "minio_tls_ca_certificate" not in entrypoint or "SSL_CERT_FILE" not in entrypoint:
        found.append("runtime does not trust the mounted MinIO CA without disabling TLS")
    required_secret_staging_fragments = (
        "private_secret_mount=/tmp/trpg-mounted-secrets",
        "install -d -o trpg -g trpg -m 0700",
        "install -o trpg -g trpg -m 0400",
        'export TRPG_SECRET_MOUNT="$private_secret_mount"',
    )
    for fragment in required_secret_staging_fragments:
        if fragment not in entrypoint:
            found.append(
                "runtime does not stage Compose/Docker secrets into a private "
                f"non-root mount: {fragment}"
            )
    if "tokio-native-tls" not in security_manifest:
        found.append("object-store client cannot consume the mounted native CA bundle")
    return found


def main() -> int:
    if sys.argv[1:] != ["--check"]:
        print("usage: verify_compose_security.py --check", file=sys.stderr)
        return 2
    found = errors()
    if found:
        print("\n".join(found), file=sys.stderr)
        return 1
    print("production Compose security contract verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
