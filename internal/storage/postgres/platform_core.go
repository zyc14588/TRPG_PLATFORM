// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package postgres

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"io"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

type PlatformCoreRepository struct{ data **platformCoreData }

type platformCoreData struct {
	db    *sql.DB
	fault func(context.Context, string) error
}

// OpenPlatformCoreRepository is a trusted operator/composition seam. It does
// not bootstrap implicitly, retain a separate DSN, or return driver errors.
func OpenPlatformCoreRepository(ctx context.Context, dsn string, fault func(context.Context, string) error) (*PlatformCoreRepository, error) {
	if ctx == nil || ctx.Err() != nil || dsn == "" {
		return nil, core.ErrUnavailable
	}
	db, err := sql.Open("pgx", dsn)
	if err != nil {
		return nil, core.ErrUnavailable
	}
	db.SetMaxOpenConns(8)
	db.SetMaxIdleConns(2)
	if err := db.PingContext(ctx); err != nil {
		_ = db.Close()
		return nil, core.ErrUnavailable
	}
	d := &platformCoreData{db: db, fault: fault}
	return &PlatformCoreRepository{data: &d}, nil
}

func (*PlatformCoreRepository) String() string   { return "<platform repository>" }
func (*PlatformCoreRepository) GoString() string { return "<platform repository>" }
func (*PlatformCoreRepository) Format(s fmt.State, _ rune) {
	_, _ = io.WriteString(s, "<platform repository>")
}

func (r *PlatformCoreRepository) state() *platformCoreData {
	if r == nil || r.data == nil || *r.data == nil {
		return nil
	}
	return *r.data
}

func (r *PlatformCoreRepository) Close() error {
	d := r.state()
	if d == nil {
		return nil
	}
	return platformError(d.db.Close())
}

func platformError(err error) error {
	if err == nil {
		return nil
	}
	if errors.Is(err, sql.ErrNoRows) {
		return core.ErrDenied
	}
	var state interface{ SQLState() string }
	if errors.As(err, &state) {
		switch state.SQLState() {
		case "23505", "23503", "23514", "23502", "22001":
			return core.ErrConflict
		}
	}
	return core.SafeError(err)
}

// A workspace row lock serializes every workspace authorization and mutation.
// Account reads hold shared row locks against revocation. These locks stay held
// until commit, so membership changes cannot race an already authorized write.
func (r *PlatformCoreRepository) Transact(ctx context.Context, f func(core.Transaction) error) error {
	d := r.state()
	if d == nil || ctx == nil || ctx.Err() != nil || f == nil {
		return core.ErrUnavailable
	}
	tx, err := d.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelReadCommitted})
	if err != nil {
		return platformError(err)
	}
	defer tx.Rollback()
	for _, statement := range []string{`SET LOCAL statement_timeout='3s'`, `SET LOCAL lock_timeout='3s'`} {
		if _, err := tx.ExecContext(ctx, statement); err != nil {
			return platformError(err)
		}
	}
	t := &platformCoreTransaction{tx: tx, fault: d.fault}
	if err := f(t); err != nil {
		return core.SafeError(err)
	}
	if err := t.inject(ctx, "before-commit"); err != nil {
		return err
	}
	if err := tx.Commit(); err != nil {
		mapped := platformError(err)
		if mapped == core.ErrConflict {
			return mapped // A deferred constraint violation is a known rollback.
		}
		return core.ErrOutcomeUnknown
	}
	return nil
}

type platformCoreTransaction struct {
	tx    *sql.Tx
	fault func(context.Context, string) error
}

func (t *platformCoreTransaction) inject(ctx context.Context, point string) error {
	if ctx.Err() != nil {
		return core.ErrUnavailable
	}
	if t.fault != nil {
		return core.SafeError(t.fault(ctx, point))
	}
	return nil
}

func (t *platformCoreTransaction) Now(ctx context.Context) (time.Time, error) {
	var now time.Time
	err := t.tx.QueryRowContext(ctx, `SELECT clock_timestamp()`).Scan(&now)
	return now, platformError(err)
}

func (t *platformCoreTransaction) Account(ctx context.Context, id string) (core.Account, error) {
	var a core.Account
	err := t.tx.QueryRowContext(ctx, `SELECT id,display_name,disabled FROM platform_core.accounts WHERE id=$1 FOR SHARE`, id).Scan(&a.ID, &a.DisplayName, &a.Disabled)
	return a, platformError(err)
}

func (t *platformCoreTransaction) InsertAccount(ctx context.Context, a core.Account) error {
	_, err := t.tx.ExecContext(ctx, `INSERT INTO platform_core.accounts(id,display_name,disabled) VALUES($1,$2,$3)`, a.ID, a.DisplayName, a.Disabled)
	return platformError(err)
}

func platformAffected(result sql.Result, err error) error {
	if err != nil {
		return platformError(err)
	}
	n, err := result.RowsAffected()
	if err != nil {
		return platformError(err)
	}
	if n != 1 {
		return core.ErrDenied
	}
	return nil
}

