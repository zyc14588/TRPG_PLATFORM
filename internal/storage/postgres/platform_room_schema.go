// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package postgres

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

// Bootstrap uses separate relational tables; no existing authentication or
// core DDL changes and no listener or Session is started by this operation.
func (r *PlatformRoomStorage) Bootstrap(ctx context.Context) error {
	if r.state() == nil || ctx == nil || ctx.Err() != nil {
		return auth.ErrUnavailable
	}
	tx, e := r.state().repo.state().core.state().db.BeginTx(ctx, nil)
	if e != nil {
		return auth.SafeError(platformError(e))
	}
	defer tx.Rollback()
	for _, ddl := range []string{
		`SET LOCAL statement_timeout='3s'`, `SET LOCAL lock_timeout='3s'`,
		`CREATE SCHEMA IF NOT EXISTS platform_room`,
		`CREATE TABLE IF NOT EXISTS platform_room.rooms(
		 workspace_id text NOT NULL REFERENCES platform_core.workspaces(id),
		 room_id text NOT NULL CHECK(room_id ~ '^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$'),
		 game_id text NOT NULL CHECK(game_id ~ '^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$'),
		 name text NOT NULL CHECK(length(name) BETWEEN 1 AND 80 AND name ~ '\S' AND name !~ '[\x00-\x1F\x7F]'),
		 owner_account_id text NOT NULL REFERENCES platform_core.accounts(id),
		 state text NOT NULL DEFAULT 'lobby' CHECK(state IN ('lobby','launched','closed')),
		 PRIMARY KEY(workspace_id,room_id), UNIQUE(workspace_id,room_id,game_id))`,
		`CREATE OR REPLACE FUNCTION platform_room.immutable_identity() RETURNS trigger LANGUAGE plpgsql AS $$
		 BEGIN IF (NEW.workspace_id,NEW.room_id,NEW.game_id,NEW.owner_account_id) IS DISTINCT FROM
		 (OLD.workspace_id,OLD.room_id,OLD.game_id,OLD.owner_account_id) THEN
		 RAISE EXCEPTION 'immutable room identity' USING ERRCODE='23514'; END IF; RETURN NEW; END $$`,
		`DO $$ BEGIN IF NOT EXISTS(SELECT 1 FROM pg_trigger WHERE tgrelid='platform_room.rooms'::regclass AND tgname='platform_room_identity') THEN
		 CREATE TRIGGER platform_room_identity BEFORE UPDATE ON platform_room.rooms FOR EACH ROW EXECUTE FUNCTION platform_room.immutable_identity(); END IF; END $$`,
		`CREATE TABLE IF NOT EXISTS platform_room.invitations(
		 workspace_id text NOT NULL, room_id text NOT NULL, game_id text NOT NULL,
		 id text NOT NULL CHECK(id ~ '^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$'),
		 link_hash text NOT NULL UNIQUE CHECK(link_hash ~ '^[a-f0-9]{64}$'),
		 code_hash text NOT NULL UNIQUE CHECK(code_hash ~ '^[a-f0-9]{64}$'),
		 approval_required boolean NOT NULL, expires_at timestamptz NOT NULL CHECK(isfinite(expires_at)),
		 max_uses integer NOT NULL CHECK(max_uses BETWEEN 1 AND 64),
		 uses integer NOT NULL DEFAULT 0 CHECK(uses>=0 AND uses<=max_uses), revoked boolean NOT NULL DEFAULT false,
		 PRIMARY KEY(workspace_id,room_id,game_id,id),
		 FOREIGN KEY(workspace_id,room_id,game_id) REFERENCES platform_room.rooms(workspace_id,room_id,game_id))`,
		`CREATE TABLE IF NOT EXISTS platform_room.participants(
		 workspace_id text NOT NULL, room_id text NOT NULL, game_id text NOT NULL,
		 id text NOT NULL CHECK(id ~ '^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$'),
		 account_id text REFERENCES platform_core.accounts(id),guest_id text,
		 name text NOT NULL CHECK(length(name) BETWEEN 1 AND 128 AND name ~ '\S' AND name !~ '[\x00-\x1F\x7F]'),
		 active boolean NOT NULL DEFAULT true, host boolean NOT NULL DEFAULT false,
		 CHECK(account_id IS NOT NULL OR guest_id IS NOT NULL), CHECK(account_id IS NOT NULL OR length(name)<=80),
		 PRIMARY KEY(workspace_id,room_id,game_id,id),
		 UNIQUE(workspace_id,room_id,game_id,account_id), UNIQUE(workspace_id,room_id,game_id,guest_id),
		 FOREIGN KEY(workspace_id,room_id,game_id) REFERENCES platform_room.rooms(workspace_id,room_id,game_id),
		 FOREIGN KEY(workspace_id,room_id,game_id,guest_id) REFERENCES platform_core.guests(workspace_id,room_id,game_id,id))`,
		`CREATE TABLE IF NOT EXISTS platform_room.managers(
		 workspace_id text NOT NULL, room_id text NOT NULL, game_id text NOT NULL,account_id text NOT NULL,
		 PRIMARY KEY(workspace_id,room_id,game_id,account_id),
		 FOREIGN KEY(workspace_id,room_id,game_id) REFERENCES platform_room.rooms(workspace_id,room_id,game_id),
		 FOREIGN KEY(workspace_id,account_id) REFERENCES platform_core.memberships(workspace_id,account_id) ON DELETE CASCADE)`,
		`CREATE TABLE IF NOT EXISTS platform_room.admissions(
		 id text PRIMARY KEY CHECK(id ~ '^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$'),
		 workspace_id text NOT NULL,room_id text NOT NULL,game_id text NOT NULL,invitation_id text NOT NULL,
		 mode text NOT NULL CHECK(mode IN ('account','guest')),status text NOT NULL CHECK(status IN ('pending','approved','rejected','expired')),
		 name text NOT NULL CHECK(length(name) BETWEEN 1 AND 128 AND name ~ '\S' AND name !~ '[\x00-\x1F\x7F]'),
		 owner_account_id text REFERENCES platform_core.accounts(id),owner_session_hash text REFERENCES platform_auth.sessions(token_hash),
		 participant_id text,expires_at timestamptz NOT NULL CHECK(isfinite(expires_at)),
		 token_hash text UNIQUE CHECK(token_hash ~ '^[a-f0-9]{64}$'),token_expires_at timestamptz CHECK(isfinite(token_expires_at)),token_used boolean NOT NULL DEFAULT false,
		 CHECK(mode!='guest' OR length(name)<=80),
		 CHECK((mode='account' AND owner_account_id IS NOT NULL AND owner_session_hash IS NULL AND token_hash IS NULL) OR
		       (mode='guest' AND owner_session_hash IS NOT NULL AND owner_account_id IS NULL)),
		 CHECK((token_hash IS NULL)=(token_expires_at IS NULL)),CHECK(NOT token_used OR (token_hash IS NOT NULL AND participant_id IS NOT NULL)),
		 CHECK(mode!='account' OR status!='approved' OR participant_id IS NOT NULL),
		 FOREIGN KEY(workspace_id,room_id,game_id,invitation_id) REFERENCES platform_room.invitations(workspace_id,room_id,game_id,id),
		 FOREIGN KEY(workspace_id,room_id,game_id,participant_id) REFERENCES platform_room.participants(workspace_id,room_id,game_id,id))`,
		`CREATE UNIQUE INDEX IF NOT EXISTS platform_room_account_admission ON platform_room.admissions(workspace_id,room_id,game_id,invitation_id,owner_account_id) WHERE mode='account'`,
		`CREATE UNIQUE INDEX IF NOT EXISTS platform_room_guest_admission ON platform_room.admissions(workspace_id,room_id,game_id,invitation_id,owner_session_hash) WHERE mode='guest'`,
		`CREATE INDEX IF NOT EXISTS platform_room_pending ON platform_room.admissions(workspace_id,room_id,game_id,status,expires_at)`,
	} {
		if _, e = tx.ExecContext(ctx, ddl); e != nil {
			return auth.SafeError(platformError(e))
		}
	}
	if e = tx.Commit(); e != nil {
		if platformError(e) == core.ErrConflict {
			return auth.ErrConflict
		}
		return auth.ErrOutcomeUnknown
	}
	return nil
}
