// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

type memoryControl struct {
	mu        sync.Mutex
	execution sync.RWMutex
	value     State
	leases    []Lease
	now       time.Time
}

func (m *memoryControl) Bind(core.Transaction) (Transaction, error) { return m, nil }
func (m *memoryControl) Transact(ctx context.Context, f func(Transaction) error) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	return f(m)
}
func (m *memoryControl) ExecutionLock(ctx context.Context, _ data.Binding, exclusive bool) (func(), error) {
	if ctx.Err() != nil {
		return nil, auth.ErrUnavailable
	}
	if exclusive {
		m.execution.Lock()
		return m.execution.Unlock, nil
	}
	m.execution.RLock()
	return m.execution.RUnlock, nil
}
func (m *memoryControl) Now(context.Context) (time.Time, error)      { return m.now, nil }
func (m *memoryControl) Barrier(context.Context, data.Binding) error { return nil }
func (m *memoryControl) State(context.Context, data.Binding) (State, error) {
	if m.value.StorageValue().Revision == 0 {
		return State{}, auth.ErrDenied
	}
	return m.value, nil
}
func (m *memoryControl) PutState(_ context.Context, v State) error {
	m.value = auth.RoomSecret(ownState(v.StorageValue()))
	return nil
}
func (m *memoryControl) Leases(context.Context, data.Binding) ([]Lease, error) {
	return append([]Lease{}, m.leases...), nil
}
func (m *memoryControl) PutLease(_ context.Context, v Lease) error {
	for i, l := range m.leases {
		if l.StorageValue().ID == v.StorageValue().ID {
			m.leases[i] = v
			return nil
		}
	}
	m.leases = append(m.leases, v)
	return nil
}
func controlFixture(t *testing.T) (*Control, *memoryControl, launch.PlayerReference, launch.PlayerReference) {
	t.Helper()
	now := time.Now()
	b := data.Binding{Workspace: "workspace", Session: "session", GraphHash: "sha256:" + strings.Repeat("a", 64)}
	scope := core.Scope{WorkspaceID: "workspace", RoomID: "room", GameID: "game"}
	humans := []launch.PlayerHuman{{Principal: "one", Seat: "gm", Required: true}, {Principal: "two", Seat: "player", Required: true}}
	m := &memoryControl{now: now}
	c, e := NewControl(m)
	if e != nil {
		t.Fatal("control constructor rejected fixture")
	}
	r := launch.PlayerReferenceData{Access: launch.SessionAccessData{Scope: scope, Binding: b, ConfigurationHash: "sha256:" + strings.Repeat("b", 64), Principal: "one", Seat: "gm"}, PreparationRevision: 1, Humans: humans, OwnConfirmed: true, Ready: true}
	one := auth.RoomSecret(r)
	r.Access.Principal = "two"
	r.Access.Seat = "player"
	two := auth.RoomSecret(r)
	if _, e = c.ReadWithin(context.Background(), m, one); e != nil {
		t.Fatal("control initialization failed")
	}
	for _, h := range humans {
		m.leases = append(m.leases, auth.RoomSecret(LeaseData{Scope: scope, Binding: b, ID: h.Principal, Principal: h.Principal, Seat: h.Seat, CookieHash: "synthetic-private-cookie-hash", ExpiresAt: now.Add(LeaseLifetime)}))
	}
	return c, m, one, two
}
func TestPlayerResumeNeedsEachCurrentNecessaryParticipant(t *testing.T) {
	c, m, one, two := controlFixture(t)
	ctx := context.Background()
	if _, e := c.ResumeWithin(ctx, m, one, 1); e != nil {
		t.Fatal("own resume vote rejected")
	}
	if !m.value.StorageValue().Paused || !controlDTO(m.value, "two").OwnResumeRequired {
		t.Fatal("one participant resumed for another")
	}
	if _, e := c.ResumeWithin(ctx, m, two, 1); e != auth.ErrConflict {
		t.Fatal("stale control revision accepted")
	}
	if _, e := c.ResumeWithin(ctx, m, two, 2); e != nil || m.value.StorageValue().Paused {
		t.Fatal("current unanimous resume failed")
	}
	if _, e := c.PauseWithin(ctx, m, two, 3); e != nil {
		t.Fatal("ordinary participant could not pause")
	}
	if _, e := c.ResumeWithin(ctx, m, one, 4); e != auth.ErrConflict {
		t.Fatal("resume entered before pause fence")
	}
	if e := c.Quiesce(ctx, one.StorageValue().Access.Binding); e != nil {
		t.Fatal("pause fence failed")
	}
	if _, e := c.ResumeWithin(ctx, m, one, 4); e != nil || !m.value.StorageValue().Paused {
		t.Fatal("new pause retained another participant vote")
	}
}
func TestPlayerExpiryPersistsPauseBeforeMutationRefusal(t *testing.T) {
	c, m, one, two := controlFixture(t)
	ctx := context.Background()
	_, _ = c.ResumeWithin(ctx, m, one, 1)
	_, _ = c.ResumeWithin(ctx, m, two, 2)
	m.now = m.now.Add(LeaseLifetime + time.Second)
	if e := c.Check(ctx, one.StorageValue().Access.Scope, one.StorageValue().Access.Binding); e != ErrPaused {
		t.Fatal("expired human lease admitted mutation")
	}
	if !m.value.StorageValue().Paused || m.value.StorageValue().Reason != "disconnect" || m.value.StorageValue().Revision != 4 {
		t.Fatal("expiry pause was lost on refusal")
	}
	old := m.value.StorageValue().Revision
	if e := c.Check(ctx, one.StorageValue().Access.Scope, one.StorageValue().Access.Binding); e != ErrPaused || m.value.StorageValue().Revision != old {
		t.Fatal("repeated expiry changed control revision")
	}
}
func TestPlayerPreparationAndPrivateHandlesCannotBeSubstitutedOrLogged(t *testing.T) {
	c, m, one, _ := controlFixture(t)
	r := one.StorageValue()
	r.PreparationRevision++
	if _, e := c.ReadWithin(context.Background(), m, auth.RoomSecret(r)); e != auth.ErrDenied {
		t.Fatal("different preparation admitted")
	}
	for _, handle := range []any{c, m.value, m.leases[0], auth.RoomSecret(RequestData{Fields: map[string]any{"secret": "synthetic-private-cookie-hash"}})} {
		for _, format := range []string{"%v", "%+v", "%#v", "%s", "%d", "%x"} {
			if strings.Contains(fmt.Sprintf(format, handle), "synthetic-private-cookie-hash") {
				t.Fatal("private player handle leaked through formatting")
			}
		}
		if _, e := json.Marshal(handle); e == nil {
			t.Fatal("private player handle serialized")
		}
	}
}

func TestPlayerOwnResumeDoesNotRequireModelManagementPermission(t *testing.T) {
	c, m, one, two := controlFixture(t)
	a := two.StorageValue()
	a.Ready = false
	two = auth.RoomSecret(a)
	if _, e := c.ResumeWithin(context.Background(), m, one, 1); e != nil {
		t.Fatal("host own vote rejected")
	}
	a.OwnConfirmed = false
	if _, e := c.ResumeWithin(context.Background(), m, auth.RoomSecret(a), 2); e != auth.ErrConflict {
		t.Fatal("missing own current confirmation accepted")
	}
	if _, e := c.ResumeWithin(context.Background(), m, two, 2); e != nil || m.value.StorageValue().Paused {
		t.Fatal("ordinary own confirmation required model management permission")
	}
}
