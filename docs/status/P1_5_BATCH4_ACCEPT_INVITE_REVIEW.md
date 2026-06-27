# P1.5 Batch 4 Accept Invite Idempotency Review

Status: follow-up note

## Confirmed Behavior

- Successful `accept_room_invitation` calls now enter repository-level idempotency handling before checking whether the invite is still pending.
- A retry with the same token and same idempotency key should replay the stored successful response instead of returning `404 invitation not found`.
- The focused HTTP/in-memory regression test is `accept_invite_duplicate_replays_after_invite_is_accepted`.

## Remaining Coverage Concern

The required behavior is implemented in the PostgreSQL repository path through `PostgresRepositories::accept_room_invite_idempotent`, including the transaction, `SELECT ... FOR UPDATE`, invite validation, membership write, audit write, idempotency completion, and replay.

However, the explicit regression tests added in Batch 4 exercise the HTTP route with `InMemoryAuthStore`. The existing PostgreSQL HTTP flow compiles and runs when `DATABASE_URL` is available, but there is no storage-level PostgreSQL test dedicated to this exact replay-after-accepted scenario.

Follow-up test to add when tightening DB coverage:

- `postgres_accept_invite_duplicate_replays_after_invite_is_accepted`

That test should use `PostgresRepositories`, create a room invite, call `accept_room_invite_idempotent` once successfully, then call it again with the same token hash and idempotency key after the invite status is already `accepted`, and assert `IdempotentOutcome::Replayed` rather than `RepositoryError::NotFound`.
