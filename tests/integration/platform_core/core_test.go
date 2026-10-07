//go:build integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package platform_core_test

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"errors"
	"fmt"
	"net/url"
	"os"
	"os/exec"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

var dsn, runID string
var sequence atomic.Uint64

func TestMain(m *testing.M) {
	dsn = os.Getenv("M2B001_POSTGRES_DSN")
	runID = os.Getenv("M2B001_RUN_ID")
	u, err := url.Parse(dsn)
	if err != nil || u.Scheme != "postgres" || u.Hostname() != "127.0.0.1" || u.Path != "/m2_b001_fixture" || runID == "" || os.Getenv("M2B001_OWNED_DATABASE") != "1" {
		fmt.Fprintln(os.Stderr, "M2-B001 owned local database required; integration NOT_RUN")
		os.Exit(1)
	}
	// A container lease may serve consecutive or independent verification runs.
	// Keep their persisted fixtures disjoint without truncating previous evidence.
	var nonce [16]byte
	if _, err := rand.Read(nonce[:]); err != nil {
		fmt.Fprintln(os.Stderr, "M2-B001 test isolation unavailable; integration NOT_RUN")
		os.Exit(1)
	}
	runID += "-" + hex.EncodeToString(nonce[:])
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	r, err := postgres.OpenPlatformCoreRepository(ctx, dsn, nil)
	if err == nil {
		err = r.Bootstrap(ctx)
		_ = r.Close()
	}
	cancel()
	if err != nil {
		fmt.Fprintln(os.Stderr, "M2-B001 database bootstrap failed; integration NOT_RUN")
		os.Exit(1)
	}
	os.Exit(m.Run())
}

type fixture struct {
	r                                                           *postgres.PlatformCoreRepository
	s                                                           *core.Service
	owner, admin, member, out                                   core.Actor
	ownerID, adminID, memberID, outID, workspace, other, prefix string
	ctx                                                         context.Context
}

func need(t *testing.T, err error) {
	t.Helper()
	if err != nil {
		t.Fatalf("operation failed: %v", core.SafeError(err))
	}
}

func deny(t *testing.T, err error) {
	t.Helper()
	if err != core.ErrDenied {
		t.Fatalf("expected denial, received bounded result: %v", core.SafeError(err))
	}
}

func newFixture(t *testing.T, fault func(context.Context, string) error) *fixture {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	t.Cleanup(cancel)
	r, err := postgres.OpenPlatformCoreRepository(ctx, dsn, fault)
	need(t, err)
	t.Cleanup(func() { need(t, r.Close()) })
	s, err := core.NewService(r)
	need(t, err)
	prefix := fmt.Sprintf("%s-%d", runID, sequence.Add(1))
	f := &fixture{r: r, s: s, ctx: ctx, prefix: prefix, workspace: prefix + "-w", other: prefix + "-other", ownerID: prefix + "-owner", adminID: prefix + "-admin", memberID: prefix + "-member", outID: prefix + "-out"}
	for _, id := range []string{f.ownerID, f.adminID, f.memberID, f.outID} {
		need(t, s.RegisterAccount(ctx, id, "Synthetic account"))
	}
	f.owner, err = s.AccountActor(ctx, f.ownerID)
	need(t, err)
	f.admin, err = s.AccountActor(ctx, f.adminID)
	need(t, err)
	f.member, err = s.AccountActor(ctx, f.memberID)
	need(t, err)
	f.out, err = s.AccountActor(ctx, f.outID)
	need(t, err)
	need(t, s.CreateWorkspace(ctx, f.owner, f.workspace, "Same workspace name"))
	need(t, s.CreateWorkspace(ctx, f.out, f.other, "Same workspace name"))
	need(t, s.SetMembership(ctx, f.owner, f.workspace, f.adminID, core.Admin))
	need(t, s.SetMembership(ctx, f.owner, f.workspace, f.memberID, core.Member))
	return f
}

func (f *fixture) scope() core.Scope {
	return core.Scope{WorkspaceID: f.workspace, RoomID: "room", GameID: "game"}
}

