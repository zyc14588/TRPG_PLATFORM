//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package model_config_test

import (
	"bytes"
	"context"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/credential"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

func TestDefaultRouteAndAdvancedCertifiedFallback(t *testing.T) {
	f := newFixture(t, nil)
	c, r := f.configured(t)
	if c.StorageValue().Primary.ID != "default-model" || c.StorageValue().Tuple.Model != "fixture:small" {
		t.Fatal("server default route not selected")
	}
	need(t, f.check(t, f.models, f.proof()))
	d := r.StorageValue()
	d.ExpectedVersion = 1
	d.FallbackIDs = []string{"approved-fallback"}
	updated, e := f.models.Configure(f.ctx, f.caller(f.owner, token(t)), model.NewConfigureRequest(d))
	need(t, e)
	if len(updated.StorageValue().Fallbacks) != 1 || updated.StorageValue().Version != 2 {
		t.Fatal("explicit qualified fallback not stored")
	}
	need(t, f.check(t, f.models, f.proof()))
}
func TestCredentialCiphertextAndPrivateExports(t *testing.T) {
	f := newFixture(t, nil)
	config, _ := f.configured(t)
	var record credential.Record
	f.inspect(t, f.owner, func(tx auth.Transaction) error {
		mt, e := f.mt.Bind(tx.Core())
		if e != nil {
			return e
		}
		record, e = mt.Credential(f.ctx, f.scope, "ai", "byok")
		return e
	})
	raw := []byte("owned-fixture-private-provider-key-329874")
	if bytes.Contains(record.StorageValue().Ciphertext, raw) {
		t.Fatal("database stored plaintext key")
	}
	dump := sqlCapture(t, "SELECT encode(ciphertext,'hex') FROM platform_model.credentials WHERE workspace_id='"+f.scope.WorkspaceID+"'; SELECT convert_from(body,'UTF8') FROM platform_model.configurations WHERE workspace_id='"+f.scope.WorkspaceID+"'")
	if bytes.Contains(dump, raw) || bytes.Contains(dump, []byte(hex.EncodeToString(raw))) {
		t.Fatal("raw provider key entered ordinary storage projection")
	}
	for _, v := range []any{config, &config, record, &record, f.models, f.vault, f.mt} {
		for _, format := range []string{"%+v", "%#v", "%s", "%d", "%x", "%*s"} {
			s := fmt.Sprintf(format, v)
			if strings.Contains(s, string(raw)) || strings.Contains(s, f.scope.GameID) {
				t.Fatal("private storage handle leaked diagnostics")
			}
		}
		if _, e := json.Marshal(v); e == nil {
			t.Fatal("private state exported to ordinary JSON")
		}
	}
}
func TestCrossWorkspaceRoomGameAndSeatDenied(t *testing.T) {
	f := newFixture(t, nil)
	f.configured(t)
	targets := []model.TargetData{{Scope: f.scope, SeatID: "human", ID: "selected"}, {Scope: core.Scope{WorkspaceID: f.scope.WorkspaceID, RoomID: f.scope.RoomID, GameID: "other-game"}, SeatID: "ai", ID: "selected"}, {Scope: core.Scope{WorkspaceID: f.scope.WorkspaceID, RoomID: "other-room", GameID: f.scope.GameID}, SeatID: "ai", ID: "selected"}, {Scope: core.Scope{WorkspaceID: "other-workspace", RoomID: f.scope.RoomID, GameID: f.scope.GameID}, SeatID: "ai", ID: "selected"}}
	for i, target := range targets {
		t.Run(fmt.Sprintf("scope-%d", i), func(t *testing.T) {
			_, e := f.models.Read(f.ctx, f.caller(f.owner, ""), auth.RoomSecret(target))
			want(t, e, auth.ErrDenied)
		})
	}
}
func (f *fixture) join(t *testing.T, kind string, manager bool) actor {
	t.Helper()
	var a actor
	if kind == "guest" {
		id := unique("guest")
		raw := token(t)
		expiry := time.Now().UTC().Truncate(time.Microsecond).Add(time.Hour)
		need(t, f.r.Transact(f.ctx, func(tx auth.Transaction) error {
			if e := tx.Core().InsertGuest(f.ctx, core.Guest{Scope: f.scope, ID: id, ExpiresAt: expiry}); e != nil {
				return e
			}
			return tx.PutSession(f.ctx, auth.StoredSession(auth.SessionData{Hash: hashed(raw), Kind: "guest", GuestID: id, Scope: f.scope, ExpiresAt: expiry, LastSeen: time.Now().UTC()}))
		}))
		a = f.context(t, actorData{ID: id, Kind: "guest", Scope: f.scope, ExpiresAt: expiry, Cookie: auth.BrowserCookie(raw)})
	} else {
		a = f.account(t)
	}
	x := a.StorageValue()
	part := unique("part")
	seat := "seat_" + x.ID
	f.inspect(t, f.owner, func(tx auth.Transaction) error {
		if x.Kind == "account" {
			role := core.Member
			if kind == "admin" {
				role = core.Admin
			}
			if e := tx.Core().PutMembership(f.ctx, core.Membership{WorkspaceID: f.scope.WorkspaceID, AccountID: x.ID, Role: role}); e != nil {
				return e
			}
		}
		rt, e := f.rt.Bind(tx.Core())
		if e != nil {
			return e
		}
		p := room.ParticipantData{Scope: f.scope, ID: part, Name: "Fixture participant", Active: true}
		if x.Kind == "account" {
			p.AccountID = x.ID
		} else {
			p.GuestID = x.ID
		}
		if e = rt.PutParticipant(f.ctx, auth.RoomSecret(p)); e != nil {
			return e
		}
		if manager {
			if e = rt.SetManager(f.ctx, f.scope, x.ID, true); e != nil {
				return e
			}
		}
		f.prep.Slots = append(f.prep.Slots, launch.Slot{ID: seat, Mode: "human", ParticipantID: part})
		lt, e := f.lt.Bind(tx.Core())
		if e != nil {
			return e
		}
		if e = lt.PutPreparation(f.ctx, auth.RoomSecret(f.prep)); e != nil {
			return e
		}
		return lt.PutAcknowledgment(f.ctx, auth.RoomSecret(f.ack))
	})
	return a
}
func TestGuestTemporaryCredentialAndNoRetainedGrant(t *testing.T) {
	f := newFixture(t, nil)
	g := f.join(t, "guest", false)
	seat := "seat_" + g.StorageValue().ID
	need(t, f.store(t, g, seat, "temporary", credential.Temporary, time.Now().UTC().Truncate(time.Microsecond).Add(20*time.Minute)))
	want(t, f.store(t, g, seat, "retained", credential.Retained, time.Time{}), auth.ErrDenied)
	want(t, f.store(t, g, "human", "other-seat", credential.Temporary, time.Now().UTC().Truncate(time.Microsecond).Add(time.Minute)), auth.ErrDenied)
	want(t, f.store(t, g, "ai", "ai-key", credential.Temporary, time.Now().UTC().Truncate(time.Microsecond).Add(time.Minute)), auth.ErrDenied)
}
func TestWorkspaceMemberCannotGainRetainedOrAISeatRights(t *testing.T) {
	f := newFixture(t, nil)
	a := f.join(t, "member", false)
	seat := "seat_" + a.StorageValue().ID
	need(t, f.store(t, a, seat, "temporary", credential.Temporary, time.Now().UTC().Truncate(time.Microsecond).Add(time.Minute)))
	want(t, f.store(t, a, seat, "longterm", credential.Retained, time.Time{}), auth.ErrDenied)
	want(t, f.store(t, a, "ai", "other", credential.Temporary, time.Now().UTC().Truncate(time.Microsecond).Add(time.Minute)), auth.ErrDenied)
}
func TestDelegatedManagerRetainedGrantIsCurrent(t *testing.T) {
	f := newFixture(t, nil)
	a := f.join(t, "admin", true)
	need(t, f.store(t, a, "ai", "admin-key", credential.Retained, time.Time{}))
	r := model.NewConfigureRequest(model.ConfigureRequestData{Scope: f.scope, SeatID: "ai", Selection: "selected", CredentialID: "admin-key", Budget: limits()})
	_, e := f.models.Configure(f.ctx, f.caller(a, token(t)), r)
	need(t, e)
	need(t, f.check(t, f.models, f.proof()))
	need(t, f.r.Transact(f.ctx, func(tx auth.Transaction) error {
		return tx.Core().DeleteMembership(f.ctx, f.scope.WorkspaceID, a.StorageValue().ID)
	}))
	want(t, f.check(t, f.models, f.proof()), auth.ErrDenied)
}
func TestGuestRevocationAndClaimInvalidateTemporaryAccess(t *testing.T) {
	for _, kind := range []string{"disabled", "claimed"} {
		t.Run(kind, func(t *testing.T) {
			f := newFixture(t, nil)
			g := f.join(t, "guest", false)
			seat := "seat_" + g.StorageValue().ID
			need(t, f.store(t, g, seat, "temporary", credential.Temporary, time.Now().UTC().Truncate(time.Microsecond).Add(time.Minute)))
			need(t, f.r.Transact(f.ctx, func(tx auth.Transaction) error {
				if kind == "disabled" {
					return tx.Core().DisableGuest(f.ctx, f.scope, g.StorageValue().ID)
				}
				return tx.Core().ClaimGuest(f.ctx, f.scope, g.StorageValue().ID, f.owner.StorageValue().ID)
			}))
			want(t, f.store(t, g, seat, "second", credential.Temporary, time.Now().UTC().Truncate(time.Microsecond).Add(time.Minute)), auth.ErrUnauthenticated)
		})
	}
}
func TestExpiredTemporaryCredentialFailsCurrentModelCheck(t *testing.T) {
	f := newFixture(t, nil)
	expiry := time.Now().UTC().Truncate(time.Microsecond).Add(250 * time.Millisecond)
	need(t, f.store(t, f.owner, "ai", "temporary", credential.Temporary, expiry))
	r := model.NewConfigureRequest(model.ConfigureRequestData{Scope: f.scope, SeatID: "ai", Selection: "selected", CredentialID: "temporary", Budget: limits()})
	_, e := f.models.Configure(f.ctx, f.caller(f.owner, token(t)), r)
	need(t, e)
	need(t, f.check(t, f.models, f.proof()))
	time.Sleep(time.Until(expiry) + 20*time.Millisecond)
	want(t, f.check(t, f.models, f.proof()), auth.ErrDenied)
}
func TestCredentialRevocationInvalidatesConfigAndReplay(t *testing.T) {
	f := newFixture(t, nil)
	f.configured(t)
	target := auth.RoomSecret(model.TargetData{Scope: f.scope, SeatID: "ai", ID: "byok"})
	caller := f.caller(f.owner, token(t))
	need(t, f.models.RevokeCredential(f.ctx, caller, target))
	need(t, f.models.RevokeCredential(f.ctx, caller, target))
	want(t, f.check(t, f.models, f.proof()), auth.ErrDenied)
}
func TestCookieCSRFAndCurrentAccountRevocation(t *testing.T) {
	f := newFixture(t, nil)
	_, r := f.configured(t)
	bad := f.caller(f.owner, token(t)).StorageValue()
	bad.CSRF = token(t)
	_, e := f.models.Configure(f.ctx, auth.RoomSecret(bad), r)
	want(t, e, auth.ErrDenied)
	bad = f.caller(f.owner, token(t)).StorageValue()
	bad.Credential = auth.BrowserCookie(token(t))
	_, e = f.models.Configure(f.ctx, auth.RoomSecret(bad), r)
	want(t, e, auth.ErrUnauthenticated)
	need(t, f.authority.Authentication().Revoke(f.ctx, f.owner.StorageValue().Cookie))
	e = f.check(t, f.models, f.proof())
	if e != auth.ErrUnauthenticated && e != auth.ErrDenied {
		t.Fatal("revoked cookie retained access")
	}
}
func TestCertificationRevocationAndUnapprovedSelection(t *testing.T) {
	f := newFixture(t, nil)
	_, r := f.configured(t)
	d := r.StorageValue()
	d.CertificationID = "unreviewed"
	d.ExpectedVersion = 1
	_, e := f.models.Configure(f.ctx, f.caller(f.owner, token(t)), model.NewConfigureRequest(d))
	want(t, e, auth.ErrDenied)
	need(t, f.models.RevokeCertification(f.scope.WorkspaceID, "default-model"))
	want(t, f.check(t, f.models, f.proof()), auth.ErrDenied)
}
func TestAllSixCertificationChangesInvalidateStoredSelection(t *testing.T) {
	for _, field := range []string{"model", "endpoint", "adapter", "prompt", "tools", "test-version"} {
		t.Run(field, func(t *testing.T) {
			f := newFixture(t, nil)
			f.configured(t)
			c := model.NewCertification(f.certs[0].StorageValue()).StorageValue()
			es := append([]model.Endpoint{}, f.endpoints...)
			switch field {
			case "model":
				c.Tuple.Model = "fixture:other"
			case "endpoint":
				c.Tuple.Endpoint = "https://approved-second.example/v1"
				e := f.endpoints[0].StorageValue()
				e.ID = "second"
				e.URL = c.Tuple.Endpoint
				es = append(es, model.NewEndpoint(e))
			case "adapter":
				c.Tuple.Adapter = "unsupported-adapter"
				e := f.endpoints[0].StorageValue()
				e.ID = "other-adapter"
				e.Adapter = c.Tuple.Adapter
				es = append(es, model.NewEndpoint(e))
			case "prompt":
				c.Tuple.PromptTemplate = "safe-v2"
			case "tools":
				c.Tuple.ToolMode = "none"
				c.Level = 1
				c.Capabilities = nil
				c.Games[0].Capabilities = nil
			case "test-version":
				c.Tuple.TestVersion = "fixture-v2"
				c.Games[0].TestVersion = "fixture-v2"
			}
			if field == "adapter" {
				_, e := model.New(model.Options{Authority: f.authority, Rooms: f.rt, Launches: f.lt, Storage: f.mt, Vault: f.vault, Endpoints: es, Certifications: []model.Certification{model.NewCertification(c)}, Defaults: map[string]string{f.scope.WorkspaceID: "default-model"}, WorkspaceLimits: map[string]model.Limits{f.scope.WorkspaceID: limits()}})
				want(t, e, auth.ErrInvalid)
				return
			}
			changed := f.service(t, es, []model.Certification{model.NewCertification(c)})
			want(t, f.check(t, changed, f.proof()), auth.ErrDenied)
		})
	}
}
func TestPreparationRevisionGraphSeatAndSafetyCannotBeReplayed(t *testing.T) {
	for _, field := range []string{"revision", "graph", "seat", "safety"} {
		t.Run(field, func(t *testing.T) {
			f := newFixture(t, nil)
			f.configured(t)
			if field == "safety" {
				f.ack.SafetyConfirmed = false
				want(t, f.check(t, f.models, f.proof()), auth.ErrDenied)
				return
			}
			f.inspect(t, f.owner, func(tx auth.Transaction) error {
				lt, e := f.lt.Bind(tx.Core())
				if e != nil {
					return e
				}
				p := f.prep
				switch field {
				case "revision":
					p.Revision++
				case "graph":
					p.GraphHash = "sha256:" + strings.Repeat("b", 64)
				case "seat":
					p.Slots = append([]launch.Slot{}, p.Slots...)
					p.Slots[1].ModelSelection = "different"
				}
				return lt.PutPreparation(f.ctx, auth.RoomSecret(p))
			})
			want(t, f.check(t, f.models, f.proof()), auth.ErrDenied)
		})
	}
}
func TestBudgetAndFallbackRequirePriorBoundedAuthorization(t *testing.T) {
	f := newFixture(t, nil)
	_, r := f.configured(t)
	for _, kind := range []string{"budget", "missing", "duplicate", "self"} {
		t.Run(kind, func(t *testing.T) {
			d := r.StorageValue()
			d.ExpectedVersion = 1
			switch kind {
			case "budget":
				d.Budget.CostMicros++
			case "missing":
				d.FallbackIDs = []string{"unapproved"}
			case "duplicate":
				d.FallbackIDs = []string{"approved-fallback", "approved-fallback"}
			case "self":
				d.FallbackIDs = []string{"default-model"}
			}
			_, e := f.models.Configure(f.ctx, f.caller(f.owner, token(t)), model.NewConfigureRequest(d))
			if e != auth.ErrDenied && e != auth.ErrInvalid {
				t.Fatal("unapproved fallback or expanded budget accepted")
			}
		})
	}
}
func TestIdempotencyCurrentReplayAndConflictingPayload(t *testing.T) {
	f := newFixture(t, nil)
	need(t, f.store(t, f.owner, "ai", "byok", credential.Retained, time.Time{}))
	r := model.NewConfigureRequest(model.ConfigureRequestData{Scope: f.scope, SeatID: "ai", Selection: "selected", CredentialID: "byok", Budget: limits()})
	caller := f.caller(f.owner, token(t))
	a, e := f.models.Configure(f.ctx, caller, r)
	need(t, e)
	b, e := f.models.Configure(f.ctx, caller, r)
	need(t, e)
	if a.StorageValue().Version != b.StorageValue().Version {
		t.Fatal("replay changed configuration version")
	}
	d := r.StorageValue()
	d.Budget.Tokens--
	_, e = f.models.Configure(f.ctx, caller, model.NewConfigureRequest(d))
	want(t, e, auth.ErrConflict)
	need(t, f.models.RevokeCertification(f.scope.WorkspaceID, "default-model"))
	_, e = f.models.Configure(f.ctx, caller, r)
	want(t, e, auth.ErrDenied)
}
func TestUnknownCommitOutcomeRecoversExactReceipt(t *testing.T) {
	var armed atomic.Bool
	f := newFixture(t, func(_ context.Context, p string) error {
		if armed.Load() && p == "after-commit-unknown" {
			return fmt.Errorf("private-fixture-error-token")
		}
		return nil
	})
	need(t, f.store(t, f.owner, "ai", "byok", credential.Retained, time.Time{}))
	r := model.NewConfigureRequest(model.ConfigureRequestData{Scope: f.scope, SeatID: "ai", Selection: "selected", CredentialID: "byok", Budget: limits()})
	caller := f.caller(f.owner, token(t))
	armed.Store(true)
	_, e := f.models.Configure(f.ctx, caller, r)
	want(t, e, auth.ErrOutcomeUnknown)
	armed.Store(false)
	config, e := f.models.Configure(f.ctx, caller, r)
	need(t, e)
	if config.StorageValue().Version != 1 {
		t.Fatal("uncertain commit duplicated configuration")
	}
}
func TestCredentialAndConfigFaultsRollbackWithCanonicalErrors(t *testing.T) {
	for _, point := range []string{"model-after-credential", "model-after-configuration"} {
		t.Run(point, func(t *testing.T) {
			var armed atomic.Bool
			f := newFixture(t, func(_ context.Context, p string) error {
				if armed.Load() && p == point {
					return fmt.Errorf("owned-fixture-private-provider-key-329874")
				}
				return nil
			})
			if point == "model-after-credential" {
				armed.Store(true)
				want(t, f.store(t, f.owner, "ai", "byok", credential.Retained, time.Time{}), auth.ErrUnavailable)
				armed.Store(false)
				need(t, f.store(t, f.owner, "ai", "byok", credential.Retained, time.Time{}))
			} else {
				need(t, f.store(t, f.owner, "ai", "byok", credential.Retained, time.Time{}))
				r := model.NewConfigureRequest(model.ConfigureRequestData{Scope: f.scope, SeatID: "ai", Selection: "selected", CredentialID: "byok", Budget: limits()})
				caller := f.caller(f.owner, token(t))
				armed.Store(true)
				_, e := f.models.Configure(f.ctx, caller, r)
				want(t, e, auth.ErrUnavailable)
				armed.Store(false)
				_, e = f.models.Configure(f.ctx, caller, r)
				need(t, e)
			}
		})
	}
}
func TestConcurrentReplayDoesNotDuplicateCredentialOrConfig(t *testing.T) {
	f := newFixture(t, nil)
	key, e := credential.NewKey([]byte("concurrent-private-fixture-key"))
	need(t, e)
	defer key.Close()
	r := auth.RoomSecret(model.CredentialRequestData{Scope: f.scope, SeatID: "ai", ID: "byok", Lifetime: credential.Retained, Key: key})
	caller := f.caller(f.owner, token(t))
	var wg sync.WaitGroup
	errs := make(chan error, 8)
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func() { defer wg.Done(); errs <- f.models.StoreCredential(f.ctx, caller, r) }()
	}
	wg.Wait()
	close(errs)
	for e := range errs {
		need(t, e)
	}
	q := model.NewConfigureRequest(model.ConfigureRequestData{Scope: f.scope, SeatID: "ai", Selection: "selected", CredentialID: "byok", Budget: limits()})
	caller = f.caller(f.owner, token(t))
	errs = make(chan error, 8)
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func() { defer wg.Done(); _, e := f.models.Configure(f.ctx, caller, q); errs <- e }()
	}
	wg.Wait()
	close(errs)
	for e := range errs {
		need(t, e)
	}
	need(t, f.check(t, f.models, f.proof()))
}
func TestConfigurationCASAndStoredBodyAliasIsolation(t *testing.T) {
	f := newFixture(t, nil)
	config, r := f.configured(t)
	d := r.StorageValue()
	d.ExpectedVersion = 1
	d.FallbackIDs = []string{"approved-fallback"}
	updated, e := f.models.Configure(f.ctx, f.caller(f.owner, token(t)), model.NewConfigureRequest(d))
	need(t, e)
	value := updated.StorageValue()
	value.Fallbacks[0].Hash = "tampered-caller-copy"
	need(t, f.check(t, f.models, f.proof()))
	d.ExpectedVersion = 0
	_, e = f.models.Configure(f.ctx, f.caller(f.owner, token(t)), model.NewConfigureRequest(d))
	want(t, e, auth.ErrConflict)
	if config.StorageValue().Version != 1 {
		t.Fatal("old receipt mutated")
	}
}
func TestTamperedStoredConfigurationBodyFailsClosed(t *testing.T) {
	f := newFixture(t, nil)
	config, _ := f.configured(t)
	c := config.StorageValue()
	c.Tuple.PromptTemplate = "unapproved"
	raw, e := json.Marshal(c)
	need(t, e)
	_ = sqlCapture(t, "UPDATE platform_model.configurations SET body=decode('"+hex.EncodeToString(raw)+"','hex') WHERE workspace_id='"+f.scope.WorkspaceID+"'")
	want(t, f.check(t, f.models, f.proof()), auth.ErrDenied)
}
func TestUnboundStorageAndCheckerRejectSpoofedTransactions(t *testing.T) {
	f := newFixture(t, nil)
	need(t, f.r.Transact(f.ctx, func(tx auth.Transaction) error {
		_, e := f.mt.Bind(tx.Core())
		want(t, e, auth.ErrDenied)
		want(t, f.models.Check(f.ctx, tx.Core(), f.proof(), []launch.Acknowledgment{auth.RoomSecret(f.ack)}), auth.ErrDenied)
		return nil
	}))
	_, e := f.models.Read(f.ctx, model.Caller{}, auth.RoomSecret(model.TargetData{Scope: f.scope, SeatID: "ai", ID: "selected"}))
	if e != auth.ErrUnauthenticated && e != auth.ErrDenied {
		t.Fatal("spoofed cookie gained scoped configuration")
	}
}

