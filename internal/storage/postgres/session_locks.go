// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package postgres

import (
	"context"
	"database/sql"
	"errors"
	"reflect"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/migration"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

func readLocks(ctx context.Context, tx *sql.Tx, b data.Binding) (data.SessionLock, data.SessionLock, error) {
	var origin, active data.SessionLock
	var first, last []byte
	err := tx.QueryRowContext(ctx, `SELECT origin,active FROM host_command.session_locks WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session).Scan(&first, &last)
	if errors.Is(err, sql.ErrNoRows) {
		return origin, active, data.ErrNotFound
	}
	if err != nil {
		return origin, active, err
	}
	if checkpoint.StrictDecode(first, &origin, 256<<10) != nil || checkpoint.StrictDecode(last, &active, 256<<10) != nil || data.ValidateSessionLock(origin) != nil || data.ValidateSessionLock(active) != nil || active.GraphHash != b.GraphHash {
		return origin, active, eventstore.ErrHistory
	}
	return origin, active, nil
}

func historyLocks(h data.ReplayHistory, active data.SessionLock) ([]data.SessionLock, error) {
	if h.OriginLock == nil {
		return nil, data.ErrNotFound
	}
	current := *h.OriginLock
	locks := []data.SessionLock{current}
	seen := map[string]data.SessionLock{current.GraphHash: current}
	for _, r := range h.Records {
		if r.Header.Binding.GraphHash != current.GraphHash {
			return nil, eventstore.ErrHistory
		}
		if m := r.Migration; m != nil {
			if !reflect.DeepEqual(m.From, current) {
				return nil, eventstore.ErrHistory
			}
			current = m.To
			if old, ok := seen[current.GraphHash]; ok {
				if !reflect.DeepEqual(old, current) {
					return nil, eventstore.ErrHistory
				}
			} else {
				seen[current.GraphHash] = current
				locks = append(locks, current)
			}
		}
	}
	if !reflect.DeepEqual(current, active) {
		return nil, eventstore.ErrHistory
	}
	return locks, nil
}

func (r *HostRepository) ReadSessionLocks(ctx context.Context, g *store.Graph, b data.Binding) ([]data.SessionLock, error) {
	tx, expected, err := r.installedTransaction(ctx, g, b)
	if err != nil {
		return nil, err
	}
	defer tx.Rollback()
	_, active, err := readLocks(ctx, tx, b)
	if err != nil {
		return nil, err
	}
	authenticated, err := migration.ExactLock(g)
	if err != nil || !reflect.DeepEqual(authenticated, active) {
		return nil, data.ErrDenied
	}
	h, err := readReplay(ctx, tx, b, expected)
	if err != nil {
		return nil, err
	}
	locks, err := historyLocks(h, active)
	if err != nil {
		return nil, err
	}
	if err = tx.Commit(); err != nil {
		return nil, err
	}
	return locks, nil
}