func (f *fixture) guest(t *testing.T, id string, expiry time.Time) core.Actor {
	t.Helper()
	need(t, f.s.ProvisionGuest(f.ctx, f.owner, f.scope(), id, expiry))
	a, err := f.s.GuestActor(f.ctx, f.scope(), id)
	need(t, err)
	return a
}

func TestOwnedDatabaseIsMandatory(t *testing.T) {
	binary, err := os.Executable()
	need(t, err)
	cmd := exec.Command(binary, "-test.run=^$")
	for _, variable := range os.Environ() {
		if !strings.HasPrefix(variable, "M2B001_POSTGRES_DSN=") {
			cmd.Env = append(cmd.Env, variable)
		}
	}
	out, err := cmd.CombinedOutput()
	var exit *exec.ExitError
	if !errors.As(err, &exit) || exit.ExitCode() != 1 || !strings.Contains(string(out), "integration NOT_RUN") {
		t.Fatal("missing database configuration did not fail closed")
	}
}

func TestAccountsAndAtomicWorkspacePersistAfterReopen(t *testing.T) {
	f := newFixture(t, nil)
	if err := f.s.RegisterAccount(f.ctx, f.ownerID, "Duplicate"); err != core.ErrConflict {
		t.Fatal("duplicate account accepted")
	}
	need(t, f.r.Bootstrap(f.ctx))
	r, err := postgres.OpenPlatformCoreRepository(f.ctx, dsn, nil)
	need(t, err)
	defer r.Close()
	s, err := core.NewService(r)
	need(t, err)
	a, err := s.AccountActor(f.ctx, f.ownerID)
	need(t, err)
	w, err := s.Workspace(f.ctx, a, f.workspace)
	need(t, err)
	if w.ID != f.workspace || w.OwnerID != f.ownerID || w.Name != "Same workspace name" {
		t.Fatal("workspace persistence differs")
	}
	need(t, r.Transact(f.ctx, func(tx core.Transaction) error {
		m, err := tx.Membership(f.ctx, f.workspace, f.ownerID)
		if err != nil {
			return err
		}
		if m.Role != core.Owner {
			t.Fatal("owner membership missing")
		}
		return nil
	}))
}

func TestWorkspaceCreationRollsBackAtEveryWriteBoundary(t *testing.T) {
	for _, point := range []string{"after-workspace", "after-membership", "before-commit"} {
		t.Run(point, func(t *testing.T) {
			var enabled atomic.Bool
			f := newFixture(t, func(_ context.Context, got string) error {
				if enabled.Load() && got == point {
					return errors.New("synthetic-private-rollback-detail")
				}
				return nil
			})
			id := f.prefix + "-rollback"
			enabled.Store(true)
			if err := f.s.CreateWorkspace(f.ctx, f.owner, id, "Rollback"); err != core.ErrUnavailable {
				t.Fatal("fault not propagated safely")
			}
			enabled.Store(false)
			_, err := f.s.Workspace(f.ctx, f.owner, id)
			deny(t, err)
			need(t, f.s.CreateWorkspace(f.ctx, f.owner, id, "Rollback"))
			_, err = f.s.Workspace(f.ctx, f.owner, id)
			need(t, err)
		})
	}
}

