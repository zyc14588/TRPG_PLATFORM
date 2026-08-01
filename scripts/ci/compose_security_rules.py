from __future__ import annotations

from compose_security_support import *

"""Fail-closed validation rules for the production Compose graph."""

def errors(root: Path = ROOT) -> list[str]:
    compose_path = root / "compose.yml"
    override_path = root / "docker-compose.ci.yml"
    dockerfile_path = root / "Dockerfile"
    workflow_path = root / ".github/workflows/docker-compose-smoke.yml"
    smoke_path = root / "scripts/ci/production-security-smoke.sh"
    smoke_phase_paths = sorted(
        (root / "scripts/ci/production-security-smoke").glob("*.sh")
    )
    try:
        compose = compose_path.read_text(encoding="utf-8")
        override = override_path.read_text(encoding="utf-8")
        dockerfile = dockerfile_path.read_text(encoding="utf-8")
        workflow = workflow_path.read_text(encoding="utf-8")
        smoke_entrypoint = smoke_path.read_text(encoding="utf-8")
        smoke = "\n".join(
            [
                smoke_entrypoint,
                *(path.read_text(encoding="utf-8") for path in smoke_phase_paths),
            ]
        )
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
    for service in ("postgres", "postgres-witness", "witness-role-bootstrap"):
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

    agent_worker_block = service_blocks.get("agent-worker", "")
    for fragment in (
        "TRPG_OBJECT_STORAGE_CA_CERT_PATH: /tmp/trpg-ca-bundle.crt",
        "- minio_tls_ca_certificate",
    ):
        if fragment not in agent_worker_block:
            found.append(f"agent worker object-storage TLS binding is incomplete: {fragment}")
    for forbidden_secret in ("minio_root_user", "minio_root_password"):
        if re.search(
            rf"(?m)^\s+- (?:source:\s+)?{re.escape(forbidden_secret)}\s*$",
            agent_worker_block,
        ):
            found.append(
                f"agent worker receives forbidden MinIO root secret: {forbidden_secret}"
            )

    minio_init_block = service_blocks.get("minio-init", "")
    for fragment in (
        "- object_storage_access_key",
        "- object_storage_secret_key",
        "minio bootstrap refused root application credentials",
        '"s3:GetBucketVersioning"',
        '"s3:ListBucketVersions"',
        '"s3:DeleteObjectVersion"',
        '"arn:aws:s3:::$$runtime_bucket/subjects/*"',
        "admin policy create",
        "admin user add",
        "admin policy attach",
        "TRPG_OBJECT_STORAGE_BUCKET: ${TRPG_OBJECT_STORAGE_BUCKET:-trpg-private-data}",
        'runtime_bucket="$${TRPG_OBJECT_STORAGE_BUCKET:?TRPG_OBJECT_STORAGE_BUCKET is required}"',
        'version enable "trpg/$$runtime_bucket"',
    ):
        if fragment not in minio_init_block:
            found.append(f"MinIO least-privilege bootstrap is incomplete: {fragment}")

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

    witness_secret_names = set(WITNESS_DATABASE_SECRET_BY_SERVICE.values())
    if "witness_database_url" in production_secrets:
        found.append("production Compose still declares the shared witness owner URL")
    for service, expected_secret in WITNESS_DATABASE_SECRET_BY_SERVICE.items():
        block = service_blocks.get(service, "")
        if not re.search(
            rf"(?m)^\s+TRPG_WITNESS_DATABASE_URL_SECRET_ID:\s+{re.escape(expected_secret)}\s*$",
            block,
        ):
            found.append(
                f"{service} does not select its least-privilege witness URL: "
                f"{expected_secret}"
            )
        if not re.search(
            rf"(?m)^\s+- source:\s+{re.escape(expected_secret)}\s*\n"
            rf"\s+target:\s+{re.escape(expected_secret)}\.v1\s*$",
            block,
        ):
            found.append(
                f"{service} does not mount its least-privilege witness URL: "
                f"{expected_secret}"
            )
        for forbidden_secret in sorted(witness_secret_names - {expected_secret}):
            if re.search(
                rf"(?m)^\s+(?:TRPG_WITNESS_DATABASE_URL_SECRET_ID:\s+|- source:\s+)"
                rf"{re.escape(forbidden_secret)}\s*$",
                block,
            ):
                found.append(
                    f"{service} also receives forbidden witness URL: {forbidden_secret}"
                )

    witness_database_block = service_blocks.get("postgres-witness", "")
    for password_secret in (
        "postgres_witness_owner_password",
        "postgres_witness_append_password",
        "postgres_witness_read_password",
    ):
        if not re.search(
            rf"(?m)^\s+- {re.escape(password_secret)}\s*$",
            witness_database_block,
        ):
            found.append(
                f"postgres-witness does not mount role password: {password_secret}"
            )
    if (
        "POSTGRES_PASSWORD_FILE: /run/secrets/postgres_witness_owner_password"
        not in witness_database_block
    ):
        found.append("postgres-witness bootstrap password is not owner-only")

    bootstrap_block = service_blocks.get("witness-role-bootstrap", "")
    required_bootstrap_fragments = (
        "user: postgres",
        "read_only: true",
        "no-new-privileges:true",
        "PGSSLMODE: verify-full",
        "PGSSLROOTCERT: /run/secrets/postgres_ca_certificate",
        "- postgres_witness_owner_password",
        "- postgres_ca_certificate",
        "source: postgres_witness_runtime_roles",
        'entrypoint: ["/bin/sh", "/usr/local/bin/witness-runtime-roles.sh"]',
    )
    for fragment in required_bootstrap_fragments:
        if fragment not in bootstrap_block:
            found.append(f"witness role bootstrap is incomplete: {fragment}")
    for forbidden_secret in (
        "postgres_witness_append_password",
        "postgres_witness_read_password",
        "witness_owner_database_url",
    ):
        if re.search(
            rf"(?m)^\s+- (?:source:\s+)?{re.escape(forbidden_secret)}\s*$",
            bootstrap_block,
        ):
            found.append(
                f"witness role bootstrap directly receives forbidden secret: "
                f"{forbidden_secret}"
            )
    migration_block = service_blocks.get("migration-runner", "")
    if not re.search(
        r"(?m)^\s+witness-role-bootstrap:\s*\n"
        r"\s+condition:\s+service_completed_successfully\s*$",
        migration_block,
    ):
        found.append("migration-runner does not wait for witness role bootstrap")

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
            (root / "config/postgres/witness-runtime-roles.sh").read_text(
                encoding="utf-8"
            ),
            (
                root
                / "migrations/witness/20260726000100_restrict_witness_runtime_privileges.up.sql"
            ).read_text(encoding="utf-8"),
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
    for witness_role in (
        "trpg_witness_append_login",
        "trpg_witness_read_login",
        "trpg_witness_owner",
    ):
        if not re.search(
            rf"(?m)^hostssl\s+coc_ai_trpg_witness\s+{witness_role}"
            r"\s+samenet\s+scram-sha-256\s*$",
            witness_hba,
        ):
            found.append(
                "PostgreSQL witness role is not limited to its database and "
                f"backend network: {witness_role}"
            )
    if re.search(
        r"(?m)^(?:host|hostssl)\s+all\s+"
        r"(?:trpg_database_owner|trpg_witness_owner|"
        r"trpg_witness_append_login|trpg_witness_read_login)\s+"
        r"(?:0\.0\.0\.0/0|::/0)",
        primary_hba + "\n" + witness_hba,
    ):
        found.append(
            "PostgreSQL privileged/runtime access still accepts unrestricted "
            "routed addresses"
        )

    role_bootstrap = (root / "config/postgres/witness-runtime-roles.sh").read_text(
        encoding="utf-8"
    )
    witness_privilege_migration = (
        root
        / "migrations/witness/20260726000100_restrict_witness_runtime_privileges.up.sql"
    ).read_text(encoding="utf-8")
    required_witness_role_fragments = (
        "pg_read_file('/run/secrets/postgres_witness_append_password')",
        "pg_read_file('/run/secrets/postgres_witness_read_password')",
        "FROM pg_auth_members membership",
        "Re-apply the privilege boundary on every startup",
        "REVOKE ALL PRIVILEGES ON ALL TABLES IN SCHEMA public FROM PUBLIC",
        "GRANT SELECT, INSERT ON TABLE external_audit_witness",
        "dependency.deptype = 'o'",
        "GRANT trpg_witness_append_service TO trpg_witness_append_login",
        "GRANT trpg_witness_read_service TO trpg_witness_read_login",
    )
    for fragment in required_witness_role_fragments:
        if fragment not in role_bootstrap:
            found.append(f"witness role bootstrap is not fail-closed: {fragment}")
    required_witness_migration_fragments = (
        "REVOKE ALL PRIVILEGES ON ALL TABLES IN SCHEMA public FROM PUBLIC;",
        "REVOKE ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public FROM PUBLIC;",
        "GRANT SELECT, INSERT ON TABLE external_audit_witness",
        "TO trpg_witness_append_service;",
        "GRANT SELECT ON TABLE external_audit_witness",
        "TO trpg_witness_read_service;",
    )
    for fragment in required_witness_migration_fragments:
        if fragment not in witness_privilege_migration:
            found.append(f"witness privilege migration is incomplete: {fragment}")
    if re.search(
        r"(?is)GRANT\s+(?:ALL|.*\b(?:UPDATE|DELETE|TRUNCATE|REFERENCES|TRIGGER)\b)"
        r".*?\bTO\s+trpg_witness_(?:append|read)_(?:service|login)",
        witness_privilege_migration,
    ):
        found.append("witness runtime migration grants mutable/owner privileges")

    if "pull_request:" not in workflow:
        found.append("production security workflow does not run on pull requests")
    if "production-security-smoke.sh" not in workflow:
        found.append("production security workflow does not execute the runtime smoke")
    if not smoke_phase_paths:
        found.append("production runtime smoke has no responsibility-focused phases")
    for phase_path in smoke_phase_paths:
        phase_reference = f"production-security-smoke/{phase_path.name}"
        if phase_reference not in smoke_entrypoint:
            found.append(f"production runtime smoke does not source phase: {phase_reference}")
    for fragment in REQUIRED_RUNTIME_SMOKE_FRAGMENTS:
        if fragment not in smoke:
            found.append(f"production runtime smoke is incomplete: {fragment}")
    if smoke.count('"${compose_command[@]}" run --rm witness-role-bootstrap') < 2:
        found.append(
            "production runtime smoke does not prove existing-volume witness "
            "privilege repair"
        )
    if "runtime-smoke-infrastructure-only" in smoke:
        found.append("production runtime smoke still uses a placeholder NATS credential")
    found.extend(runtime_secret_staging_errors(root))
    return found
