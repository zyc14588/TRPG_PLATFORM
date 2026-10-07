//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package launch_test

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"sync/atomic"
	"testing"
)

func TestLaunchHumanCommitsOneBindingAndActualActorRecovery(t *testing.T) {
	l := newLaunchFixture(t, nil)
	r, part := l.hostRoom(t)
	p := l.configure(t, r, part, "minimal")
	l.ready(t, r, p)
	s, e := l.start(t, l.owner, r, p.StorageValue().Revision, "launch-one")
	need(t, e)
	v := s.StorageValue()
	if v.Scope.RoomID != r || v.Binding.Workspace != l.w || v.Binding.GraphHash != p.StorageValue().GraphHash || v.Binding.Session == "game" {
		t.Fatal("native room not bound to its own authoritative session")
	}
	for _, table := range []string{"sessions", "installed_graphs", "session_locks", "creation"} {
		if l.sql(t, `SELECT count(*) FROM host_command.`+table+` WHERE workspace='`+l.w+`'`) != "1" {
			t.Fatal("authoritative creation not atomic and singular")
		}
	}
	if l.sql(t, `SELECT count(*) FROM platform_launch.sessions WHERE workspace_id='`+l.w+`'`) != "1" || l.sql(t, `SELECT state FROM platform_room.rooms WHERE workspace_id='`+l.w+`' AND room_id='`+r+`'`) != "launched" {
		t.Fatal("native launch mapping missing")
	}
	l.mu.Lock()
	ready := 0
	destroyed := 0
	for _, x := range l.executions {
		if x.Case == "session-vm-ready" {
			ready++
		}
		if x.Case == "session-vm-destroy" {
			destroyed++
		}
	}
	l.mu.Unlock()
	if ready < 2 || destroyed < 1 {
		t.Fatal("real creation and installed Actor recovery not observed")
	}
	for _, key := range []string{"launch-one", "launch-new-key"} {
		again, e := l.start(t, l.owner, r, p.StorageValue().Revision, key)
		need(t, e)
		if again.StorageValue().Binding != v.Binding {
			t.Fatal("idempotent launch changed session binding")
		}
	}
	if l.sql(t, `SELECT count(*) FROM host_command.sessions WHERE workspace='`+l.w+`'`) != "1" {
		t.Fatal("repeated launch created a second session")
	}
}
func TestTwoRoomsSameGameHaveIndependentSessions(t *testing.T) {
	l := newLaunchFixture(t, nil)
	r, p := l.hostRoom(t)
	prep := l.configure(t, r, p, "minimal")
	l.ready(t, r, prep)
	first, e := l.start(t, l.owner, r, prep.StorageValue().Revision, "first-room-launch")
	need(t, e)
	r2, p2 := l.hostRoom(t)
	prep2 := l.configure(t, r2, p2, "minimal")
	l.ready(t, r2, prep2)
	second, e := l.start(t, l.owner, r2, prep2.StorageValue().Revision, "second-room-launch")
	need(t, e)
	if first.StorageValue().Scope.GameID != second.StorageValue().Scope.GameID || first.StorageValue().Binding.Session == second.StorageValue().Binding.Session {
		t.Fatal("room identity collapsed into a shared game identifier")
	}
}
func TestLaunchRejectsEveryMissingConsentGateWithoutPartialSession(t *testing.T) {
	for _, tc := range []struct {
		name                   string
		consent, ready, safety bool
	}{{"consent", false, true, true}, {"ready", true, false, true}, {"safety", true, true, false}} {
		t.Run(tc.name, func(t *testing.T) {
			l := newLaunchFixture(t, nil)
			r, p := l.hostRoom(t)
			prep := l.configure(t, r, p, "minimal")
			need(t, l.acknowledge(t, l.owner, r, prep.StorageValue().Revision, tc.consent, tc.ready, tc.safety, nil, "incomplete-ready"))
			_, e := l.start(t, l.owner, r, prep.StorageValue().Revision, "missing-gate-launch")
			want(t, e, auth.ErrDenied)
			l.inspectAbsent(t, r)
		})
	}
}
func TestLaunchRejectsPrivateContentBoundaryAndUnavailableModel(t *testing.T) {
	for _, id := range []string{"labeled", "ai-required"} {
		t.Run(id, func(t *testing.T) {
			l := newLaunchFixture(t, nil)
			r, p := l.hostRoom(t)
			prep := l.configure(t, r, p, id)
			boundaries := []string(nil)
			if id == "labeled" {
				boundaries = []string{"violence"}
			}
			need(t, l.acknowledge(t, l.owner, r, prep.StorageValue().Revision, true, true, true, boundaries, "safety-ready"))
			_, e := l.start(t, l.owner, r, prep.StorageValue().Revision, "denied-launch")
			want(t, e, auth.ErrDenied)
			l.inspectAbsent(t, r)
		})
	}
}
func TestPreparationRevisionInvalidatesAcknowledgmentsAndOldReceipts(t *testing.T) {
	l := newLaunchFixture(t, nil)
	r, p := l.hostRoom(t)
	first := l.configure(t, r, p, "minimal")
	need(t, l.acknowledge(t, l.owner, r, first.StorageValue().Revision, true, true, true, nil, "old-ready-key"))
	second := l.configure(t, r, p, "minimal")
	if second.StorageValue().Revision != first.StorageValue().Revision+1 {
		t.Fatal("preparation revision did not advance")
	}
	_, e := l.start(t, l.owner, r, first.StorageValue().Revision, "old-version-launch")
	want(t, e, auth.ErrConflict)
	_, e = l.start(t, l.owner, r, second.StorageValue().Revision, "new-version-unready-launch")
	want(t, e, auth.ErrDenied)
	want(t, l.acknowledge(t, l.owner, r, first.StorageValue().Revision, true, true, true, nil, "old-ready-key"), auth.ErrConflict)
	l.inspectAbsent(t, r)
}
func TestWithdrawalCannotBeRestoredByOldAcknowledgmentReceipt(t *testing.T) {
	l := newLaunchFixture(t, nil)
	r, p := l.hostRoom(t)
	prep := l.configure(t, r, p, "minimal")
	revision := prep.StorageValue().Revision
	need(t, l.acknowledge(t, l.owner, r, revision, true, true, true, nil, "ready-before-withdraw"))
	need(t, l.acknowledge(t, l.owner, r, revision, true, false, true, nil, "withdraw-ready"))
	want(t, l.acknowledge(t, l.owner, r, revision, true, true, true, nil, "ready-before-withdraw"), auth.ErrConflict)
	_, e := l.start(t, l.owner, r, revision, "after-withdraw")
	want(t, e, auth.ErrDenied)
	l.inspectAbsent(t, r)
}
func TestOwnerManagementIsIndependentFromHostLaunchAndParticipantConsent(t *testing.T) {
	l := newLaunchFixture(t, nil)
	r := l.create(t)
	_, e := l.service.Configure(l.ctx, l.caller(l.owner, "configure-no-participant"), auth.RoomSecret(launch.ConfigureData{WorkspaceID: l.w, RoomID: r, ConfigurationID: "minimal"}))
	need(t, e)
	want(t, l.acknowledge(t, l.owner, r, 1, true, true, true, nil, "owner-no-participant-ready"), auth.ErrDenied)
	_, e = l.start(t, l.owner, r, 1, "owner-no-host-launch")
	want(t, e, auth.ErrDenied)
	l.inspectAbsent(t, r)
}
func TestLaunchRollbackRemovesEveryAuthoritativeEffectAndCanRetry(t *testing.T) {
	for _, point := range []string{"launch-after-session", "launch-after-data-targets", "launch-after-room-transition", "before-commit"} {
		t.Run(point, func(t *testing.T) {
			var armed atomic.Bool
			l := newLaunchFixture(t, func(_ context.Context, p string) error {
				if armed.Load() && p == point {
					return auth.ErrUnavailable
				}
				return nil
			})
			r, p := l.hostRoom(t)
			prep := l.configure(t, r, p, "minimal")
			l.ready(t, r, prep)
			armed.Store(true)
			_, e := l.start(t, l.owner, r, prep.StorageValue().Revision, "rollback-launch")
			if e == nil {
				t.Fatal("injected rollback accepted")
			}
			l.inspectAbsent(t, r)
			armed.Store(false)
			_, e = l.start(t, l.owner, r, prep.StorageValue().Revision, "rollback-launch")
			need(t, e)
		})
	}
}
func TestUnknownCommitRecoversOriginalSessionAndReceipt(t *testing.T) {
	var armed atomic.Bool
	l := newLaunchFixture(t, func(_ context.Context, p string) error {
		if armed.Load() && p == "after-commit-unknown" {
			return auth.ErrUnavailable
		}
		return nil
	})
	r, p := l.hostRoom(t)
	prep := l.configure(t, r, p, "minimal")
	l.ready(t, r, prep)
	armed.Store(true)
	_, e := l.start(t, l.owner, r, prep.StorageValue().Revision, "unknown-launch")
	want(t, e, auth.ErrOutcomeUnknown)
	if l.sql(t, `SELECT count(*) FROM platform_launch.sessions WHERE workspace_id='`+l.w+`'`) != "1" {
		t.Fatal("unknown commit lost native session")
	}
	armed.Store(false)
	s, e := l.start(t, l.owner, r, prep.StorageValue().Revision, "unknown-launch")
	need(t, e)
	if s.StorageValue().Binding.Session == "" || l.sql(t, `SELECT count(*) FROM host_command.sessions WHERE workspace='`+l.w+`'`) != "1" {
		t.Fatal("unknown retry duplicated creation")
	}
}
func TestActivationFailureRetriesCommittedCreation(t *testing.T) {
	l := newLaunchFixture(t, nil)
	r, p := l.hostRoom(t)
	prep := l.configure(t, r, p, "minimal")
	l.ready(t, r, prep)
	l.mu.Lock()
	seen := 0
	l.vmFault = func(v install.Execution) error {
		if v.Case == "session-vm-ready" {
			seen++
			if seen == 2 {
				return auth.ErrUnavailable
			}
		}
		return nil
	}
	l.mu.Unlock()
	_, e := l.start(t, l.owner, r, prep.StorageValue().Revision, "activation-failure")
	want(t, e, auth.ErrOutcomeUnknown)
	l.mu.Lock()
	l.vmFault = nil
	l.mu.Unlock()
	_, e = l.start(t, l.owner, r, prep.StorageValue().Revision, "activation-failure")
	need(t, e)
	if l.sql(t, `SELECT count(*) FROM host_command.sessions WHERE workspace='`+l.w+`'`) != "1" {
		t.Fatal("activation retry duplicated authoritative session")
	}
}