func TestRelationalConstraintsRejectOrphansAndExtraOwners(t *testing.T) {
	f := newFixture(t, nil)
	cases := []struct {
		name  string
		write func(core.Transaction) error
	}{
		{"workspace-without-owner-membership", func(tx core.Transaction) error {
			return tx.InsertWorkspace(f.ctx, core.Workspace{ID: f.prefix + "-orphan", Name: "Orphan", OwnerID: f.ownerID})
		}},
		{"owner-demotion", func(tx core.Transaction) error {
			return tx.PutMembership(f.ctx, core.Membership{WorkspaceID: f.workspace, AccountID: f.ownerID, Role: core.Member})
		}},
		{"owner-removal", func(tx core.Transaction) error { return tx.DeleteMembership(f.ctx, f.workspace, f.ownerID) }},
		{"extra-owner", func(tx core.Transaction) error {
			return tx.PutMembership(f.ctx, core.Membership{WorkspaceID: f.workspace, AccountID: f.memberID, Role: core.Owner})
		}},
		{"unknown-account", func(tx core.Transaction) error {
			return tx.PutMembership(f.ctx, core.Membership{WorkspaceID: f.workspace, AccountID: f.prefix + "-unknown", Role: core.Member})
		}},
		{"unknown-workspace", func(tx core.Transaction) error {
			return tx.PutMembership(f.ctx, core.Membership{WorkspaceID: f.prefix + "-unknown", AccountID: f.memberID, Role: core.Member})
		}},
		{"invalid-role", func(tx core.Transaction) error {
			return tx.PutMembership(f.ctx, core.Membership{WorkspaceID: f.workspace, AccountID: f.memberID, Role: "host"})
		}},
		{"invalid-account-id", func(tx core.Transaction) error {
			return tx.InsertAccount(f.ctx, core.Account{ID: "../invalid", DisplayName: "Invalid"})
		}},
		{"unknown-guest-workspace", func(tx core.Transaction) error {
			return tx.InsertGuest(f.ctx, core.Guest{Scope: core.Scope{WorkspaceID: f.prefix + "-missing", RoomID: "room", GameID: "game"}, ID: "guest", ExpiresAt: time.Now().Add(time.Hour)})
		}},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			if err := f.r.Transact(f.ctx, c.write); err != core.ErrConflict {
				t.Fatal("database relation constraint not enforced")
			}
		})
	}
	need(t, f.s.Authorize(f.ctx, f.owner, f.scope(), core.ManageWorkspace))
	_, err := f.s.Workspace(f.ctx, f.owner, f.prefix+"-orphan")
	deny(t, err)
	need(t, f.s.CreateWorkspace(f.ctx, f.owner, f.prefix+"-orphan", "Recovered"))
}

func TestTenantReadsAndMutationsNeverInferAccessFromMatchingNames(t *testing.T) {
	f := newFixture(t, nil)
	_, err := f.s.Workspace(f.ctx, f.owner, f.other)
	deny(t, err)
	_, err = f.s.Workspace(f.ctx, f.out, f.workspace)
	deny(t, err)
	deny(t, f.s.SetMembership(f.ctx, f.owner, f.other, f.memberID, core.Admin))
	deny(t, f.s.RemoveMembership(f.ctx, f.owner, f.other, f.outID))
	deny(t, f.s.Authorize(f.ctx, f.owner, core.Scope{WorkspaceID: f.other}, core.ManageWorkspace))
	_, err = f.s.AccountActor(f.ctx, f.prefix+"-unknown")
	deny(t, err)
	_, err = f.s.Workspace(f.ctx, core.Actor{}, f.workspace)
	deny(t, err)
	deny(t, f.s.CreateWorkspace(f.ctx, core.Actor{}, f.prefix+"-unverified", "Unverified"))
	otherService, err := core.NewService(f.r)
	need(t, err)
	foreign, err := otherService.AccountActor(f.ctx, f.ownerID)
	need(t, err)
	_, err = f.s.Workspace(f.ctx, foreign, f.workspace)
	deny(t, err)
}

func TestRolesCannotGrantSeatsPrivateViewsOrPromoteOwners(t *testing.T) {
	f := newFixture(t, nil)
	for _, actor := range []core.Actor{f.owner, f.admin, f.member} {
		need(t, f.s.Authorize(f.ctx, actor, f.scope(), core.ReadWorkspace))
		for _, p := range []core.Permission{core.Participate, core.ControlSeat, core.ReadPrivateView, 0, 255} {
			deny(t, f.s.Authorize(f.ctx, actor, f.scope(), p))
		}
	}
	deny(t, f.s.Authorize(f.ctx, f.member, f.scope(), core.ManageWorkspace))
	deny(t, f.s.SetMembership(f.ctx, f.member, f.workspace, f.outID, core.Member))
	deny(t, f.s.SetMembership(f.ctx, f.admin, f.workspace, f.outID, core.Admin))
	deny(t, f.s.SetMembership(f.ctx, f.admin, f.workspace, f.adminID, core.Member))
	deny(t, f.s.SetMembership(f.ctx, f.owner, f.workspace, f.memberID, core.Owner))
	deny(t, f.s.RemoveMembership(f.ctx, f.owner, f.workspace, f.ownerID))
	need(t, f.s.SetMembership(f.ctx, f.admin, f.workspace, f.outID, core.Member))
	need(t, f.s.Authorize(f.ctx, f.out, f.scope(), core.ReadWorkspace))
	deny(t, f.s.Authorize(f.ctx, f.out, f.scope(), core.ManageWorkspace))
}