func (t *platformCoreTransaction) DisableAccount(ctx context.Context, id string) error {
	return platformAffected(t.tx.ExecContext(ctx, `UPDATE platform_core.accounts SET disabled=true WHERE id=$1`, id))
}

func (t *platformCoreTransaction) Workspace(ctx context.Context, id string) (core.Workspace, error) {
	var w core.Workspace
	err := t.tx.QueryRowContext(ctx, `SELECT id,name,owner_account_id FROM platform_core.workspaces WHERE id=$1 FOR UPDATE`, id).Scan(&w.ID, &w.Name, &w.OwnerID)
	return w, platformError(err)
}

func (t *platformCoreTransaction) InsertWorkspace(ctx context.Context, w core.Workspace) error {
	_, err := t.tx.ExecContext(ctx, `INSERT INTO platform_core.workspaces(id,name,owner_account_id) VALUES($1,$2,$3)`, w.ID, w.Name, w.OwnerID)
	if err != nil {
		return platformError(err)
	}
	return t.inject(ctx, "after-workspace")
}

func (t *platformCoreTransaction) Membership(ctx context.Context, workspace, account string) (core.Membership, error) {
	var m core.Membership
	err := t.tx.QueryRowContext(ctx, `SELECT workspace_id,account_id,role FROM platform_core.memberships WHERE workspace_id=$1 AND account_id=$2`, workspace, account).Scan(&m.WorkspaceID, &m.AccountID, &m.Role)
	return m, platformError(err)
}

func (t *platformCoreTransaction) PutMembership(ctx context.Context, m core.Membership) error {
	_, err := t.tx.ExecContext(ctx, `INSERT INTO platform_core.memberships(workspace_id,account_id,role) VALUES($1,$2,$3) ON CONFLICT(workspace_id,account_id) DO UPDATE SET role=excluded.role`, m.WorkspaceID, m.AccountID, m.Role)
	if err != nil {
		return platformError(err)
	}
	return t.inject(ctx, "after-membership")
}

func (t *platformCoreTransaction) DeleteMembership(ctx context.Context, workspace, account string) error {
	return platformAffected(t.tx.ExecContext(ctx, `DELETE FROM platform_core.memberships WHERE workspace_id=$1 AND account_id=$2`, workspace, account))
}

func (t *platformCoreTransaction) Guest(ctx context.Context, scope core.Scope, id string) (core.Guest, error) {
	g := core.Guest{Scope: scope}
	var claimed sql.NullString
	err := t.tx.QueryRowContext(ctx, `SELECT id,expires_at,disabled,claimed_account_id FROM platform_core.guests WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND id=$4 FOR UPDATE`, scope.WorkspaceID, scope.RoomID, scope.GameID, id).Scan(&g.ID, &g.ExpiresAt, &g.Disabled, &claimed)
	g.ClaimedAccountID = claimed.String
	return g, platformError(err)
}

func (t *platformCoreTransaction) InsertGuest(ctx context.Context, g core.Guest) error {
	// Provisioning cannot smuggle an already claimed identity into the relation.
	if g.ClaimedAccountID != "" || g.Disabled {
		return core.ErrDenied
	}
	_, err := t.tx.ExecContext(ctx, `INSERT INTO platform_core.guests(workspace_id,room_id,game_id,id,expires_at) VALUES($1,$2,$3,$4,$5)`, g.WorkspaceID, g.RoomID, g.GameID, g.ID, g.ExpiresAt)
	return platformError(err)
}

func (t *platformCoreTransaction) ClaimGuest(ctx context.Context, scope core.Scope, id, account string) error {
	err := platformAffected(t.tx.ExecContext(ctx, `UPDATE platform_core.guests SET claimed_account_id=$5 WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND id=$4 AND NOT disabled AND claimed_account_id IS NULL AND expires_at>clock_timestamp()`, scope.WorkspaceID, scope.RoomID, scope.GameID, id, account))
	if err != nil {
		return err
	}
	return t.inject(ctx, "after-claim")
}

func (t *platformCoreTransaction) DisableGuest(ctx context.Context, scope core.Scope, id string) error {
	return platformAffected(t.tx.ExecContext(ctx, `UPDATE platform_core.guests SET disabled=true WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND id=$4`, scope.WorkspaceID, scope.RoomID, scope.GameID, id))
}

func (t *platformCoreTransaction) Participation(ctx context.Context, scope core.Scope, account string) (core.Participation, error) {
	p := core.Participation{Scope: scope, AccountID: account}
	err := t.tx.QueryRowContext(ctx, `SELECT id FROM platform_core.guests WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND claimed_account_id=$4 AND NOT disabled AND expires_at>clock_timestamp()`, scope.WorkspaceID, scope.RoomID, scope.GameID, account).Scan(&p.GuestID)
	return p, platformError(err)
}
