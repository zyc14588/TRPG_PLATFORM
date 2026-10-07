// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package postgres

import (
	"context"
	"database/sql"
	"fmt"
	"io"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

type PlatformAuthRepository struct{ data **platformAuthData }
type platformAuthData struct {
	core  *PlatformCoreRepository
	fault func(context.Context, string) error
}

func OpenPlatformAuthRepository(ctx context.Context, dsn string, fault func(context.Context, string) error) (*PlatformAuthRepository, error) {
	r, e := OpenPlatformCoreRepository(ctx, dsn, nil)
	if e != nil {
		return nil, auth.SafeError(e)
	}
	d := &platformAuthData{core: r, fault: fault}
	return &PlatformAuthRepository{data: &d}, nil
}
func (*PlatformAuthRepository) String() string   { return "<authentication repository>" }
func (*PlatformAuthRepository) GoString() string { return "<authentication repository>" }
func (*PlatformAuthRepository) Format(s fmt.State, _ rune) {
	_, _ = io.WriteString(s, "<authentication repository>")
}
func (r *PlatformAuthRepository) state() *platformAuthData {
	if r == nil || r.data == nil {
		return nil
	}
	return *r.data
}
func (r *PlatformAuthRepository) Close() error {
	if r.state() == nil {
		return nil
	}
	return auth.SafeError(r.state().core.Close())
}
func authStorageError(e error) error { return auth.SafeError(platformError(e)) }

// One bounded authentication transaction serializes receipt/session changes.
// Existing core transactions are reused directly, including all owner and
// guest checks; no nested transaction or core policy replacement is involved.
func (r *PlatformAuthRepository) Transact(ctx context.Context, f func(auth.Transaction) error) error {
	d := r.state()
	if d == nil || ctx == nil || ctx.Err() != nil || f == nil {
		return auth.ErrUnavailable
	}
	tx, e := d.core.state().db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelReadCommitted})
	if e != nil {
		return authStorageError(e)
	}
	defer tx.Rollback()
	for _, statement := range []string{`SET LOCAL statement_timeout='3s'`, `SET LOCAL lock_timeout='3s'`, `SELECT pg_advisory_xact_lock(1295143241)`} {
		if _, e := tx.ExecContext(ctx, statement); e != nil {
			return authStorageError(e)
		}
	}
	t := &platformAuthTransaction{tx: tx, core: &platformCoreTransaction{tx: tx, fault: func(ctx context.Context, p string) error { return core.SafeError(injectAuth(ctx, d.fault, p)) }}, fault: d.fault}
	if e := f(t); e != nil {
		return auth.SafeError(e)
	}
	if e := t.inject(ctx, "before-commit"); e != nil {
		return e
	}
	if e := tx.Commit(); e != nil {
		if mapped := authStorageError(e); mapped == auth.ErrConflict {
			return mapped
		}
		return auth.ErrOutcomeUnknown
	}
	if d.fault != nil && d.fault(ctx, "after-commit-unknown") != nil {
		return auth.ErrOutcomeUnknown
	}
	return nil
}
func injectAuth(ctx context.Context, f func(context.Context, string) error, point string) error {
	if ctx.Err() != nil {
		return auth.ErrUnavailable
	}
	if f != nil {
		return auth.SafeError(f(ctx, point))
	}
	return nil
}

type platformAuthTransaction struct {
	tx    *sql.Tx
	core  *platformCoreTransaction
	fault func(context.Context, string) error
}