func TestDisabledAccountsAndRemovedMembershipInvalidateExistingHandles(t *testing.T) {
	f := newFixture(t, nil)
	need(t, f.s.RemoveMembership(f.ctx, f.owner, f.workspace, f.adminID))
	_, err := f.s.Workspace(f.ctx, f.admin, f.workspace)
	deny(t, err)
	deny(t, f.s.SetMembership(f.ctx, f.admin, f.workspace, f.outID, core.Member))
	need(t, f.s.DisableAccount(f.ctx, f.memberID))
	_, err = f.s.Workspace(f.ctx, f.member, f.workspace)
	deny(t, err)
	_, err = f.s.AccountActor(f.ctx, f.memberID)
	deny(t, err)
	deny(t, f.s.CreateWorkspace(f.ctx, f.member, f.prefix+"-disabled", "Disabled"))
	deny(t, f.s.SetMembership(f.ctx, f.owner, f.workspace, f.memberID, core.Admin))
	need(t, f.s.RemoveMembership(f.ctx, f.owner, f.workspace, f.memberID))
}

func TestGuestsAreRestrictedToExactGameScope(t *testing.T) {
	f := newFixture(t, nil)
	g := f.guest(t, "guest", time.Now().Add(time.Hour))
	need(t, f.s.Authorize(f.ctx, g, f.scope(), core.Participate))
	for _, scope := range []core.Scope{{WorkspaceID: f.other, RoomID: "room", GameID: "game"}, {WorkspaceID: f.workspace, RoomID: "other", GameID: "game"}, {WorkspaceID: f.workspace, RoomID: "room", GameID: "other"}} {
		deny(t, f.s.Authorize(f.ctx, g, scope, core.Participate))
		_, err := f.s.GuestActor(f.ctx, scope, "guest")
		deny(t, err)
	}
	for _, p := range []core.Permission{core.ReadWorkspace, core.ManageWorkspace, core.OwnCampaign, core.UploadPackage, core.RetainCredential, core.ControlSeat, core.ReadPrivateView} {
		deny(t, f.s.Authorize(f.ctx, g, f.scope(), p))
	}
	deny(t, f.s.CreateWorkspace(f.ctx, g, f.prefix+"-guest-owned", "Forbidden"))
	deny(t, f.s.SetMembership(f.ctx, g, f.workspace, f.outID, core.Admin))
	_, err := f.s.Workspace(f.ctx, g, f.workspace)
	deny(t, err)
}

func TestExpiredAndRevokedGuestsFailClosed(t *testing.T) {
	f := newFixture(t, nil)
	deny(t, f.s.ProvisionGuest(f.ctx, f.owner, f.scope(), "already-expired", time.Now().Add(-time.Second)))
	expiry := time.Now().Add(150 * time.Millisecond)
	g := f.guest(t, "expires", expiry)
	for time.Now().Before(expiry.Add(20 * time.Millisecond)) {
		time.Sleep(10 * time.Millisecond)
	}
	deny(t, f.s.Authorize(f.ctx, g, f.scope(), core.Participate))
	_, err := f.s.GuestActor(f.ctx, f.scope(), "expires")
	deny(t, err)
	_, err = f.s.ClaimGuest(f.ctx, g, f.out)
	deny(t, err)
	g = f.guest(t, "revoked", time.Now().Add(time.Hour))
	need(t, f.s.RevokeGuest(f.ctx, f.owner, f.scope(), "revoked"))
	deny(t, f.s.Authorize(f.ctx, g, f.scope(), core.Participate))
	_, err = f.s.ClaimGuest(f.ctx, g, f.out)
	deny(t, err)
}

