# Security, Legal, and Provider Policy

## License gate

Default policy:

- Official SRD/open license/explicit user rights -> `Allowed`.
- Missing or ambiguous license -> `PendingReview`.
- Known incompatible/no-redistribution/commercial rule prose -> `Denied`.
- Commercial adapter schemas/mechanics code may be allowed only when they contain no protected prose.

Enforcement points:

1. Source registration.
2. Before chunking.
3. Before embedding.
4. Before index upsert.
5. Before retrieval query.
6. In PostgreSQL RLS/policies.
7. In API serialization.

## Visibility gate

Filter before scoring:

- `PublicRule`: any authenticated allowed user, license allowed.
- `RoomRule` / `PlVisibleClue`: room members.
- `CharacterPrivate`: character owner, Owner, KP, AssistantKp.
- `KpOnlyModule` / `KpSecret` / `MemoryPrivate`: Owner, KP, AssistantKp.
- `SystemInternal`: never returned by ordinary retrieval.

## Provider boundary

Provider traits must expose metadata:

- provider kind
- provider version/model
- local/cloud classification
- input hash or request id where safe

LocalOnly mode:

- Reject cloud LLM.
- Reject cloud embedder.
- Reject cloud reranker.
- Reject cloud OCR/image provider.
- Use deterministic local providers in tests.

## Secrets

- No provider API key in frontend code.
- No secrets in OpenAPI examples.
- No secrets in logs, audit rows, or error messages.

## Audit

Record audit events for:

- source submitted
- source pending review
- source approved/denied
- document indexed
- retrieval query denied by policy, using privacy-preserving metadata only

Do not log raw query text if it may contain private player content unless the project explicitly allows it and redaction is implemented.

## Error handling

- Hidden or denied resources should not reveal existence.
- Prefer generic 403/404 for authorization failures.
- License-denied ingest can return an explicit license status because the actor submitted the source.
- Retrieval denial must not reveal KP-only content names.
