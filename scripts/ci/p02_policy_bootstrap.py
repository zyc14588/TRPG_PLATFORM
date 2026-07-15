#!/usr/bin/env python3
"""Bootstrap the real P02 OpenFGA/OPA/PostgreSQL integration environment."""

from __future__ import annotations

import argparse
import json
import os
import time
import urllib.error
import urllib.request
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def request_json(method: str, url: str, body: object | None = None) -> dict[str, object]:
    encoded = None if body is None else json.dumps(body).encode("utf-8")
    request = urllib.request.Request(
        url,
        data=encoded,
        method=method,
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=5) as response:
        payload = response.read()
    if not payload:
        return {}
    value = json.loads(payload)
    if not isinstance(value, dict):
        raise RuntimeError(f"unexpected JSON response from {url}")
    return value


def wait_until_ready(url: str, timeout_seconds: float = 30.0) -> None:
    deadline = time.monotonic() + timeout_seconds
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        try:
            request_json("GET", url)
            return
        except (OSError, RuntimeError, urllib.error.HTTPError) as error:
            last_error = error
            time.sleep(0.25)
    raise RuntimeError(f"service did not become ready at {url}: {last_error}")


def require_string(response: dict[str, object], key: str) -> str:
    value = response.get(key)
    if not isinstance(value, str) or not value.strip():
        raise RuntimeError(f"bootstrap response omitted {key}")
    return value


def bootstrap(openfga_address: str, opa_address: str, model_path: Path) -> dict[str, str]:
    openfga_url = f"http://{openfga_address}"
    opa_url = f"http://{opa_address}"
    wait_until_ready(f"{openfga_url}/healthz")
    wait_until_ready(f"{opa_url}/health")

    store = request_json(
        "POST",
        f"{openfga_url}/stores",
        {"name": f"p02-ci-{os.environ.get('GITHUB_RUN_ID', os.getpid())}"},
    )
    store_id = require_string(store, "id")
    model = json.loads(model_path.read_text(encoding="utf-8"))
    model_response = request_json(
        "POST",
        f"{openfga_url}/stores/{store_id}/authorization-models",
        model,
    )
    model_id = require_string(model_response, "authorization_model_id")
    request_json(
        "POST",
        f"{openfga_url}/stores/{store_id}/write",
        {
            "authorization_model_id": model_id,
            "writes": {
                "tuple_keys": [
                    {
                        "user": "principal:workflow_001",
                        "relation": "workflow",
                        "object": "campaign:camp_human_archive",
                    },
                    {
                        "user": "principal:workflow_001",
                        "relation": "workflow",
                        "object": "campaign:camp_ai_harbor",
                    },
                    {
                        "user": "principal:owner_a",
                        "relation": "server_owner",
                        "object": "campaign:campaign_a",
                    },
                ]
            },
        },
    )
    return {
        "P02_DATABASE_URL": "postgresql://postgres@127.0.0.1:15432/p02_identity",
        "P02_OPENFGA_ADDRESS": openfga_address,
        "P02_OPENFGA_STORE_ID": store_id,
        "P02_OPENFGA_MODEL_ID": model_id,
        "P02_OPA_ADDRESS": opa_address,
        "P02_OPA_REVISION": "opa-security-governance-v2",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--openfga-address", default="127.0.0.1:18080")
    parser.add_argument("--opa-address", default="127.0.0.1:18082")
    parser.add_argument(
        "--model",
        type=Path,
        default=ROOT / "policy/openfga/security_governance.json",
    )
    parser.add_argument("--github-env", type=Path)
    args = parser.parse_args()

    environment = bootstrap(args.openfga_address, args.opa_address, args.model.resolve())
    if args.github_env is not None:
        with args.github_env.open("a", encoding="utf-8") as output:
            for key, value in environment.items():
                output.write(f"{key}={value}\n")
    print(json.dumps(environment, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
