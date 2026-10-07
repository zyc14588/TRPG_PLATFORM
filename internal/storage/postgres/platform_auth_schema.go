// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package postgres

import (
	"context"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

// Bootstrap is an explicit trusted operator action. No implicit schema writes
// run in request handlers, and accepted core DDL remains unchanged.
func (r *PlatformAuthRepository) Bootstrap(ctx context.Context) error {
	d := r.state()
	if d == nil || ctx == nil || ctx.Err() != nil {
		return auth.ErrUnavailable
	}
	if e := d.core.Bootstrap(ctx); e != nil {
		return auth.SafeError(e)
	}
	tx, e := d.core.state().db.BeginTx(ctx, nil)
	if e != nil {
		return authStorageError(e)
	}
	defer tx.Rollback()
	for _, statement := range []string{
		`CREATE SCHEMA IF NOT EXISTS platform_auth`,
		`CREATE TABLE IF NOT EXISTS platform_auth.credentials(
			login_name text PRIMARY KEY CHECK(login_name ~ '^[a-z][a-z0-9_]{2,31}$'),
			account_id text NOT NULL UNIQUE REFERENCES platform_core.accounts(id),
			password_digest bytea NOT NULL CHECK(octet_length(password_digest)=48),
			password_version integer NOT NULL DEFAULT 1 CHECK(password_version=1))`,
		`CREATE TABLE IF NOT EXISTS platform_auth.registration_grants(
			token_hash text PRIMARY KEY CHECK(token_hash ~ '^[A-Za-z0-9_-]{43}$'),
			expires_at timestamptz NOT NULL CHECK(isfinite(expires_at)),consumed boolean NOT NULL DEFAULT false)`,
		`CREATE TABLE IF NOT EXISTS platform_auth.sessions(
			token_hash text PRIMARY KEY CHECK(token_hash ~ '^[A-Za-z0-9_-]{43}$'),
			kind text NOT NULL CHECK(kind IN ('preauth','account','guest')),
			account_id text REFERENCES platform_core.accounts(id),
			guest_id text,workspace_id text,room_id text,game_id text,
			expires_at timestamptz NOT NULL CHECK(isfinite(expires_at)),
			last_seen timestamptz NOT NULL CHECK(isfinite(last_seen)),
			retired boolean NOT NULL DEFAULT false,revoked boolean NOT NULL DEFAULT false,
			successor_hash text REFERENCES platform_auth.sessions(token_hash),
			CHECK((retired AND successor_hash IS NOT NULL) OR (NOT retired AND successor_hash IS NULL)),
			CHECK((kind='preauth' AND account_id IS NULL AND guest_id IS NULL AND workspace_id IS NULL AND room_id IS NULL AND game_id IS NULL) OR
			(kind='account' AND account_id IS NOT NULL AND guest_id IS NULL AND workspace_id IS NULL AND room_id IS NULL AND game_id IS NULL) OR
			(kind='guest' AND account_id IS NULL AND guest_id IS NOT NULL AND workspace_id IS NOT NULL AND room_id IS NOT NULL AND game_id IS NOT NULL)),
			FOREIGN KEY(workspace_id,room_id,game_id,guest_id) REFERENCES platform_core.guests(workspace_id,room_id,game_id,id))`,
		`CREATE TABLE IF NOT EXISTS platform_auth.receipts(
			owner_hash text NOT NULL REFERENCES platform_auth.sessions(token_hash),
			endpoint text NOT NULL CHECK(length(endpoint) BETWEEN 1 AND 300),
			idempotency_key text NOT NULL CHECK(idempotency_key ~ '^[A-Za-z0-9][A-Za-z0-9_.-]{15,127}$'),
			request_digest text NOT NULL CHECK(request_digest ~ '^[A-Za-z0-9_-]{43}$'),
			expires_at timestamptz NOT NULL CHECK(isfinite(expires_at)),
			ciphertext bytea NOT NULL CHECK(octet_length(ciphertext) BETWEEN 28 AND 65536),
			PRIMARY KEY(owner_hash,endpoint,idempotency_key))`,
	} {
		if _, e := tx.ExecContext(ctx, statement); e != nil {
			return authStorageError(e)
		}
	}
	if e := tx.Commit(); e != nil {
		if mapped := authStorageError(e); mapped == auth.ErrConflict {
			return mapped
		}
		return auth.ErrOutcomeUnknown
	}
	return nil
}
