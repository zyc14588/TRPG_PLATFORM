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

type inspectionCore struct {
	core.Transaction
	now                                          time.Time
	accountDisabled, guestDisabled, guestClaimed bool
	guestExpired                                 bool
}

func (c *inspectionCore) Now(context.Context) (time.Time, error) { return c.now, nil }
func (c *inspectionCore) Account(_ context.Context, id string) (core.Account, error) {
	return core.Account{ID: id, Disabled: c.accountDisabled}, nil
}
func (c *inspectionCore) Workspace(_ context.Context, id string) (core.Workspace, error) {
	return core.Workspace{ID: id}, nil
}
func (c *inspectionCore) Guest(_ context.Context, scope core.Scope, id string) (core.Guest, error) {
	v := core.Guest{Scope: scope, ID: id, ExpiresAt: c.now.Add(time.Hour), Disabled: c.guestDisabled}
	if c.guestExpired {
		v.ExpiresAt = c.now
	}
	if c.guestClaimed {
		v.ClaimedAccountID = "claimed-account"
	}
	return v, nil
}

type inspectionTx struct {
	Transaction
	c       *inspectionCore
	session SessionData
	writes  int
}

func (t *inspectionTx) Core() core.Transaction { return t.c }
func (t *inspectionTx) Session(_ context.Context, hash string) (Session, error) {
	if hash != t.session.Hash {
		return Session{}, ErrDenied
	}
	return StoredSession(t.session), nil
}
func (t *inspectionTx) PutSession(_ context.Context, v Session) error {
	t.session = v.StorageValue()
	t.writes++
	return nil
}

type inspectionRepo struct{ tx *inspectionTx }

func (r inspectionRepo) Transact(_ context.Context, f func(Transaction) error) error { return f(r.tx) }
func newInspectionFixture(t *testing.T) (*RoomAuthority, *inspectionTx, BrowserCredential) {
	t.Helper()
	b, e := os.ReadFile("../../../schemas/platform/platform-auth-api-v1.schema.json")
	if e != nil {
		t.Fatal("Schema fixture missing")
	}
	now := time.Now().UTC()
	cookie := BrowserCookie(strings.Repeat("A", 43))
	tx := &inspectionTx{c: &inspectionCore{now: now}, session: SessionData{Hash: tokenHash(cookie.StorageValue()), Kind: "account", AccountID: "account", ExpiresAt: now.Add(time.Hour), LastSeen: now}}
	a, e := NewRoomAuthority(inspectionRepo{tx}, bytes.Repeat([]byte{1}, 32), bytes.Repeat([]byte{2}, 32), bytes.Repeat([]byte{3}, 32), b, roomDenyVerifier{}, &roomTestGuard{})
	if e != nil {
		t.Fatal("authority fixture construction")
	}
	return a, tx, cookie
}

