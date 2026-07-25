#!/usr/bin/env python3
"""Bootstrap the real P02 OpenFGA/OPA/PostgreSQL integration environment."""

from __future__ import annotations

import argparse
import json
import os
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path


def repository_root() -> Path:
    configured = os.environ.get("TRPG_REPOSITORY_ROOT")
    if configured:
        return Path(configured).resolve()
    script_path = Path(__file__).resolve()
    for candidate in script_path.parents:
        if (candidate / "Cargo.toml").is_file() and (
            candidate / "policy/openfga/security_governance.json"
        ).is_file():
            return candidate
    # Container deployments pass --model explicitly and mount the script in a
    # shallow /bootstrap directory. Keep argument parsing usable there instead
    # of indexing a parent that does not exist.
    return script_path.parent


ROOT = repository_root()


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


def find_store(openfga_url: str, store_name: str) -> dict[str, object] | None:
    continuation_token = ""
    matches: list[dict[str, object]] = []
    while True:
        query_parameters = {"page_size": "100"}
        if continuation_token:
            query_parameters["continuation_token"] = continuation_token
        query = urllib.parse.urlencode(query_parameters)
        response = request_json("GET", f"{openfga_url}/stores?{query}")
        stores = response.get("stores", [])
        if not isinstance(stores, list):
            raise RuntimeError("OpenFGA list stores response omitted stores")
        matches.extend(
            store
            for store in stores
            if isinstance(store, dict) and store.get("name") == store_name
        )
        token = response.get("continuation_token")
        if not isinstance(token, str) or not token:
            break
        continuation_token = token
    if len(matches) > 1:
        raise RuntimeError(f"multiple OpenFGA stores named {store_name!r}")
    return matches[0] if matches else None


def bootstrap(
    openfga_address: str,
    opa_address: str,
    model_path: Path,
    store_name: str | None = None,
    seed_test_fixtures: bool = True,
) -> dict[str, str]:
    openfga_url = f"http://{openfga_address}"
    opa_url = f"http://{opa_address}"
    wait_until_ready(f"{openfga_url}/healthz")
    wait_until_ready(f"{opa_url}/health")

    resolved_store_name = store_name or f"p02-ci-{os.environ.get('GITHUB_RUN_ID', os.getpid())}"
    store = find_store(openfga_url, resolved_store_name)
    if store is None:
        store = request_json(
            "POST",
            f"{openfga_url}/stores",
            {"name": resolved_store_name},
        )
    store_id = require_string(store, "id")
    model = json.loads(model_path.read_text(encoding="utf-8"))
    model_response = request_json(
        "POST",
        f"{openfga_url}/stores/{store_id}/authorization-models",
        model,
    )
    model_id = require_string(model_response, "authorization_model_id")
    if seed_test_fixtures:
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
        "P02_OPENFGA_ADDRESS": openfga_address,
        "P02_OPENFGA_STORE_ID": store_id,
        "P02_OPENFGA_MODEL_ID": model_id,
        "P02_OPA_ADDRESS": opa_address,
        "P02_OPA_REVISION": "opa-security-governance-v3",
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
    parser.add_argument("--store-name")
    parser.add_argument("--no-test-fixtures", action="store_true")
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--github-env", type=Path)
    args = parser.parse_args()

    environment = bootstrap(
        args.openfga_address,
        args.opa_address,
        args.model.resolve(),
        store_name=args.store_name,
        seed_test_fixtures=not args.no_test_fixtures,
    )
    if args.github_env is not None:
        with args.github_env.open("a", encoding="utf-8") as output:
            for key, value in environment.items():
                output.write(f"{key}={value}\n")
    if args.output_dir is not None:
        output_dir = args.output_dir.resolve()
        output_dir.mkdir(parents=True, exist_ok=True, mode=0o755)
        output_dir.chmod(0o755)
        for filename, key in (
            ("openfga_store_id", "P02_OPENFGA_STORE_ID"),
            ("openfga_model_id", "P02_OPENFGA_MODEL_ID"),
        ):
            temporary = output_dir / f".{filename}.tmp"
            destination = output_dir / filename
            temporary.write_text(f"{environment[key]}\n", encoding="utf-8")
            temporary.chmod(0o444)
            temporary.replace(destination)
    print(json.dumps(environment, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
