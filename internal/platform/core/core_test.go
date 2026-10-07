// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package core

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"testing"
)

func TestWorkspacePermissionMatrix(t *testing.T) {
	for _, role := range []Role{Owner, Admin, Member, "", "host", "guest", "OWNER"} {
		for p := Permission(0); p <= ReadPrivateView+1; p++ {
			want := (role == Owner || role == Admin || role == Member) && p == ReadWorkspace || (role == Owner || role == Admin) && (p == ManageWorkspace || p == OwnCampaign || p == UploadPackage || p == RetainCredential)
			t.Run(fmt.Sprintf("%s/%d", role, p), func(t *testing.T) {
				if got := workspaceAllowed(role, p); got != want {
					t.Fatal("workspace permission matrix differs")
				}
			})
		}
	}
}

func TestIdentifierAndLabelBoundaries(t *testing.T) {
	for _, id := range []string{"", "../workspace", "two spaces", "-first", strings.Repeat("a", 129), "x\n", "租户"} {
		if validID(id) {
			t.Fatal("invalid identifier accepted")
		}
	}
	for _, id := range []string{"w", "Tenant_1.a-b", strings.Repeat("a", 128)} {
		if !validID(id) {
			t.Fatal("valid identifier denied")
		}
	}
	for _, name := range []string{"", " \t", "line\n", string([]byte{0xff}), strings.Repeat("名", 129)} {
		if validLabel(name) {
			t.Fatal("invalid label accepted")
		}
	}
	if !validLabel(strings.Repeat("名", 128)) {
		t.Fatal("Unicode label limit counted as bytes")
	}
	if validScope(Scope{WorkspaceID: "w", RoomID: "r"}) || validScope(Scope{RoomID: "r", GameID: "g"}) {
		t.Fatal("incomplete guest scope accepted")
	}
}

type probeRepository struct {
	calls int
	err   error
}

func (r *probeRepository) Transact(context.Context, func(Transaction) error) error {
	r.calls++
	return r.err
}

func TestInvalidAndForeignActorsNeverReachStorage(t *testing.T) {
	r := &probeRepository{}
	s, _ := NewService(r)
	other, _ := NewService(r)
	foreign := other.issue("account", Scope{}, false)
	for _, actor := range []Actor{{}, foreign} {
		if s.CreateWorkspace(context.Background(), actor, "workspace", "Workspace") != ErrDenied {
			t.Fatal("unverified actor created workspace")
		}
		if _, err := s.Workspace(context.Background(), actor, "workspace"); err != ErrDenied {
			t.Fatal("unverified actor read workspace")
		}
		if s.Authorize(context.Background(), actor, Scope{WorkspaceID: "workspace"}, ReadWorkspace) != ErrDenied {
			t.Fatal("unverified actor obtained permission")
		}
	}
	if r.calls != 0 {
		t.Fatal("invalid actor reached repository")
	}
}

func TestGuestLongTermOperationsNeverReachStorage(t *testing.T) {
	r := &probeRepository{}
	s, _ := NewService(r)
	scope := Scope{WorkspaceID: "workspace", RoomID: "room", GameID: "game"}
	guest := s.issue("guest", scope, true)
	for _, permission := range []Permission{ReadWorkspace, ManageWorkspace, OwnCampaign, UploadPackage, RetainCredential, ControlSeat, ReadPrivateView} {
		if s.Authorize(context.Background(), guest, scope, permission) != ErrDenied {
			t.Fatal("guest obtained long-term or seat permission")
		}
	}
	if s.CreateWorkspace(context.Background(), guest, "workspace", "Workspace") != ErrDenied || s.SetMembership(context.Background(), guest, "workspace", "account", Admin) != ErrDenied {
		t.Fatal("guest reached management operation")
	}
	if r.calls != 0 {
		t.Fatal("guest long-term operation reached repository")
	}
}

func TestErrorsAreBoundedAndUnwrapped(t *testing.T) {
	private := errors.New("synthetic-private-database-detail")
	r := &probeRepository{err: private}
	s, _ := NewService(r)
	if err := s.RegisterAccount(context.Background(), "account", "Name"); err != ErrUnavailable || errors.Is(err, private) {
		t.Fatal("storage diagnostic crossed application boundary")
	}
	for _, err := range []error{ErrDenied, ErrConflict, ErrUnavailable, ErrOutcomeUnknown} {
		if SafeError(err) != err {
			t.Fatal("bounded error identity lost")
		}
	}
	cancelled, cancel := context.WithCancelCause(context.Background())
	cancel(private)
	if err := s.RegisterAccount(cancelled, "account", "Name"); err != ErrUnavailable || r.calls != 1 {
		t.Fatal("cancelled context reached storage or exported cause")
	}
}

func TestOpaqueHandlesHideIdentityInFormattingAndEncoding(t *testing.T) {
	r := &probeRepository{err: errors.New("synthetic-private-repository-detail")}
	s, _ := NewService(r)
	actor := s.issue("synthetic-private-account", Scope{WorkspaceID: "synthetic-private-workspace"}, true)
	for _, value := range []any{actor, &actor, []Actor{actor}, struct{ A Actor }{actor}, s, *s, []Service{*s}} {
		for _, format := range []string{"%v", "%+v", "%#v", "%s", "%d", "%x", "%q", "%f"} {
			if strings.Contains(fmt.Sprintf(format, value), "synthetic-private") {
				t.Fatal("opaque handle formatting exposed private data")
			}
		}
	}
	if _, err := json.Marshal(actor); err == nil {
		t.Fatal("identity handle was serialized")
	}
}
