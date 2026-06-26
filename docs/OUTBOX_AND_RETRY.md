# Transactional Outbox and Retry Policy

## State Writes

All authoritative writes for combat, turns, character sheets, and other versioned aggregates must include:

- `expected_version` for optimistic compare-and-swap;
- `idempotency_key` when a command may be retried.

Version conflicts return a conflict response with the current version or latest safe snapshot. Client drafts must not be silently overwritten.

## Retryable SQLSTATEs

The application classifies these PostgreSQL SQLSTATEs as retryable only for idempotent commands:

- `40P01`: deadlock detected;
- `40001`: serialization failure;
- `55P03`: lock not available.

Retries are bounded to at most three attempts with backoff. Non-idempotent commands are not retried automatically.

## External Side Effects

External model calls, embeddings, image generation, media jobs, and notifications must not be executed inside the same transaction that mutates authoritative game state. Write an outbox record in the transaction, commit, then process the side effect from the worker.

## Worker Semantics

Outbox processing must be idempotent. A worker may mark jobs as pending, processing, succeeded, or failed. Failed jobs keep enough metadata for audit and retry decisions without leaking API keys or protected prompts.

## Tests

Required tests include retry classification, retry exhaustion, duplicate idempotency replay, and proof that external side effects are represented as outbox records before execution.
