// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package postgres

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

// Schema provisioning remains an explicit operator action. Existing room,
// authentication, installation and Host schemas are never rewritten here.
func (s *PlatformLaunchStorage) Bootstrap(ctx context.Context) error {
	if s.state() == nil || ctx == nil || ctx.Err() != nil {
		return auth.ErrUnavailable
	}
	if e := s.state().host.Bootstrap(ctx); e != nil {
		return auth.ErrUnavailable
	}
	tx, e := s.state().repo.state().core.state().db.BeginTx(ctx, nil)
	if e != nil {
		return auth.ErrUnavailable
	}
	defer tx.Rollback()
	for _, q := range []string{
		`SET LOCAL statement_timeout='3s'`, `SET LOCAL lock_timeout='3s'`,
		`CREATE SCHEMA IF NOT EXISTS platform_launch`,
		`CREATE TABLE IF NOT EXISTS platform_launch.preparation(
		 workspace_id text NOT NULL,room_id text NOT NULL,game_id text NOT NULL,
		 revision bigint NOT NULL CHECK(revision>0),graph_hash text NOT NULL CHECK(graph_hash ~ '^sha256:[a-f0-9]{64}$'),
		 configuration_id text NOT NULL CHECK(configuration_id ~ '^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$'),
		 configuration_hash text NOT NULL CHECK(configuration_hash ~ '^sha256:[a-f0-9]{64}$'),
		 body bytea NOT NULL CHECK(octet_length(body) BETWEEN 1 AND 16384),
		 PRIMARY KEY(workspace_id,room_id,game_id),UNIQUE(workspace_id,room_id,game_id,revision,graph_hash,configuration_hash),
		 FOREIGN KEY(workspace_id,room_id,game_id) REFERENCES platform_room.rooms(workspace_id,room_id,game_id))`,
		`CREATE TABLE IF NOT EXISTS platform_launch.acknowledgments(
		 workspace_id text NOT NULL,room_id text NOT NULL,game_id text NOT NULL,participant_id text NOT NULL,
		 revision bigint NOT NULL,graph_hash text NOT NULL,configuration_hash text NOT NULL,
		 body bytea NOT NULL CHECK(octet_length(body) BETWEEN 1 AND 16384),
		 PRIMARY KEY(workspace_id,room_id,game_id,participant_id),
		 FOREIGN KEY(workspace_id,room_id,game_id,participant_id) REFERENCES platform_room.participants(workspace_id,room_id,game_id,id),
		 FOREIGN KEY(workspace_id,room_id,game_id,revision,graph_hash,configuration_hash) REFERENCES platform_launch.preparation(workspace_id,room_id,game_id,revision,graph_hash,configuration_hash))`,
		`CREATE TABLE IF NOT EXISTS platform_launch.sessions(
		 workspace_id text NOT NULL,room_id text NOT NULL,game_id text NOT NULL,session_id text NOT NULL CHECK(session_id ~ '^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$'),
		 graph_hash text NOT NULL CHECK(graph_hash ~ '^sha256:[a-f0-9]{64}$'),configuration_id text NOT NULL,configuration_hash text NOT NULL,
		 revision bigint NOT NULL CHECK(revision>0),PRIMARY KEY(workspace_id,room_id,game_id),UNIQUE(workspace_id,session_id),
		 FOREIGN KEY(workspace_id,session_id) REFERENCES host_command.sessions(workspace,session),
		 FOREIGN KEY(workspace_id,room_id,game_id,revision,graph_hash,configuration_hash) REFERENCES platform_launch.preparation(workspace_id,room_id,game_id,revision,graph_hash,configuration_hash))`,
		`CREATE OR REPLACE FUNCTION platform_launch.bound_session_identity() RETURNS trigger LANGUAGE plpgsql AS $$
		 BEGIN IF TG_OP='UPDATE' THEN RAISE EXCEPTION 'immutable launch binding' USING ERRCODE='23514'; END IF;
		 IF NOT EXISTS(SELECT 1 FROM host_command.sessions s WHERE s.workspace=NEW.workspace_id AND s.session=NEW.session_id AND s.graph_hash=NEW.graph_hash) THEN RAISE EXCEPTION 'invalid launch graph binding' USING ERRCODE='23514'; END IF; RETURN NEW; END $$`,
		`DO $$ BEGIN IF NOT EXISTS(SELECT 1 FROM pg_trigger WHERE tgrelid='platform_launch.sessions'::regclass AND tgname='platform_launch_session_identity') THEN CREATE TRIGGER platform_launch_session_identity BEFORE INSERT OR UPDATE ON platform_launch.sessions FOR EACH ROW EXECUTE FUNCTION platform_launch.bound_session_identity(); END IF; END $$`,
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
