// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package auth

import (
	"context"
	"fmt"
	"io"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

type RoomClaimGuard interface {
	BeforeRoomClaim(context.Context, core.Transaction, core.Scope, string, string) error
}

type roomRepository struct{ data **roomRepositoryData }
type roomRepositoryData struct {
	repo   Repository
	claims RoomClaimGuard
}

func newRoomRepository(repo Repository, claims RoomClaimGuard) (Repository, error) {
	if repo == nil || claims == nil {
		return nil, ErrInvalid
	}
	d := &roomRepositoryData{repo: repo, claims: claims}
	return &roomRepository{data: &d}, nil
}
func (*roomRepository) Format(s fmt.State, _ rune) {
	_, _ = io.WriteString(s, "<room authentication repository>")
}
func (*roomRepository) MarshalJSON() ([]byte, error) { return nil, ErrDenied }
func (r *roomRepository) Transact(ctx context.Context, f func(Transaction) error) error {
	if r == nil || r.data == nil || *r.data == nil || f == nil {
		return ErrUnavailable
	}
	d := *r.data
	return SafeError(d.repo.Transact(ctx, func(original Transaction) error {
		if original == nil || original.Core() == nil {
			return ErrUnavailable
		}
		cd := &roomCoreData{tx: original.Core(), claims: d.claims, alive: true}
		c := &roomBoundCore{data: &cd}
		defer func() { cd.alive = false }()
		td := &roomAuthTxData{tx: original, core: c}
		return f(&roomAuthTx{data: &td})
	}))
}

// The verifier obtains the actual cookie session from the same transaction
// that authenticated it. Neither an HTTP field nor a caller-made context value
// can manufacture this binding. Guest exchange keeps its approved wire fields.
type roomBoundCore struct{ data **roomCoreData }
type roomCoreData struct {
	tx           core.Transaction
	claims       RoomClaimGuard
	session      SessionData
	bound, alive bool
}

func (c *roomBoundCore) state() *roomCoreData {
	if c == nil || c.data == nil {
		return nil
	}
	return *c.data
}
func (*roomBoundCore) Format(s fmt.State, _ rune) {
	_, _ = io.WriteString(s, "<room bound core transaction>")
}
func (*roomBoundCore) MarshalJSON() ([]byte, error) { return nil, ErrDenied }
func RoomAdmissionSession(tx core.Transaction) (Session, error) {
	c, ok := tx.(*roomBoundCore)
	if !ok || c.state() == nil || !c.state().alive || !c.state().bound {
		return Session{}, ErrDenied
	}
	return StoredSession(c.state().session), nil
}

// RoomStorageCore exposes only the existing typed core interface at the trusted
// storage seam, never SQL, a database handle or connection credentials.
func RoomStorageCore(tx core.Transaction) core.Transaction {
	if c, ok := tx.(*roomBoundCore); ok {
		if c.state() == nil || !c.state().alive {
			return nil
		}
		return c.state().tx
	}
	return tx
}
func (c *roomBoundCore) ClaimGuest(ctx context.Context, scope core.Scope, id, account string) error {
	d := c.state()
	if d == nil || !d.alive || !d.bound || d.session.Kind != "guest" || d.session.GuestID != id || d.session.Scope != scope {
		return core.ErrDenied
	}
	if e := d.claims.BeforeRoomClaim(ctx, d.tx, scope, id, account); e != nil {
		return core.SafeError(e)
	}
	return d.tx.ClaimGuest(ctx, scope, id, account)
}

type roomAuthTx struct{ data **roomAuthTxData }
type roomAuthTxData struct {
	tx   Transaction
	core *roomBoundCore
}

func (t *roomAuthTx) state() *roomAuthTxData {
	if t == nil || t.data == nil {
		return nil
	}
	return *t.data
}
func (*roomAuthTx) Format(s fmt.State, _ rune) {
	_, _ = io.WriteString(s, "<room authentication transaction>")
}
func (*roomAuthTx) MarshalJSON() ([]byte, error) { return nil, ErrDenied }
func (t *roomAuthTx) Core() core.Transaction     { return t.state().core }
func (t *roomAuthTx) Session(ctx context.Context, hash string) (Session, error) {
	v, e := t.state().tx.Session(ctx, hash)
	if e == nil {
		d := t.state().core.state()
		if !d.bound {
			d.session = v.StorageValue()
			d.bound = true
		}
	}
	return v, e
}
func (t *roomAuthTx) PutSession(ctx context.Context, v Session) error {
	return t.state().tx.PutSession(ctx, v)
}
func (t *roomAuthTx) Receipt(ctx context.Context, owner, endpoint, key string) (Receipt, error) {
	return t.state().tx.Receipt(ctx, owner, endpoint, key)
}
func (t *roomAuthTx) PutReceipt(ctx context.Context, v Receipt) error {
	return t.state().tx.PutReceipt(ctx, v)
}
func (t *roomAuthTx) Credential(ctx context.Context, login string) (string, Password, error) {
	return t.state().tx.Credential(ctx, login)
}
func (t *roomAuthTx) PutCredential(ctx context.Context, login, id string, v Password) error {
	return t.state().tx.PutCredential(ctx, login, id, v)
}
func (t *roomAuthTx) ConsumeGrant(ctx context.Context, hash string) error {
	return t.state().tx.ConsumeGrant(ctx, hash)
}
func (t *roomAuthTx) AccountCount(ctx context.Context) (int, error) {
	return t.state().tx.AccountCount(ctx)
}
func (t *roomAuthTx) PutGrant(ctx context.Context, hash string, expiry time.Time) error {
	return t.state().tx.PutGrant(ctx, hash, expiry)
}

func (c *roomBoundCore) Now(ctx context.Context) (time.Time, error) { return c.state().tx.Now(ctx) }
func (c *roomBoundCore) Account(ctx context.Context, id string) (core.Account, error) {
	return c.state().tx.Account(ctx, id)
}
func (c *roomBoundCore) InsertAccount(ctx context.Context, v core.Account) error {
	return c.state().tx.InsertAccount(ctx, v)
}
func (c *roomBoundCore) DisableAccount(ctx context.Context, id string) error {
	return c.state().tx.DisableAccount(ctx, id)
}
func (c *roomBoundCore) Workspace(ctx context.Context, id string) (core.Workspace, error) {
	return c.state().tx.Workspace(ctx, id)
}
func (c *roomBoundCore) InsertWorkspace(ctx context.Context, v core.Workspace) error {
	return c.state().tx.InsertWorkspace(ctx, v)
}
func (c *roomBoundCore) Membership(ctx context.Context, workspace, account string) (core.Membership, error) {
	return c.state().tx.Membership(ctx, workspace, account)
}
func (c *roomBoundCore) PutMembership(ctx context.Context, v core.Membership) error {
	return c.state().tx.PutMembership(ctx, v)
}
func (c *roomBoundCore) DeleteMembership(ctx context.Context, workspace, account string) error {
	return c.state().tx.DeleteMembership(ctx, workspace, account)
}
func (c *roomBoundCore) Guest(ctx context.Context, scope core.Scope, id string) (core.Guest, error) {
	return c.state().tx.Guest(ctx, scope, id)
}
func (c *roomBoundCore) InsertGuest(ctx context.Context, v core.Guest) error {
	return c.state().tx.InsertGuest(ctx, v)
}
func (c *roomBoundCore) DisableGuest(ctx context.Context, scope core.Scope, id string) error {
	return c.state().tx.DisableGuest(ctx, scope, id)
}
func (c *roomBoundCore) Participation(ctx context.Context, scope core.Scope, account string) (core.Participation, error) {
	return c.state().tx.Participation(ctx, scope, account)
}
