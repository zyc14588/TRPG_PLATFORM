//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package launch_test

import (
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"sync"
	"testing"
)

func TestConcurrentLaunchCreatesOneSessionAndOneActorRecovery(t *testing.T) {
	l := newLaunchFixture(t, nil)
	r, part := l.hostRoom(t)
	p := l.configure(t, r, part, "minimal")
	l.ready(t, r, p)
	type outcome struct {
		key string
		err error
	}
	results := make(chan outcome, 4)
	start := make(chan struct{})
	var workers sync.WaitGroup
	for i := 0; i < 4; i++ {
		key := fmt.Sprintf("concurrent-launch-%d", i)
		workers.Add(1)
		go func() {
			defer workers.Done()
			<-start
			_, e := l.service.Launch(l.ctx, l.caller(l.owner, key), auth.RoomSecret(launch.LaunchData{WorkspaceID: l.w, RoomID: r, Revision: p.StorageValue().Revision}))
			results <- outcome{key: key, err: e}
		}()
	}
	close(start)
	workers.Wait()
	close(results)
	var first launch.Session
	for result := range results {
		if result.err != nil && result.err != auth.ErrOutcomeUnknown {
			need(t, result.err)
		}
		s, e := l.start(t, l.owner, r, p.StorageValue().Revision, result.key)
		need(t, e)
		if first.StorageValue().Binding.Session == "" {
			first = s
		} else if first.StorageValue().Binding != s.StorageValue().Binding {
			t.Fatal("concurrent retries changed the authoritative binding")
		}
	}
	for _, table := range []string{"sessions", "creation"} {
		if l.sql(t, `SELECT count(*) FROM host_command.`+table+` WHERE workspace='`+l.w+`'`) != "1" {
			t.Fatal("concurrent launch duplicated an authoritative effect")
		}
	}
	l.mu.Lock()
	ready := 0
	for _, event := range l.executions {
		if event.Case == "session-vm-ready" {
			ready++
		}
	}
	l.mu.Unlock()
	if ready != 2 {
		t.Fatal("concurrent launch created more than one initial VM and one Actor recovery")
	}
}

func TestHostRevocationRejectsLaunchAndPreviouslySuccessfulReceipt(t *testing.T) {
	l := newLaunchFixture(t, nil)
	r, part := l.hostRoom(t)
	p := l.configure(t, r, part, "minimal")
	l.ready(t, r, p)
	role := func(enabled bool) {
		l.call(t, l.owner, "set_role", l.w, r, part, map[string]any{"role": "host", "enabled": enabled})
	}
	role(false)
	_, e := l.start(t, l.owner, r, p.StorageValue().Revision, "host-revoked-before-launch")
	want(t, e, auth.ErrDenied)
	l.inspectAbsent(t, r)
	role(true)
	_, e = l.start(t, l.owner, r, p.StorageValue().Revision, "host-receipt-before-revocation")
	need(t, e)
	_, e = l.do(t, l.owner, "post-launch-role-attempt", l.request(t, "set_role", l.w, r, part, map[string]any{"role": "host", "enabled": false}))
	want(t, e, auth.ErrConflict)
	// The public role operation is lobby-only; inject native revocation to
	// verify that a successful launch receipt still checks current authority.
	l.sql(t, `UPDATE platform_room.participants SET host=false WHERE workspace_id='`+l.w+`' AND room_id='`+r+`' AND game_id='game' AND id='`+part+`'`)
	if l.sql(t, `SELECT count(*) FROM platform_room.participants WHERE workspace_id='`+l.w+`' AND room_id='`+r+`' AND id='`+part+`' AND NOT host`) != "1" {
		t.Fatal("native Host revocation was not applied")
	}
	for _, key := range []string{"host-receipt-before-revocation", "host-new-key-after-revocation"} {
		_, e = l.start(t, l.owner, r, p.StorageValue().Revision, key)
		want(t, e, auth.ErrDenied)
	}
	if l.sql(t, `SELECT count(*) FROM host_command.sessions WHERE workspace='`+l.w+`'`) != "1" {
		t.Fatal("revocation or denied replay changed committed creation")
	}
}

func TestCurrentPackageAccessAndPolicyAreRecheckedAfterConsent(t *testing.T) {
	for _, kind := range []string{"grant", "policy"} {
		t.Run(kind, func(t *testing.T) {
			l := newLaunchFixture(t, nil)
			r, part := l.hostRoom(t)
			p := l.configure(t, r, part, "minimal")
			l.ready(t, r, p)
			if kind == "grant" {
				l.sql(t, `DELETE FROM package_install.grants WHERE workspace='`+l.w+`'`)
			} else {
				l.sql(t, `UPDATE package_install.artifacts SET policy_digest='sha256:`+fmt.Sprintf("%064d", 0)+`' WHERE workspace='`+l.w+`'`)
			}
			_, e := l.start(t, l.owner, r, p.StorageValue().Revision, "package-rechecked-at-launch")
			want(t, e, auth.ErrDenied)
			l.inspectAbsent(t, r)
		})
	}
}