func TestClaimPreservesParticipationWithoutMembershipOrManagement(t *testing.T) {
	f := newFixture(t, nil)
	g := f.guest(t, "claim", time.Now().Add(time.Hour))
	p, err := f.s.ClaimGuest(f.ctx, g, f.out)
	need(t, err)
	if p.GuestID != "claim" || p.AccountID != f.outID || p.Scope != f.scope() {
		t.Fatal("claim changed participation identity or boundary")
	}
	need(t, f.s.Authorize(f.ctx, f.out, f.scope(), core.Participate))
	deny(t, f.s.Authorize(f.ctx, g, f.scope(), core.Participate))
	for _, p := range []core.Permission{core.ReadWorkspace, core.ManageWorkspace, core.OwnCampaign, core.UploadPackage, core.RetainCredential, core.ControlSeat, core.ReadPrivateView} {
		deny(t, f.s.Authorize(f.ctx, f.out, f.scope(), p))
	}
	deny(t, f.s.Authorize(f.ctx, f.out, core.Scope{WorkspaceID: f.workspace, RoomID: "room", GameID: "different"}, core.Participate))
	_, err = f.s.Workspace(f.ctx, f.out, f.workspace)
	deny(t, err)
	need(t, f.s.RevokeGuest(f.ctx, f.owner, f.scope(), "claim"))
	deny(t, f.s.Authorize(f.ctx, f.out, f.scope(), core.Participate))
}

func TestClaimRollbackDoesNotRevokeGuestOrGrantAccount(t *testing.T) {
	var enabled atomic.Bool
	f := newFixture(t, func(_ context.Context, point string) error {
		if enabled.Load() && point == "after-claim" {
			return errors.New("synthetic-private-claim-detail")
		}
		return nil
	})
	g := f.guest(t, "claim", time.Now().Add(time.Hour))
	enabled.Store(true)
	p, err := f.s.ClaimGuest(f.ctx, g, f.out)
	if err != core.ErrUnavailable || p != (core.Participation{}) {
		t.Fatal("failed claim returned participation")
	}
	enabled.Store(false)
	need(t, f.s.Authorize(f.ctx, g, f.scope(), core.Participate))
	deny(t, f.s.Authorize(f.ctx, f.out, f.scope(), core.Participate))
	_, err = f.s.ClaimGuest(f.ctx, g, f.out)
	need(t, err)
}

func TestConcurrentClaimHasOneWinner(t *testing.T) {
	f := newFixture(t, nil)
	g := f.guest(t, "race", time.Now().Add(time.Hour))
	start := make(chan struct{})
	results := make(chan error, 2)
	for _, account := range []core.Actor{f.out, f.member} {
		go func(a core.Actor) { <-start; _, err := f.s.ClaimGuest(f.ctx, g, a); results <- err }(account)
	}
	close(start)
	success, denied := 0, 0
	for range 2 {
		err := <-results
		if err == nil {
			success++
		} else if err == core.ErrDenied {
			denied++
		} else {
			t.Fatal("claim race returned unexpected result")
		}
	}
	if success != 1 || denied != 1 {
		t.Fatal("concurrent guest claim did not have exactly one winner")
	}
	deny(t, f.s.Authorize(f.ctx, g, f.scope(), core.Participate))
}

