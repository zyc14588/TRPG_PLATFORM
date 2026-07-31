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

"""Compose parsing primitives and security-policy constants."""