func TestChangedServerMetadataCannotReuseAcknowledgmentsAfterRecomposition(t *testing.T) {
	l := newLaunchFixture(t, nil)
	r, part := l.hostRoom(t)
	p := l.configure(t, r, part, "minimal")
	l.ready(t, r, p)
	changed := append([]launch.Configuration(nil), l.configs...)
	c := changed[0].StorageValue()
	c.ContentTags = []string{"violence"}
	changed[0] = auth.RoomSecret(c)
	l.recomposeLaunch(t, changed)
	_, e := l.start(t, l.owner, r, p.StorageValue().Revision, "changed-content-old-consent")
	want(t, e, auth.ErrDenied)
	l.inspectAbsent(t, r)
	next := l.configure(t, r, part, "minimal")
	if next.StorageValue().ConfigurationHash == p.StorageValue().ConfigurationHash || next.StorageValue().Revision != p.StorageValue().Revision+1 {
		t.Fatal("changed server metadata retained the old acknowledgment binding")
	}
	_, e = l.start(t, l.owner, r, next.StorageValue().Revision, "changed-content-without-new-consent")
	want(t, e, auth.ErrDenied)
	l.inspectAbsent(t, r)
	l.ready(t, r, next)
	_, e = l.start(t, l.owner, r, next.StorageValue().Revision, "changed-content-new-consent")
	need(t, e)
}

func TestGuestReadinessUsesCurrentNativeExpiryAndRevocation(t *testing.T) {
	for _, state := range []string{"active", "expired", "revoked"} {
		t.Run(state, func(t *testing.T) {
			l := newLaunchFixture(t, nil)
			r, host := l.hostRoom(t)
			guest, _, _ := l.guest(t, r, l.invite(t, r, false, 8))
			part := l.sql(t, `SELECT id FROM platform_room.participants WHERE workspace_id='`+l.w+`' AND room_id='`+r+`' AND guest_id IS NOT NULL AND guest_id<>''`)
			if part == "" {
				t.Fatal("native guest participant missing")
			}
			c := l.configs[0].StorageValue()
			c.ID = "guest-readiness"
			c.Seats = append(append([]launch.SeatRule(nil), c.Seats...), launch.SeatRule{ID: "player", Required: true, Modes: []string{"human"}})
			l.recomposeLaunch(t, append(append([]launch.Configuration(nil), l.configs...), auth.RoomSecret(c)))
			p, e := l.service.Configure(l.ctx, l.caller(l.owner, "configure-native-guest"), auth.RoomSecret(launch.ConfigureData{WorkspaceID: l.w, RoomID: r, ConfigurationID: c.ID, Slots: []launch.Slot{{ID: "gm", Mode: "human", ParticipantID: host}, {ID: "player", Mode: "human", ParticipantID: part}}}))
			need(t, e)
			l.ready(t, r, p)
			need(t, l.acknowledge(t, guest, r, p.StorageValue().Revision, true, true, true, nil, "guest-current-ready"))
			if state == "expired" {
				l.sql(t, `UPDATE platform_core.guests SET expires_at=clock_timestamp()-interval '1 second' WHERE workspace_id='`+l.w+`' AND room_id='`+r+`'`)
			} else if state == "revoked" {
				l.sql(t, `UPDATE platform_core.guests SET disabled=true WHERE workspace_id='`+l.w+`' AND room_id='`+r+`'`)
			}
			_, e = l.start(t, l.owner, r, p.StorageValue().Revision, "native-guest-launch")
			if state == "active" {
				need(t, e)
			} else {
				want(t, e, auth.ErrDenied)
				l.inspectAbsent(t, r)
			}
		})
	}
}

func TestLaunchRejectsForeignAccountAndMixedWorkspaceRoomScope(t *testing.T) {
	l := newLaunchFixture(t, nil)
	r, part := l.hostRoom(t)
	p := l.configure(t, r, part, "minimal")
	l.ready(t, r, p)
	other := newLaunchFixture(t, nil)
	_, e := l.start(t, other.owner, r, p.StorageValue().Revision, "foreign-account-launch")
	want(t, e, auth.ErrDenied)
	_, e = l.service.Launch(l.ctx, l.caller(l.owner, "mixed-workspace-room-launch"), auth.RoomSecret(launch.LaunchData{WorkspaceID: other.w, RoomID: r, Revision: p.StorageValue().Revision}))
	want(t, e, auth.ErrDenied)
	l.inspectAbsent(t, r)
}
