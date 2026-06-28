# P2 Security, Legal, Privacy and Provider Policy

## License policy

P2 must not import or index unauthorized commercial rules prose.

Default mapping:

| Input | Decision |
|---|---|
| open SRD / explicitly licensed text | Allowed |
| user-owned campaign notes | Allowed, subject to room visibility |
| unclear license | PendingReview |
| commercial rules prose without permission | Denied |
| mechanics-only schema/metadata adapter | Allowed if no copyrighted prose |
| KP private module notes | Allowed only under KP visibility policy |

License status affects every stage: chunking, embedding, indexing, retrieval, prompt construction, frontend display.

## Visibility policy

Visibility is a data access rule, not UI decoration.

| Visibility | Ordinary PL retrieval | KP retrieval | Notes |
|---|---:|---:|---|
| PublicRule | yes if license allowed | yes | still requires allowed license |
| RoomRule | yes for members | yes | room-bound |
| PlayerVisibleClue | yes if player can see | yes | scenario-dependent |
| CharacterPrivate | owner/allowed actor only | maybe | observer cannot retrieve |
| KpOnlyModule | no | yes | never in PL candidate set |
| KpSecret | no | yes | use careful previews |
| MemoryPrivate | owner/allowed actor only | maybe | strict DTO stripping |
| SystemInternal | no | no ordinary path | internal only |

## RLS and room boundary

- P2 tables must use RLS and repository predicates.
- RLS tests must not use `postgres` superuser as proof.
- Current actor/room/role context must be set by trusted server code.
- Client-supplied role is never trusted.

## LocalOnly provider policy

When privacy mode is LocalOnly:

- no cloud LLM;
- no cloud embedding;
- no cloud reranking;
- no cloud OCR/image/transcription;
- no provider telemetry containing raw hidden content;
- deterministic local/fake providers are allowed for tests.

## AllowConfiguredCloud policy

Cloud provider calls require all of:

1. server-side configured provider;
2. room/privacy policy permits it;
3. source license permits provider processing;
4. visibility permits content to be sent to that provider;
5. no client-supplied secrets;
6. logging/metrics redaction enabled.

## Secret handling

Never expose:

- provider API keys;
- `DATABASE_URL`;
- JWT/Auth secret;
- bearer/cookie/CSRF tokens;
- raw hidden content in logs;
- provider raw request/response containing hidden content.

Check these areas:

- OpenAPI examples
- test snapshots
- frontend bundle/env
- docs examples
- metrics labels
- error messages

## Error disclosure

For hidden/forbidden resources, choose project policy:

- return `403` with generic message, or
- return `404` to avoid existence disclosure.

Do not include title/source name/visibility/license reason for hidden content in ordinary errors.

## Rig-specific policy

Rig agents and tools are untrusted with respect to authorization. Tool-call closures must receive already-authorized context and call project services that enforce repository/RLS policy. Rig prompt construction must receive only authorized evidence.

## Required cross-cutting tests

- pending/denied never in ordinary retrieval
- hidden content absent from prompt context
- LocalOnly rejects cloud provider
- provider metadata has no secret
- error messages do not leak hidden titles
- OpenAPI examples contain no secret
- frontend bundle contains no provider key
- RLS blocks cross-room read using ordinary role