func (t *platformAuthTransaction) inject(ctx context.Context, p string) error {
	return injectAuth(ctx, t.fault, p)
}
func (t *platformAuthTransaction) Core() core.Transaction { return t.core }
func (t *platformAuthTransaction) Session(ctx context.Context, hash string) (auth.Session, error) {
	v := auth.SessionData{Hash: hash}
	var account, guest, workspace, room, game, successor sql.NullString
	e := t.tx.QueryRowContext(ctx, `SELECT kind,account_id,guest_id,workspace_id,room_id,game_id,expires_at,last_seen,retired,revoked,successor_hash FROM platform_auth.sessions WHERE token_hash=$1 FOR UPDATE`, hash).Scan(&v.Kind, &account, &guest, &workspace, &room, &game, &v.ExpiresAt, &v.LastSeen, &v.Retired, &v.Revoked, &successor)
	if e != nil {
		return auth.Session{}, authStorageError(e)
	}
	v.AccountID = account.String
	v.GuestID = guest.String
	v.Scope = core.Scope{WorkspaceID: workspace.String, RoomID: room.String, GameID: game.String}
	v.SuccessorHash = successor.String
	return auth.StoredSession(v), nil
}
func optional(v string) any {
	if v == "" {
		return nil
	}
	return v
}
func (t *platformAuthTransaction) PutSession(ctx context.Context, session auth.Session) error {
	v := session.StorageValue()
	_, e := t.tx.ExecContext(ctx, `INSERT INTO platform_auth.sessions(token_hash,kind,account_id,guest_id,workspace_id,room_id,game_id,expires_at,last_seen,retired,revoked,successor_hash) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT(token_hash) DO UPDATE SET last_seen=excluded.last_seen,retired=excluded.retired,revoked=excluded.revoked,successor_hash=excluded.successor_hash`, v.Hash, v.Kind, optional(v.AccountID), optional(v.GuestID), optional(v.Scope.WorkspaceID), optional(v.Scope.RoomID), optional(v.Scope.GameID), v.ExpiresAt, v.LastSeen, v.Retired, v.Revoked, optional(v.SuccessorHash))
	if e != nil {
		return authStorageError(e)
	}
	return t.inject(ctx, "after-session")
}
func (t *platformAuthTransaction) Receipt(ctx context.Context, owner, endpoint, key string) (auth.Receipt, error) {
	v := auth.ReceiptData{OwnerHash: owner, Endpoint: endpoint, Key: key}
	e := t.tx.QueryRowContext(ctx, `SELECT request_digest,expires_at,ciphertext FROM platform_auth.receipts WHERE owner_hash=$1 AND endpoint=$2 AND idempotency_key=$3`, owner, endpoint, key).Scan(&v.RequestDigest, &v.ExpiresAt, &v.Ciphertext)
	if e != nil {
		return auth.Receipt{}, authStorageError(e)
	}
	return auth.StoredReceipt(v), nil
}
func (t *platformAuthTransaction) PutReceipt(ctx context.Context, receipt auth.Receipt) error {
	v := receipt.StorageValue()
	_, e := t.tx.ExecContext(ctx, `INSERT INTO platform_auth.receipts(owner_hash,endpoint,idempotency_key,request_digest,expires_at,ciphertext) VALUES($1,$2,$3,$4,$5,$6)`, v.OwnerHash, v.Endpoint, v.Key, v.RequestDigest, v.ExpiresAt, v.Ciphertext)
	if e != nil {
		return authStorageError(e)
	}
	return t.inject(ctx, "after-receipt")
}
func (t *platformAuthTransaction) Credential(ctx context.Context, login string) (string, auth.Password, error) {
	var id string
	var bytes []byte
	e := t.tx.QueryRowContext(ctx, `SELECT account_id,password_digest FROM platform_auth.credentials WHERE login_name=$1 FOR SHARE`, login).Scan(&id, &bytes)
	if e != nil {
		return "", auth.Password{}, authStorageError(e)
	}
	p, e := auth.StoredPassword(bytes)
	clear(bytes)
	return id, p, e
}
func (t *platformAuthTransaction) PutCredential(ctx context.Context, login, id string, password auth.Password) error {
	_, e := t.tx.ExecContext(ctx, `INSERT INTO platform_auth.credentials(login_name,account_id,password_digest) VALUES($1,$2,$3)`, login, id, password.StorageValue())
	if e != nil {
		return authStorageError(e)
	}
	return t.inject(ctx, "after-credential")
}
func (t *platformAuthTransaction) ConsumeGrant(ctx context.Context, hash string) error {
	return auth.SafeError(platformAffected(t.tx.ExecContext(ctx, `UPDATE platform_auth.registration_grants SET consumed=true WHERE token_hash=$1 AND NOT consumed AND expires_at>clock_timestamp()`, hash)))
}
func (t *platformAuthTransaction) AccountCount(ctx context.Context) (int, error) {
	var n int
	e := t.tx.QueryRowContext(ctx, `SELECT count(*) FROM platform_core.accounts`).Scan(&n)
	return n, authStorageError(e)
}
func (t *platformAuthTransaction) PutGrant(ctx context.Context, hash string, expiry time.Time) error {
	_, e := t.tx.ExecContext(ctx, `INSERT INTO platform_auth.registration_grants(token_hash,expires_at) VALUES($1,$2) ON CONFLICT(token_hash) DO NOTHING`, hash, expiry)
	return authStorageError(e)
}