func TestMembershipRevocationSerializesWithAuthorizedMutation(t *testing.T) {
	var enabled atomic.Bool
	entered, release := make(chan struct{}), make(chan struct{})
	f := newFixture(t, func(ctx context.Context, point string) error {
		if enabled.Load() && point == "after-membership" {
			close(entered)
			select {
			case <-release:
				return nil
			case <-ctx.Done():
				return core.ErrUnavailable
			}
		}
		return nil
	})
	enabled.Store(true)
	write := make(chan error, 1)
	go func() { write <- f.s.SetMembership(f.ctx, f.admin, f.workspace, f.outID, core.Member) }()
	select {
	case <-entered:
	case <-time.After(3 * time.Second):
		t.Fatal("write did not reach locked boundary")
	}
	revoke := make(chan error, 1)
	go func() { revoke <- f.s.RemoveMembership(f.ctx, f.owner, f.workspace, f.adminID) }()
	select {
	case <-revoke:
		t.Fatal("revocation crossed an uncommitted workspace mutation")
	case <-time.After(60 * time.Millisecond):
	}
	enabled.Store(false)
	close(release)
	need(t, <-write)
	need(t, <-revoke)
	deny(t, f.s.SetMembership(f.ctx, f.admin, f.workspace, f.memberID, core.Admin))
	_, err := f.s.Workspace(f.ctx, f.admin, f.workspace)
	deny(t, err)
	need(t, f.s.Authorize(f.ctx, f.out, f.scope(), core.ReadWorkspace))
}

func TestDuplicateAndInvalidGuestRelationsAreRejected(t *testing.T) {
	f := newFixture(t, nil)
	g := f.guest(t, "guest", time.Now().Add(time.Hour))
	if err := f.s.ProvisionGuest(f.ctx, f.owner, f.scope(), "guest", time.Now().Add(time.Hour)); err != core.ErrConflict {
		t.Fatal("duplicate guest relation accepted")
	}
	deny(t, f.s.ProvisionGuest(f.ctx, f.member, f.scope(), "unmanaged", time.Now().Add(time.Hour)))
	deny(t, f.s.ProvisionGuest(f.ctx, f.owner, core.Scope{WorkspaceID: f.workspace, RoomID: "room"}, "incomplete", time.Now().Add(time.Hour)))
	_, err := f.s.ClaimGuest(f.ctx, g, core.Actor{})
	deny(t, err)
	_, err = f.s.ClaimGuest(f.ctx, f.owner, f.out)
	deny(t, err)
	_, err = f.s.ClaimGuest(f.ctx, g, g)
	deny(t, err)
	err = f.r.Transact(f.ctx, func(tx core.Transaction) error {
		err := tx.ClaimGuest(f.ctx, f.scope(), "guest", f.prefix+"-unknown")
		if err != core.ErrConflict {
			t.Fatal("guest claim accepted an unknown account")
		}
		return err
	})
	if err != core.ErrConflict {
		t.Fatal("unknown-account claim did not roll back")
	}
}

func TestStorageErrorsAndOpaqueFormattingDiscloseNoConnectionDetails(t *testing.T) {
	f := newFixture(t, nil)
	_, err := postgres.OpenPlatformCoreRepository(f.ctx, "postgres://synthetic-private-user:synthetic-private-password@127.0.0.1:1/m2_b001_fixture?connect_timeout=1", nil)
	if err != core.ErrUnavailable {
		t.Fatal("connection error was not sanitized")
	}
	if err := f.r.Transact(f.ctx, func(core.Transaction) error { return errors.New("synthetic-private-callback-detail") }); err != core.ErrUnavailable {
		t.Fatal("callback error was not sanitized")
	}
	for _, v := range []any{f.r, *f.r, []postgres.PlatformCoreRepository{*f.r}, f.s, *f.s, f.owner, &f.owner, []core.Actor{f.owner}} {
		for _, format := range []string{"%v", "%+v", "%#v", "%s", "%d", "%x", "%q", "%f"} {
			out := fmt.Sprintf(format, v)
			if strings.Contains(out, dsn) || strings.Contains(out, f.ownerID) || strings.Contains(out, "synthetic-private") || strings.Contains(out, "postgres://") {
				t.Fatal("opaque formatting exposed connection or actor data")
			}
		}
	}
	need(t, f.r.Close())
	_, err = f.s.Workspace(f.ctx, f.owner, f.workspace)
	if err != core.ErrUnavailable {
		t.Fatal("closed database error was not sanitized")
	}
}