func TestInspectSharesCookieSessionBindingWithoutAdmissionCharges(t *testing.T) {
	a, tx, cookie := newInspectionFixture(t)
	var escaped core.Transaction
	calls := 0
	for n := 0; n < 100; n++ {
		e := a.Inspect(context.Background(), cookie, "", false, func(ctx context.Context, bound Transaction, v SessionData) error {
			saved, e := RoomAdmissionSession(bound.Core())
			if e != nil || saved.StorageValue().Hash != v.Hash || RoomStorageCore(bound.Core()) != tx.c {
				t.Fatal("actual authenticated transaction binding lost")
			}
			escaped = bound.Core()
			calls++
			return nil
		})
		if e != nil {
			t.Fatal("internal revalidation unexpectedly denied")
		}
	}
	if calls != 100 || tx.writes != 0 || a.state().service.state().global.count != 0 || len(a.state().service.state().rates) != 0 {
		t.Fatal("read inspection touched user admission or session lifetime")
	}
	if _, e := RoomAdmissionSession(escaped); e != ErrDenied || RoomStorageCore(escaped) != nil {
		t.Fatal("inspection transaction escaped its lifetime")
	}
	cmd := RoomSecret(RoomCommandData{Action: "user-admission-check", TargetKey: "workspace/room", Write: false})
	cb := RoomCallbacks{Apply: func(context.Context, Transaction, SessionData) (Outcome, error) {
		return RoomOutcome([]byte("bounded-ok")), nil
	}, Replay: func(context.Context, Transaction, SessionData, Outcome) error { return ErrDenied }}
	for n := 0; n < 10; n++ {
		if _, e := a.Do(context.Background(), cookie, "", "", "network", cmd, cb); e != nil {
			t.Fatal("user admission allowance changed")
		}
	}
	if _, e := a.Do(context.Background(), cookie, "", "", "network", cmd, cb); e != ErrRateLimited {
		t.Fatal("user admission limiter bypassed")
	}
}
func TestInspectRevalidatesCookieAccountAndSessionLifetime(t *testing.T) {
	for _, tc := range []struct {
		name   string
		change func(*inspectionTx)
	}{
		{"revoked", func(tx *inspectionTx) { tx.session.Revoked = true }},
		{"retired", func(tx *inspectionTx) { tx.session.Retired = true }},
		{"expired", func(tx *inspectionTx) { tx.session.ExpiresAt = tx.c.now }},
		{"idle", func(tx *inspectionTx) { tx.session.LastSeen = tx.c.now.Add(-30 * time.Minute) }},
		{"disabled-account", func(tx *inspectionTx) { tx.c.accountDisabled = true }},
		{"anonymous", func(tx *inspectionTx) { tx.session.Kind = "preauth"; tx.session.AccountID = "" }},
	} {
		t.Run(tc.name, func(t *testing.T) {
			a, tx, cookie := newInspectionFixture(t)
			tc.change(tx)
			called := false
			e := a.Inspect(context.Background(), cookie, "", false, func(context.Context, Transaction, SessionData) error { called = true; return nil })
			if e != ErrUnauthenticated || called {
				t.Fatal("invalid live session reached inspection callback")
			}
		})
	}
	a, _, _ := newInspectionFixture(t)
	called := false
	if e := a.Inspect(context.Background(), BrowserCookie(strings.Repeat("B", 43)), "", false, func(context.Context, Transaction, SessionData) error { called = true; return nil }); e != ErrUnauthenticated || called {
		t.Fatal("client supplied identity accepted")
	}
}
func TestInspectGuestBoundaryExpiryClaimAndRevocation(t *testing.T) {
	for _, name := range []string{"live", "expired", "claimed", "disabled"} {
		t.Run(name, func(t *testing.T) {
			a, tx, cookie := newInspectionFixture(t)
			scope := core.Scope{WorkspaceID: "w", RoomID: "r", GameID: "g"}
			tx.session.Kind = "guest"
			tx.session.AccountID = ""
			tx.session.GuestID = "guest"
			tx.session.Scope = scope
			tx.c.guestExpired = name == "expired"
			tx.c.guestClaimed = name == "claimed"
			tx.c.guestDisabled = name == "disabled"
			called := false
			e := a.Inspect(context.Background(), cookie, "", false, func(_ context.Context, _ Transaction, v SessionData) error {
				called = true
				if v.Scope != scope || v.GuestID != "guest" {
					t.Fatal("guest scope substituted")
				}
				return nil
			})
			if name == "live" {
				if e != nil || !called {
					t.Fatal("live scoped guest denied")
				}
			} else if e != ErrUnauthenticated || called {
				t.Fatal("invalid guest reached callback")
			}
		})
	}
}
func TestInspectMutationUsesExistingCSRFAndTouchesOnlyVerifiedSession(t *testing.T) {
	a, tx, cookie := newInspectionFixture(t)
	calls := 0
	cb := func(context.Context, Transaction, SessionData) error { calls++; return nil }
	if e := a.Inspect(context.Background(), cookie, "", true, cb); e != ErrInvalid || calls != 0 || tx.writes != 0 {
		t.Fatal("missing CSRF admitted")
	}
	if e := a.Inspect(context.Background(), cookie, strings.Repeat("B", 43), true, cb); e != ErrDenied || calls != 0 || tx.writes != 0 {
		t.Fatal("wrong CSRF admitted")
	}
	expected := a.state().service.csrf(tx.session)
	tx.c.now = tx.c.now.Add(time.Second)
	if e := a.Inspect(context.Background(), cookie, expected, true, cb); e != nil || calls != 1 || tx.writes != 1 || !tx.session.LastSeen.Equal(sessionTime(tx.c.now)) {
		t.Fatal("verified mutation inspection failed")
	}
	if a.state().service.state().global.count != 0 {
		t.Fatal("internal mutation inspection charged admission")
	}
}
func TestInspectRejectsUnavailableBoundaryAndSanitizesCallbackErrors(t *testing.T) {
	a, _, cookie := newInspectionFixture(t)
	cb := func(context.Context, Transaction, SessionData) error { return fmt.Errorf("private-driver-diagnostic") }
	if e := a.Inspect(context.Background(), cookie, "", false, cb); e != ErrUnavailable {
		t.Fatal("private callback error escaped")
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	for _, e := range []error{a.Inspect(nil, cookie, "", false, cb), a.Inspect(ctx, cookie, "", false, cb), a.Inspect(context.Background(), cookie, "", false, nil), (*RoomAuthority)(nil).Inspect(context.Background(), cookie, "", false, cb)} {
		if e != ErrUnavailable {
			t.Fatal("unavailable inspection accepted")
		}
	}
}
