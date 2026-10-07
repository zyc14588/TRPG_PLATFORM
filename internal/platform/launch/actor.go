// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package launch

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/actor"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/realtime"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"io"
	"sync"
	"time"
)

// One platformd composition owns this bounded set. Each registered game has
// exactly one existing SessionActor registry with MaxSessions=1. Lobby writes
// never create an actor. The activation credential is private and admits no
// command or view fields; platform transport/seat authorization is B005.
type Coordinator struct{ data **coordinatorData }
type coordinatorData struct {
	mu      sync.Mutex
	ctx     context.Context
	cancel  context.CancelFunc
	storage Storage
	max     int
	entries map[string]*activation
	closed  bool
}
type activation struct {
	binding   data.Binding
	registry  *actor.Registry
	authority *command.Authority
	hub       *realtime.Hub
	active    bool
}
type Reservation struct{ data **reservationData }
type reservationData struct {
	owner *Coordinator
	entry *activation
}

func (Coordinator) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<game actor coordinator>") }
func (Coordinator) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (Reservation) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<game activation reservation>")
}
func (Reservation) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (c *Coordinator) state() *coordinatorData {
	if c == nil || c.data == nil {
		return nil
	}
	return *c.data
}
func NewCoordinator(ctx context.Context, storage Storage, max int) (*Coordinator, error) {
	if ctx == nil || ctx.Err() != nil || storage == nil || max < 1 || max > 256 {
		return nil, auth.ErrInvalid
	}
	owned, cancel := context.WithCancel(ctx)
	d := &coordinatorData{ctx: owned, cancel: cancel, storage: storage, max: max, entries: map[string]*activation{}}
	return &Coordinator{data: &d}, nil
}
func bindingKey(b data.Binding) string { return b.Workspace + "/" + b.Session }
func validBinding(b data.Binding) bool {
	return store.ValidID(b.Workspace) && store.ValidID(b.Session) && checkpoint.IsDigest(b.GraphHash)
}
func (c *Coordinator) Reserve(b data.Binding) (*Reservation, error) {
	d := c.state()
	if d == nil || !validBinding(b) {
		return nil, auth.ErrDenied
	}
	d.mu.Lock()
	defer d.mu.Unlock()
	if d.closed || d.ctx.Err() != nil {
		return nil, auth.ErrUnavailable
	}
	if _, exists := d.entries[bindingKey(b)]; exists {
		return nil, auth.ErrConflict
	}
	if len(d.entries) >= d.max {
		return nil, auth.ErrConflict
	}
	e := &activation{binding: b}
	d.entries[bindingKey(b)] = e
	r := &reservationData{owner: c, entry: e}
	return &Reservation{data: &r}, nil
}
func (r *Reservation) Cancel() {
	if r == nil || r.data == nil || *r.data == nil {
		return
	}
	v := *r.data
	d := v.owner.state()
	d.mu.Lock()
	defer d.mu.Unlock()
	if d.entries[bindingKey(v.entry.binding)] == v.entry && !v.entry.active && v.entry.registry == nil {
		delete(d.entries, bindingKey(v.entry.binding))
	}
}
func (c *Coordinator) Activate(ctx context.Context, reserved *Reservation, b data.Binding, config Configuration, seat string) error {
	d := c.state()
	cfg := config.StorageValue()
	if d == nil || ctx == nil || ctx.Err() != nil || !validBinding(b) || !store.ValidID(seat) || cfg.WorkspaceID != b.Workspace {
		return auth.ErrDenied
	}
	d.mu.Lock()
	if d.closed || d.ctx.Err() != nil {
		d.mu.Unlock()
		return auth.ErrUnavailable
	}
	existing := d.entries[bindingKey(b)]
	if existing != nil && existing.active {
		same := existing.binding == b
		d.mu.Unlock()
		if same {
			return nil
		}
		return auth.ErrDenied
	}
	if reserved == nil {
		if existing != nil {
			d.mu.Unlock()
			return auth.ErrConflict
		}
		if len(d.entries) >= d.max {
			d.mu.Unlock()
			return auth.ErrConflict
		}
		existing = &activation{binding: b}
		d.entries[bindingKey(b)] = existing
		v := &reservationData{owner: c, entry: existing}
		reserved = &Reservation{data: &v}
	}
	if reserved.data == nil || *reserved.data == nil || (*reserved.data).owner != c || (*reserved.data).entry != existing || existing == nil || existing.binding != b || existing.registry != nil {
		d.mu.Unlock()
		return auth.ErrDenied
	}
	var token [32]byte
	if _, e := rand.Read(token[:]); e != nil {
		delete(d.entries, bindingKey(b))
		d.mu.Unlock()
		return auth.ErrUnavailable
	}
	credential := hex.EncodeToString(token[:])
	clear(token[:])
	authority, e := command.NewFixtureAuthority([]command.FixtureSeat{{Credential: credential, Binding: b, Principal: "launch-system", Seat: seat, Commands: map[string]func(checkpoint.Value) error{}, Views: command.ViewPolicy{}}})
	if e != nil {
		delete(d.entries, bindingKey(b))
		d.mu.Unlock()
		return auth.ErrUnavailable
	}
	identity, e := authority.Authenticate(credential, b.Session, seat)
	credential = ""
	if e != nil {
		delete(d.entries, bindingKey(b))
		d.mu.Unlock()
		return auth.ErrUnavailable
	}
	hub, e := realtime.New(realtime.Options{Authority: authority, Capacity: 32, PerSession: 32, Queue: 16})
	if e != nil {
		delete(d.entries, bindingKey(b))
		d.mu.Unlock()
		return auth.ErrUnavailable
	}
	q := cfg.Request
	q.Workspace = b.Workspace
	q.Session = b.Session
	factory, e := cfg.Factory.WithRepository(d.storage.SessionRepository())
	if e != nil {
		hub.Close()
		delete(d.entries, bindingKey(b))
		d.mu.Unlock()
		return auth.ErrUnavailable
	}
	backend, e := newSessionRuntime(factory, d.storage.RuntimeRepository(), q, b, authority)
	if e != nil {
		hub.Close()
		delete(d.entries, bindingKey(b))
		d.mu.Unlock()
		return auth.ErrUnavailable
	}
	rg, e := actor.New(d.ctx, actor.Options{Authority: authority, Hub: hub, Backend: backend, MaxSessions: 1, Mailbox: 8, Idle: time.Minute, CommandTimeout: 3 * time.Second})
	if e != nil {
		hub.Close()
		delete(d.entries, bindingKey(b))
		d.mu.Unlock()
		return auth.ErrUnavailable
	}
	existing.registry = rg
	existing.authority = authority
	existing.hub = hub
	d.mu.Unlock()
	// Actual installed graph recovery runs through the existing mailbox. Its
	// private projection is ignored; an empty view policy permits no fields.
	_, e = rg.Reconnect(ctx, identity, 0, nil)
	if e != nil {
		_ = rg.Close()
		d.mu.Lock()
		if d.entries[bindingKey(b)] == existing {
			delete(d.entries, bindingKey(b))
		}
		d.mu.Unlock()
		return auth.ErrUnavailable
	}
	d.mu.Lock()
	if d.closed || d.entries[bindingKey(b)] != existing {
		d.mu.Unlock()
		_ = rg.Close()
		return auth.ErrUnavailable
	}
	existing.active = true
	d.mu.Unlock()
	return nil
}
func (c *Coordinator) Close() error {
	d := c.state()
	if d == nil {
		return nil
	}
	d.mu.Lock()
	d.closed = true
	d.cancel()
	all := []*actor.Registry{}
	for _, e := range d.entries {
		if e.registry != nil {
			all = append(all, e.registry)
		}
	}
	d.mu.Unlock()
	var failed bool
	for _, rg := range all {
		if rg.Close() != nil {
			failed = true
		}
	}
	if failed {
		return auth.ErrUnavailable
	}
	return nil
}
