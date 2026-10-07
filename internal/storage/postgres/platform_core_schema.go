// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package postgres

import (
	"context"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

// Bootstrap is a separate trusted operator action, using only static DDL. The
// deferred circular owner relation makes orphan workspaces, owner demotion and
// additional owners impossible at commit, including direct storage misuse.
func (r *PlatformCoreRepository) Bootstrap(ctx context.Context) error {
	d := r.state()
	if d == nil || ctx == nil || ctx.Err() != nil {
		return core.ErrUnavailable
	}
	tx, err := d.db.BeginTx(ctx, nil)
	if err != nil {
		return platformError(err)
	}
	defer tx.Rollback()
	for _, statement := range []string{
		`CREATE SCHEMA IF NOT EXISTS platform_core`,
		`CREATE TABLE IF NOT EXISTS platform_core.accounts(
			id text PRIMARY KEY CHECK(id ~ '^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$'),
			display_name text NOT NULL CHECK(length(btrim(display_name)) BETWEEN 1 AND 128 AND display_name !~ '[[:cntrl:]]'),
			disabled boolean NOT NULL DEFAULT false)`,
		`CREATE TABLE IF NOT EXISTS platform_core.workspaces(
			id text PRIMARY KEY CHECK(id ~ '^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$'),
			name text NOT NULL CHECK(length(btrim(name)) BETWEEN 1 AND 128 AND name !~ '[[:cntrl:]]'),
			owner_account_id text NOT NULL REFERENCES platform_core.accounts(id),
			owner_role text NOT NULL DEFAULT 'owner' CHECK(owner_role='owner'),
			UNIQUE(id,owner_account_id))`,
		`CREATE TABLE IF NOT EXISTS platform_core.memberships(
			workspace_id text NOT NULL REFERENCES platform_core.workspaces(id),
			account_id text NOT NULL REFERENCES platform_core.accounts(id),
			role text NOT NULL CHECK(role IN ('owner','admin','member')),
			owner_link text GENERATED ALWAYS AS (CASE WHEN role='owner' THEN account_id ELSE NULL END) STORED,
			PRIMARY KEY(workspace_id,account_id), UNIQUE(workspace_id,account_id,role),
			FOREIGN KEY(workspace_id,owner_link) REFERENCES platform_core.workspaces(id,owner_account_id) DEFERRABLE INITIALLY DEFERRED)`,
		`DO $$ BEGIN
			IF NOT EXISTS(SELECT 1 FROM pg_constraint WHERE conrelid='platform_core.workspaces'::regclass AND conname='platform_core_owner_membership') THEN
				ALTER TABLE platform_core.workspaces ADD CONSTRAINT platform_core_owner_membership
				FOREIGN KEY(id,owner_account_id,owner_role) REFERENCES platform_core.memberships(workspace_id,account_id,role) DEFERRABLE INITIALLY DEFERRED;
			END IF;
		END $$`,
		`CREATE TABLE IF NOT EXISTS platform_core.guests(
			workspace_id text NOT NULL REFERENCES platform_core.workspaces(id),
			room_id text NOT NULL CHECK(room_id ~ '^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$'),
			game_id text NOT NULL CHECK(game_id ~ '^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$'),
			id text NOT NULL CHECK(id ~ '^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$'),
			expires_at timestamptz NOT NULL CHECK(isfinite(expires_at)),
			disabled boolean NOT NULL DEFAULT false,
			claimed_account_id text REFERENCES platform_core.accounts(id),
			PRIMARY KEY(workspace_id,room_id,game_id,id))`,
		`CREATE UNIQUE INDEX IF NOT EXISTS platform_core_claimed_participation ON platform_core.guests(workspace_id,room_id,game_id,claimed_account_id) WHERE claimed_account_id IS NOT NULL`,
	} {
		if _, err := tx.ExecContext(ctx, statement); err != nil {
			return platformError(err)
		}
	}
	if err := tx.Commit(); err != nil {
		if mapped := platformError(err); mapped == core.ErrConflict {
			return mapped
		}
		return core.ErrOutcomeUnknown
	}
	return nil
}
