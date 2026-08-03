# Linux local-provider network topology

Production Ollama and llama.cpp use the same provider boundary:

```text
Agent Gateway -> Agent Runtime -> authenticated HTTPS provider adapter
                                      |
                                      v
                         explicitly allowlisted proxy
                                      |
                                      v
                       loopback-only Ollama or llama.cpp
```

The raw Ollama or llama.cpp listener is not a production endpoint. Keep it on
host loopback and place an authenticated TLS reverse proxy in front of it. The
proxy must validate the mounted bearer credential, present a certificate whose
SAN matches the configured provider hostname, and forward only the required
model API paths. It must not contain a cloud fallback upstream.

The primary supported Linux topology pins the internal Compose `backend`
subnet in an operator `--extra-compose-file`, for example `172.30.0.0/24` with
gateway `172.30.0.1`. Run the TLS/auth proxy on the host, bind it only to that
bridge gateway, and forward to a raw provider bound to host loopback. Do not
use Docker's default `host-gateway`: containers on an `internal: true` network
cannot route to the default bridge gateway.

Issue the proxy certificate with `IP:172.30.0.1` in its SAN and bootstrap with:

```text
--provider-type ollama
--provider-url https://172.30.0.1:9443
--local-provider-allowlist cidr:172.30.0.0/24
--provider-ca-file /absolute/private/provider-ca.pem
--provider-credential-file /absolute/private/provider-token
```

A Compose-managed provider/proxy is the supported alternative: give it a
single-label backend alias such as `ollama-proxy`, keep it off the edge network,
and use `https://ollama-proxy:9443` with `dns:ollama-proxy`. Its only upstream
must be the configured raw Ollama or llama.cpp listener. RFC1918 or unique-local
IP endpoints require an exact canonical `cidr:` entry. Public DNS, multi-label
hostnames, public CIDRs, wildcards, plaintext HTTP, URL credentials, and an
adjacent unlisted service are rejected.

The Admin control plane and agent worker parse the same immutable allowlist.
The policy is included in the provider security attestation and revalidated
immediately before a probe or HTTP-client construction. The backend Compose
network is internal, the HTTP client disables ambient proxies for local
providers, and a local failure cannot select a cloud route without a persisted,
single-use cloud-egress authorization event.

For the local Ollama installation used by this repository, verify chat,
structured output, and tool requests with `qwen3.6:35b`, and verify embeddings
with `qwen3-embedding:8b`. These are separate model identities: an embedding
probe must name the embedding model explicitly and must not silently substitute
the Keeper model. The same HTTPS, CA, credential, hostname, allowlist, and
no-fallback requirements apply to both. llama.cpp must pass the equivalent
OpenAI-compatible contract through the same boundary.
