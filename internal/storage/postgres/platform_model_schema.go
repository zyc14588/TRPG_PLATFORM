// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package postgres

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

// Operational model records do not modify historical events, game state,
// existing schema versions or authentication grants. Provisioning is explicit.
func (s *PlatformModelStorage) Bootstrap(ctx context.Context) error {
	if s.state() == nil || ctx == nil || ctx.Err() != nil {
		return auth.ErrUnavailable
	}
	tx, e := s.state().repo.state().core.state().db.BeginTx(ctx, nil)
	if e != nil {
		return authStorageError(e)
	}
	defer tx.Rollback()
	for _, q := range []string{
		`SET LOCAL statement_timeout='3s'`, `SET LOCAL lock_timeout='3s'`,
		`CREATE SCHEMA IF NOT EXISTS platform_model`,
		`CREATE TABLE IF NOT EXISTS platform_model.credentials(
 workspace_id text NOT NULL,room_id text NOT NULL,game_id text NOT NULL,
 seat_id text NOT NULL CHECK(seat_id ~ '^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$'),id text NOT NULL CHECK(id ~ '^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$'),
 owner_kind text NOT NULL CHECK(owner_kind IN('account','guest')),owner_id text NOT NULL,
 owner_account_id text REFERENCES platform_core.accounts(id),owner_guest_id text,
 lifetime text NOT NULL CHECK(lifetime IN('session','retained')),expires_at timestamptz CHECK(isfinite(expires_at)),
 version bigint NOT NULL CHECK(version=1),ciphertext bytea NOT NULL CHECK(octet_length(ciphertext) BETWEEN 29 AND 4124),revoked boolean NOT NULL DEFAULT false,
 CHECK((owner_kind='account' AND owner_account_id IS NOT NULL AND owner_account_id=owner_id AND owner_guest_id IS NULL) OR (owner_kind='guest' AND owner_guest_id IS NOT NULL AND owner_guest_id=owner_id AND owner_account_id IS NULL)),
 CHECK((lifetime='session' AND expires_at IS NOT NULL) OR(lifetime='retained' AND expires_at IS NULL AND owner_kind='account')),
 PRIMARY KEY(workspace_id,room_id,game_id,seat_id,id),UNIQUE(workspace_id,room_id,game_id,seat_id,id,version),
 FOREIGN KEY(workspace_id,room_id,game_id) REFERENCES platform_room.rooms(workspace_id,room_id,game_id),
 FOREIGN KEY(workspace_id,room_id,game_id,owner_guest_id) REFERENCES platform_core.guests(workspace_id,room_id,game_id,id))`,
		`CREATE OR REPLACE FUNCTION platform_model.immutable_credential() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
 IF (NEW.workspace_id,NEW.room_id,NEW.game_id,NEW.seat_id,NEW.id,NEW.owner_kind,NEW.owner_id,NEW.owner_account_id,NEW.owner_guest_id,NEW.lifetime,NEW.expires_at,NEW.version,NEW.ciphertext) IS DISTINCT FROM (OLD.workspace_id,OLD.room_id,OLD.game_id,OLD.seat_id,OLD.id,OLD.owner_kind,OLD.owner_id,OLD.owner_account_id,OLD.owner_guest_id,OLD.lifetime,OLD.expires_at,OLD.version,OLD.ciphertext) OR (OLD.revoked AND NOT NEW.revoked) THEN RAISE EXCEPTION 'immutable credential binding' USING ERRCODE='23514';END IF;RETURN NEW;END $$`,
		`DO $$ BEGIN IF NOT EXISTS(SELECT 1 FROM pg_trigger WHERE tgrelid='platform_model.credentials'::regclass AND tgname='platform_model_credential_identity') THEN CREATE TRIGGER platform_model_credential_identity BEFORE UPDATE ON platform_model.credentials FOR EACH ROW EXECUTE FUNCTION platform_model.immutable_credential();END IF;END $$`,
		`CREATE TABLE IF NOT EXISTS platform_model.configurations(
 workspace_id text NOT NULL,room_id text NOT NULL,game_id text NOT NULL,seat_id text NOT NULL,selection text NOT NULL CHECK(selection ~ '^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$'),
 version bigint NOT NULL CHECK(version BETWEEN 1 AND 9007199254740992),credential_id text NOT NULL,credential_version bigint NOT NULL CHECK(credential_version=1),
 primary_id text NOT NULL,primary_hash text NOT NULL CHECK(primary_hash ~ '^[a-f0-9]{64}$'),tuple_hash text NOT NULL CHECK(tuple_hash ~ '^[a-f0-9]{64}$'),
 configuration_id text NOT NULL,configuration_hash text NOT NULL CHECK(configuration_hash ~ '^sha256:[a-f0-9]{64}$'),graph_hash text NOT NULL CHECK(graph_hash ~ '^sha256:[a-f0-9]{64}$'),preparation_revision bigint NOT NULL CHECK(preparation_revision>0),
 owner_kind text NOT NULL CHECK(owner_kind IN('account','guest')),owner_id text NOT NULL,body bytea NOT NULL CHECK(octet_length(body) BETWEEN 1 AND 16384),revoked boolean NOT NULL DEFAULT false,
 PRIMARY KEY(workspace_id,room_id,game_id,seat_id,selection),
 FOREIGN KEY(workspace_id,room_id,game_id,seat_id,credential_id,credential_version) REFERENCES platform_model.credentials(workspace_id,room_id,game_id,seat_id,id,version))`,
	} {
		if _, e = tx.ExecContext(ctx, q); e != nil {
			return authStorageError(e)
		}
	}
	if e = tx.Commit(); e != nil {
		return auth.ErrOutcomeUnknown
	}
	return nil
}
