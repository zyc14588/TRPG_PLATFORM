// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package postgres

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

// These private operational records do not alter event history, game schemas,
// cookie identities, public interfaces or the version-one Host contract.
func (s *PlatformBudgetStorage) Bootstrap(ctx context.Context) error {
	if s == nil || s.data == nil || *s.data == nil || ctx == nil || ctx.Err() != nil {
		return auth.ErrUnavailable
	}
	tx, e := (*s.data).repo.state().core.state().db.BeginTx(ctx, nil)
	if e != nil {
		return authStorageError(e)
	}
	defer tx.Rollback()
	for _, q := range []string{
		`SET LOCAL statement_timeout='3s'`, `SET LOCAL lock_timeout='3s'`, `CREATE SCHEMA IF NOT EXISTS platform_budget`,
		`CREATE TABLE IF NOT EXISTS platform_budget.tasks(workspace_id text NOT NULL,room_id text NOT NULL,game_id text NOT NULL,session_id text NOT NULL,seat_id text NOT NULL,id text NOT NULL CHECK(id ~ '^[a-f0-9]{32}$'),controller text NOT NULL REFERENCES platform_core.accounts(id),graph_hash text NOT NULL,body bytea NOT NULL CHECK(octet_length(body) BETWEEN 1 AND 8192),status text NOT NULL CHECK(status IN('open','reserved','dispatched','settled','uncertain','paused')),PRIMARY KEY(workspace_id,room_id,game_id,session_id,seat_id,id),FOREIGN KEY(workspace_id,session_id) REFERENCES platform_launch.sessions(workspace_id,session_id))`,
		`CREATE TABLE IF NOT EXISTS platform_budget.reservations(workspace_id text NOT NULL,room_id text NOT NULL,game_id text NOT NULL,session_id text NOT NULL,seat_id text NOT NULL,task_id text NOT NULL,body bytea NOT NULL CHECK(octet_length(body) BETWEEN 1 AND 8192),spent bytea NOT NULL CHECK(octet_length(spent) BETWEEN 1 AND 512),status text NOT NULL CHECK(status IN('reserved','dispatched','settled','uncertain','paused')),PRIMARY KEY(workspace_id,room_id,game_id,session_id,seat_id,task_id),FOREIGN KEY(workspace_id,room_id,game_id,session_id,seat_id,task_id) REFERENCES platform_budget.tasks(workspace_id,room_id,game_id,session_id,seat_id,id))`,
		`CREATE TABLE IF NOT EXISTS platform_budget.counters(workspace_id text NOT NULL REFERENCES platform_core.workspaces(id),level text NOT NULL CHECK(level IN('workspace','room','session','seat','task')),node_id text NOT NULL CHECK(length(node_id) BETWEEN 1 AND 512),cap bytea NOT NULL CHECK(octet_length(cap) BETWEEN 1 AND 512),used bytea NOT NULL CHECK(octet_length(used) BETWEEN 1 AND 512),held bytea NOT NULL CHECK(octet_length(held) BETWEEN 1 AND 512),PRIMARY KEY(workspace_id,level,node_id))`,
		`CREATE TABLE IF NOT EXISTS platform_budget.pauses(workspace_id text NOT NULL,session_id text NOT NULL,seat_id text NOT NULL,reason text NOT NULL CHECK(reason IN('exhausted','uncertain')),PRIMARY KEY(workspace_id,session_id,seat_id),FOREIGN KEY(workspace_id,session_id) REFERENCES platform_launch.sessions(workspace_id,session_id))`,
		`CREATE TABLE IF NOT EXISTS platform_budget.memories(workspace_id text NOT NULL,room_id text NOT NULL,game_id text NOT NULL,session_id text NOT NULL,seat_id text NOT NULL,graph_hash text NOT NULL,id text NOT NULL CHECK(id ~ '^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$'),generator_version text NOT NULL,fact_level text NOT NULL CHECK(fact_level IN('summary','hypothesis')),body bytea NOT NULL CHECK(octet_length(body) BETWEEN 1 AND 8192),PRIMARY KEY(workspace_id,room_id,game_id,session_id,seat_id,id),FOREIGN KEY(workspace_id,session_id) REFERENCES platform_launch.sessions(workspace_id,session_id))`,
		`CREATE OR REPLACE FUNCTION platform_budget.immutable_record() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF TG_OP='DELETE' THEN RAISE EXCEPTION 'immutable budget identity' USING ERRCODE='23514';END IF;IF (NEW.workspace_id,NEW.room_id,NEW.game_id,NEW.session_id,NEW.seat_id,NEW.body) IS DISTINCT FROM (OLD.workspace_id,OLD.room_id,OLD.game_id,OLD.session_id,OLD.seat_id,OLD.body) THEN RAISE EXCEPTION 'immutable budget binding' USING ERRCODE='23514';END IF;RETURN NEW;END $$`,
		`DO $$ BEGIN IF NOT EXISTS(SELECT 1 FROM pg_trigger WHERE tgrelid='platform_budget.tasks'::regclass AND tgname='platform_budget_task_identity') THEN CREATE TRIGGER platform_budget_task_identity BEFORE UPDATE OR DELETE ON platform_budget.tasks FOR EACH ROW EXECUTE FUNCTION platform_budget.immutable_record();END IF;IF NOT EXISTS(SELECT 1 FROM pg_trigger WHERE tgrelid='platform_budget.reservations'::regclass AND tgname='platform_budget_reservation_identity') THEN CREATE TRIGGER platform_budget_reservation_identity BEFORE UPDATE OR DELETE ON platform_budget.reservations FOR EACH ROW EXECUTE FUNCTION platform_budget.immutable_record();END IF;END $$`,
		`CREATE OR REPLACE FUNCTION platform_budget.immutable_memory() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'immutable private memory' USING ERRCODE='23514';END $$`,
		`DO $$ BEGIN IF NOT EXISTS(SELECT 1 FROM pg_trigger WHERE tgrelid='platform_budget.memories'::regclass AND tgname='platform_budget_memory_identity') THEN CREATE TRIGGER platform_budget_memory_identity BEFORE UPDATE OR DELETE ON platform_budget.memories FOR EACH ROW EXECUTE FUNCTION platform_budget.immutable_memory();END IF;END $$`,
		`CREATE OR REPLACE FUNCTION platform_budget.immutable_counter() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF (NEW.workspace_id,NEW.level,NEW.node_id,NEW.cap) IS DISTINCT FROM (OLD.workspace_id,OLD.level,OLD.node_id,OLD.cap) THEN RAISE EXCEPTION 'immutable budget cap' USING ERRCODE='23514';END IF;RETURN NEW;END $$`,
		`DO $$ BEGIN IF NOT EXISTS(SELECT 1 FROM pg_trigger WHERE tgrelid='platform_budget.counters'::regclass AND tgname='platform_budget_counter_identity') THEN CREATE TRIGGER platform_budget_counter_identity BEFORE UPDATE ON platform_budget.counters FOR EACH ROW EXECUTE FUNCTION platform_budget.immutable_counter();END IF;END $$`,
	} {
		if _, e = tx.ExecContext(ctx, q); e != nil {
			return authStorageError(e)
		}
	}
	return authStorageError(tx.Commit())
}