func TestTemporaryCredentialTimeZonesSurvivePostgresRoundtrip(t *testing.T) {
	for _, tc := range []struct {
		name   string
		offset int
	}{{"UTC", 0}, {"positive-offset", 19800}, {"negative-offset", -25200}} {
		t.Run(tc.name, func(t *testing.T) {
			f := newFixture(t, nil)
			expiry := time.Now().UTC().Truncate(time.Microsecond).Add(20 * time.Minute).In(time.FixedZone(tc.name, tc.offset))
			need(t, f.store(t, f.owner, "ai", "temporary", credential.Temporary, expiry))
			var record credential.Record
			f.inspect(t, f.owner, func(tx auth.Transaction) error {
				mt, e := f.mt.Bind(tx.Core())
				if e != nil {
					return e
				}
				record, e = mt.Credential(f.ctx, f.scope, "ai", "temporary")
				return e
			})
			b := record.StorageValue().Binding
			if !b.ExpiresAt.Equal(expiry) || b.ExpiresAt.Location() != time.UTC {
				t.Fatal("PostgreSQL changed expiry instant or failed UTC normalization")
			}
			opened, e := f.vault.Open(f.ctx, record, b, time.Now())
			need(t, e)
			need(t, opened.Use(func(raw []byte) error {
				if !bytes.Equal(raw, []byte("owned-fixture-private-provider-key-329874")) {
					t.Fatal("PostgreSQL roundtrip changed provider credential")
				}
				return nil
			}))
			opened.Close()
			r := model.NewConfigureRequest(model.ConfigureRequestData{Scope: f.scope, SeatID: "ai", Selection: "selected", CredentialID: "temporary", Budget: limits()})
			_, e = f.models.Configure(f.ctx, f.caller(f.owner, token(t)), r)
			need(t, e)
			need(t, f.check(t, f.models, f.proof()))
			if _, e := f.vault.Open(f.ctx, record, b, b.ExpiresAt); e != auth.ErrDenied {
				t.Fatal("persisted temporary credential remained valid at expiry")
			}
		})
	}
}
