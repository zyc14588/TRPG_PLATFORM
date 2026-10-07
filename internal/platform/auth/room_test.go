// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package auth

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

type roomTestCore struct {
	core.Transaction
	claimed bool
}

func (c *roomTestCore) ClaimGuest(context.Context, core.Scope, string, string) error {
	c.claimed = true
	return nil
}

type roomTestTx struct {
	Transaction
	c *roomTestCore
	v SessionData
}

func (t *roomTestTx) Core() core.Transaction { return t.c }
func (t *roomTestTx) Session(context.Context, string) (Session, error) {
	return StoredSession(t.v), nil
}

type roomTestRepo struct{ tx *roomTestTx }

func (r roomTestRepo) Transact(_ context.Context, f func(Transaction) error) error { return f(r.tx) }

type roomTestGuard struct {
	called bool
	err    error
}

func (g *roomTestGuard) BeforeRoomClaim(context.Context, core.Transaction, core.Scope, string, string) error {
	g.called = true
	return g.err
}
func TestRoomTransactionSessionBindingAndLifetime(t *testing.T) {
	ctx := context.Background()
	scope := core.Scope{WorkspaceID: "w", RoomID: "r", GameID: "g"}
	original := &roomTestTx{c: &roomTestCore{}, v: SessionData{Hash: "synthetic-private-marker", Kind: "guest", GuestID: "guest", Scope: scope}}
	guard := &roomTestGuard{}
	repo, e := newRoomRepository(roomTestRepo{original}, guard)
	if e != nil {
		t.Fatal("wrapper construction")
	}
	var escaped core.Transaction
	e = repo.Transact(ctx, func(tx Transaction) error {
		escaped = tx.Core()
		if _, e := RoomAdmissionSession(tx.Core()); e != ErrDenied {
			t.Fatal("unbound session accepted")
		}
		if _, e := tx.Session(ctx, "hash"); e != nil {
			t.Fatal("session lookup")
		}
		s, e := RoomAdmissionSession(tx.Core())
		if e != nil || s.StorageValue().Hash != original.v.Hash {
			t.Fatal("actual session not bound")
		}
		if RoomStorageCore(tx.Core()) != original.c {
			t.Fatal("storage transaction replaced")
		}
		original.v.Hash = "later-historical-session"
		if _, e := tx.Session(ctx, "historical"); e != nil {
			t.Fatal("historical session lookup")
		}
		bound, e := RoomAdmissionSession(tx.Core())
		if e != nil || bound.StorageValue().Hash != "synthetic-private-marker" {
			t.Fatal("historical lookup replaced cookie binding")
		}
		if e = tx.Core().ClaimGuest(ctx, core.Scope{WorkspaceID: "other", RoomID: "r", GameID: "g"}, "guest", "a"); e != core.ErrDenied || guard.called || original.c.claimed {
			t.Fatal("cross-scope claim reached storage")
		}
		guard.err = core.ErrConflict
		if e = tx.Core().ClaimGuest(ctx, scope, "guest", "a"); e != core.ErrConflict || original.c.claimed {
			t.Fatal("claim conflict bypassed")
		}
		guard.err = nil
		if e = tx.Core().ClaimGuest(ctx, scope, "guest", "a"); e != nil || !original.c.claimed {
			t.Fatal("valid claim did not reach same transaction")
		}
		return nil
	})
	if e != nil {
		t.Fatal("transaction wrapper failed")
	}
	if _, e = RoomAdmissionSession(escaped); e != ErrDenied || RoomStorageCore(escaped) != nil {
		t.Fatal("expired transaction capability survived")
	}
	if _, e = RoomAdmissionSession(original.c); e != ErrDenied {
		t.Fatal("caller-made core session accepted")
	}
	for _, verb := range []string{"%v", "%#v", "%+v", "%d", "%!"} {
		if strings.Contains(fmt.Sprintf(verb, escaped), "synthetic-private-marker") {
			t.Fatal("bound session leaked")
		}
	}
	if b, e := json.Marshal(escaped); e == nil || bytes.Contains(b, []byte("synthetic-private-marker")) {
		t.Fatal("bound transaction export accepted")
	}
}

type roomDenyVerifier struct{}

func (roomDenyVerifier) Verify(context.Context, core.Transaction, BrowserCredential) (core.Guest, error) {
	return core.Guest{}, ErrDenied
}
func TestRoomAuthorityRequiresIndependentKeysAndVerifier(t *testing.T) {
	b, e := os.ReadFile("../../../schemas/platform/platform-auth-api-v1.schema.json")
	if e != nil {
		t.Fatal("Schema read")
	}
	repo := roomTestRepo{&roomTestTx{c: &roomTestCore{}, v: SessionData{ExpiresAt: time.Now()}}}
	a := bytes.Repeat([]byte{1}, 32)
	r := bytes.Repeat([]byte{2}, 32)
	i := bytes.Repeat([]byte{3}, 32)
	g := &roomTestGuard{}
	for _, k := range [][]byte{a, r, i[:31]} {
		if _, e = NewRoomAuthority(repo, a, r, k, b, roomDenyVerifier{}, g); e != ErrInvalid {
			t.Fatal("invalid key separation accepted")
		}
	}
	if _, e = NewRoomAuthority(repo, a, r, i, b, nil, g); e != ErrInvalid {
		t.Fatal("missing verifier accepted")
	}
	if _, e = NewRoomAuthority(repo, a, r, i, b, roomDenyVerifier{}, nil); e != ErrInvalid {
		t.Fatal("missing claim guard accepted")
	}
	authority, e := NewRoomAuthority(repo, a, r, i, b, roomDenyVerifier{}, g)
	if e != nil {
		t.Fatal("valid authority denied")
	}
	clear(i)
	if authority.InvitationKeyMatches(i) {
		t.Fatal("invitation key caller mutation accepted")
	}
}
