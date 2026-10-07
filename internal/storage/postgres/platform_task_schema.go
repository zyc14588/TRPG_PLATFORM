// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package postgres

import (
	"context"

	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

// Bootstrap is an explicit operator action. Jobs are a recoverable operational
// projection of committed Host intents; this schema confers no game writes.
func (s *PlatformTaskStorage) Bootstrap(ctx context.Context) error {
	if s.state() == nil || ctx == nil || ctx.Err() != nil {
		return task.ErrUnavailable
	}
	tx, e := s.state().repo.state().core.state().db.BeginTx(ctx, nil)
	if e != nil {
		return task.ErrUnavailable
	}
	defer tx.Rollback()
	for _, q := range []string{
		`SET LOCAL statement_timeout='3s'`, `SET LOCAL lock_timeout='3s'`,
		`CREATE SCHEMA IF NOT EXISTS platform_task`,
		`CREATE TABLE IF NOT EXISTS platform_task.jobs(
		 workspace text NOT NULL,session text NOT NULL,task_id text NOT NULL,outbox_id text NOT NULL,
		 status text NOT NULL CHECK(status IN ('queued','running','ready','delivering','done','failed','cancelled','expired','stale')),
		 attempt integer NOT NULL CHECK(attempt BETWEEN 0 AND 4),
		 lease_owner text NOT NULL,lease_digest text NOT NULL CHECK(lease_digest='' OR lease_digest ~ '^[a-f0-9]{64}$'),
		 lease_until timestamptz NOT NULL,expires timestamptz NOT NULL,
		 body bytea NOT NULL CHECK(octet_length(body) BETWEEN 1 AND 524288),
		 PRIMARY KEY(workspace,session,task_id),UNIQUE(workspace,session,outbox_id),
		 FOREIGN KEY(workspace,session,task_id) REFERENCES host_command.tasks(workspace,session,id),
		 FOREIGN KEY(workspace,session,outbox_id) REFERENCES host_command.outbox(workspace,session,id))`,
		`CREATE INDEX IF NOT EXISTS platform_task_pending ON platform_task.jobs(workspace,status,lease_until)`,
	} {
		if _, e = tx.ExecContext(ctx, q); e != nil {
			return task.ErrUnavailable
		}
	}
	if e = tx.Commit(); e != nil {
		return task.ErrUnavailable
	}
	return nil
}
