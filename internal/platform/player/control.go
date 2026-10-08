// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package player supplies the approved HTTPS player facade. Its authorities,
// connection leases and storage values are private server composition seams.
package player

import (
	"context"
	"errors"
	"fmt"
	"io"
	"math"
	"slices"
	"sync"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

var ErrConnectionExpired = errors.New("PLAYER_CONNECTION_EXPIRED")
var ErrPaused = launch.ErrPlayerPaused

const LeaseLifetime = 30 * time.Second
const MaxRoomConnections = 32

type StateData struct {
	Scope                         core.Scope
	Binding                       data.Binding
	ConfigurationHash             string
	PreparationRevision, Revision uint64
	Paused                        bool
	Quiescing                     bool
	Reason                        string
	Humans                        []launch.PlayerHuman
	Resumed                       []string
}
type State = auth.Secret[StateData]

func (StateData) Format(f fmt.State, _ rune) { _, _ = io.WriteString(f, "<private player state data>") }

type LeaseData struct {
	Scope                           core.Scope
	Binding                         data.Binding
	ID, CookieHash, Principal, Seat string
	ExpiresAt                       time.Time
	Closed                          bool
}
type Lease = auth.Secret[LeaseData]

func (LeaseData) Format(f fmt.State, _ rune) { _, _ = io.WriteString(f, "<private player lease data>") }

type nativeLeaseKey struct{}

// NativeLease is a read-only trusted repository seam. Only this facade can put
// the private key into a context; an HTTP field can never construct a permit.
func NativeLease(ctx context.Context) Lease {
	if ctx == nil {
		return Lease{}
	}
	l, _ := ctx.Value(nativeLeaseKey{}).(Lease)
	return l
}
func withNativeLease(ctx context.Context, l Lease) context.Context {
	return context.WithValue(ctx, nativeLeaseKey{}, l)
}

// Barrier locks the existing native Session row before the control row. A
// pause acknowledgment therefore follows any transaction already executing.
// ExecutionLock fences external work too; it holds no auth/game SQL transaction.
type Storage interface {
	Bind(core.Transaction) (Transaction, error)
	Transact(context.Context, func(Transaction) error) error
	ExecutionLock(context.Context, data.Binding, bool) (func(), error)
}
type Transaction interface {
	Now(context.Context) (time.Time, error)
	Barrier(context.Context, data.Binding) error
	State(context.Context, data.Binding) (State, error)
	PutState(context.Context, State) error
	Leases(context.Context, data.Binding) ([]Lease, error)
	PutLease(context.Context, Lease) error
}
type Control struct{ data **controlData }
type controlData struct {
	storage Storage
	mu      sync.Mutex
	next    uint64
	active  map[uint64]execution
}
type execution struct {
	binding data.Binding
	cancel  context.CancelFunc
}

func (Control) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<private player control>") }
func (Control) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (c *Control) state() *controlData {
	if c == nil || c.data == nil {
		return nil
	}
	return *c.data
}
func NewControl(s Storage) (*Control, error) {
	if s == nil {
		return nil, auth.ErrInvalid
	}
	d := &controlData{storage: s, active: map[uint64]execution{}}
	return &Control{data: &d}, nil
}
func ValidState(v StateData) bool {
	if !store.ValidID(v.Scope.WorkspaceID) || !store.ValidID(v.Scope.RoomID) || !store.ValidID(v.Scope.GameID) || v.Binding.Workspace != v.Scope.WorkspaceID || !store.ValidID(v.Binding.Session) || !checkpoint.IsDigest(v.Binding.GraphHash) || !checkpoint.IsDigest(v.ConfigurationHash) || v.Revision < 1 || v.Revision >= math.MaxInt64 || v.PreparationRevision < 1 || v.PreparationRevision >= math.MaxInt64 || len(v.Humans) < 1 || len(v.Humans) > 64 || len(v.Resumed) > 64 {
		return false
	}
	if (!v.Paused && v.Reason != "none") || (v.Paused && v.Reason != "safety" && v.Reason != "disconnect" && v.Reason != "service_unavailable") {
		return false
	}
	if v.Quiescing && !v.Paused {
		return false
	}
	people, seats := map[string]bool{}, map[string]bool{}
	for _, h := range v.Humans {
		if !store.ValidID(h.Principal) || !store.ValidID(h.Seat) || people[h.Principal] || seats[h.Seat] {
			return false
		}
		people[h.Principal] = true
		seats[h.Seat] = true
	}
	seen := map[string]bool{}
	for _, p := range v.Resumed {
		if !people[p] || seen[p] {
			return false
		}
		seen[p] = true
	}
	return true
}
func ownState(v StateData) StateData {
	v.Humans = slices.Clone(v.Humans)
	v.Resumed = slices.Clone(v.Resumed)
	return v
}
func needed(v StateData, principal string) bool {
	any := false
	for _, h := range v.Humans {
		if h.Required {
			any = true
			if h.Principal == principal {
				return true
			}
		}
	}
	if any {
		return false
	}
	// A configuration with optional human seats still needs its actual humans
	// to confirm, so it can never silently continue with zero human agreement.
	for _, h := range v.Humans {
		if h.Principal == principal {
			return true
		}
	}
	return false
}
func liveLease(v StateData, leases []Lease, p string, now time.Time) bool {
	for _, l := range leases {
		x := l.StorageValue()
		if x.Scope == v.Scope && x.Binding == v.Binding && x.Principal == p && !x.Closed && now.Before(x.ExpiresAt) {
			for _, h := range v.Humans {
				if h.Principal == p && h.Seat == x.Seat {
					return true
				}
			}
		}
	}
	return false
}
func disconnected(v StateData, leases []Lease, now time.Time) bool {
	for _, h := range v.Humans {
		if needed(v, h.Principal) && !liveLease(v, leases, h.Principal, now) {
			return true
		}
	}
	return false
}
func pauseState(v StateData, reason string) (StateData, error) {
	if v.Revision+1 >= math.MaxInt64 {
		return StateData{}, auth.ErrConflict
	}
	v = ownState(v)
	v.Revision++
	v.Paused = true
	v.Quiescing = true
	v.Reason = reason
	v.Resumed = []string{}
	return v, nil
}
func (c *Control) Bind(tx core.Transaction) (Transaction, error) {
	if c.state() == nil || tx == nil {
		return nil, auth.ErrDenied
	}
	return c.state().storage.Bind(tx)
}
func (c *Control) ReadWithin(ctx context.Context, tx Transaction, ref launch.PlayerReference) (State, error) {
	r := ref.StorageValue()
	a := r.Access
	if tx == nil || c.state() == nil || ctx == nil || ctx.Err() != nil {
		return State{}, auth.ErrDenied
	}
	if e := tx.Barrier(ctx, a.Binding); e != nil {
		return State{}, auth.SafeError(e)
	}
	s, e := tx.State(ctx, a.Binding)
	var v StateData
	if e == auth.ErrDenied {
		v = StateData{Scope: a.Scope, Binding: a.Binding, ConfigurationHash: a.ConfigurationHash, PreparationRevision: r.PreparationRevision, Revision: 1, Paused: true, Reason: "disconnect", Humans: slices.Clone(r.Humans), Resumed: []string{}}
		if !ValidState(v) {
			return State{}, auth.ErrDenied
		}
		if e = tx.PutState(ctx, auth.RoomSecret(v)); e != nil {
			return State{}, auth.SafeError(e)
		}
	} else if e != nil {
		return State{}, auth.SafeError(e)
	} else {
		v = ownState(s.StorageValue())
	}
	if !ValidState(v) || v.Scope != a.Scope || v.Binding != a.Binding || v.ConfigurationHash != a.ConfigurationHash || v.PreparationRevision != r.PreparationRevision || !slices.Equal(v.Humans, r.Humans) {
		return State{}, auth.ErrDenied
	}
	return refresh(ctx, tx, v)
}
func refresh(ctx context.Context, tx Transaction, v StateData) (State, error) {
	if !ValidState(v) {
		return State{}, auth.ErrDenied
	}
	leases, e := tx.Leases(ctx, v.Binding)
	if e != nil {
		return State{}, auth.SafeError(e)
	}
	now, e := tx.Now(ctx)
	if e != nil {
		return State{}, auth.SafeError(e)
	}
	if !v.Paused && disconnected(v, leases, now) {
		v, e = pauseState(v, "disconnect")
		if e != nil {
			return State{}, e
		}
		if e = tx.PutState(ctx, auth.RoomSecret(v)); e != nil {
			return State{}, auth.SafeError(e)
		}
	}
	return auth.RoomSecret(ownState(v)), nil
}
func (c *Control) Check(ctx context.Context, scope core.Scope, b data.Binding) error {
	if c.state() == nil || ctx == nil || ctx.Err() != nil {
		return auth.ErrDenied
	}
	paused := true
	e := c.state().storage.Transact(ctx, func(tx Transaction) error {
		if e := tx.Barrier(ctx, b); e != nil {
			return auth.SafeError(e)
		}
		s, e := tx.State(ctx, b)
		if e != nil {
			return auth.SafeError(e)
		}
		v := s.StorageValue()
		if v.Scope != scope || v.Binding != b {
			return auth.ErrDenied
		}
		s, e = refresh(ctx, tx, v)
		if e != nil {
			return e
		}
		paused = s.StorageValue().Paused
		return nil // Persist expiry before reporting its refusal to the caller.
	})
	if e != nil {
		return auth.SafeError(e)
	}
	if paused {
		return ErrPaused
	}
	return nil
}
func (c *Control) BeginMutation(ctx context.Context, scope core.Scope, b data.Binding) (context.Context, func(), error) {
	if c.state() == nil || ctx == nil || ctx.Err() != nil {
		return nil, nil, auth.ErrDenied
	}
	d := c.state()
	d.mu.Lock()
	if len(d.active) >= 4 {
		d.mu.Unlock()
		return nil, nil, auth.ErrRateLimited
	}
	d.next++
	id := d.next
	owned, cancel := context.WithCancel(ctx)
	d.active[id] = execution{b, cancel}
	d.mu.Unlock()
	var once sync.Once
	unlock, e := d.storage.ExecutionLock(owned, b, false)
	release := func() {
		once.Do(func() {
			cancel()
			if unlock != nil {
				unlock()
			}
			d.mu.Lock()
			delete(d.active, id)
			d.mu.Unlock()
		})
	}
	if e != nil {
		release()
		return nil, nil, auth.SafeError(e)
	}
	if e = c.Check(owned, scope, b); e != nil {
		release()
		return nil, nil, e
	}
	return owned, release, nil
}

// Quiesce runs AFTER the durable pause transaction commits. Cancellation stops
// further provider attempts; the exclusive execution fence waits for admitted
// work to leave. No auth lock is held while waiting for the Actor/provider.
func (c *Control) Quiesce(ctx context.Context, b data.Binding) error {
	if c.state() == nil {
		return auth.ErrDenied
	}
	d := c.state()
	d.mu.Lock()
	for _, a := range d.active {
		if a.binding == b {
			a.cancel()
		}
	}
	d.mu.Unlock()
	u, e := d.storage.ExecutionLock(ctx, b, true)
	if e != nil {
		return auth.SafeError(e)
	}
	defer u()
	return d.storage.Transact(ctx, func(tx Transaction) error {
		if e := tx.Barrier(ctx, b); e != nil {
			return e
		}
		state, e := tx.State(ctx, b)
		if e != nil {
			return e
		}
		v := state.StorageValue()
		if v.Paused && v.Quiescing {
			v = ownState(v)
			v.Quiescing = false
			return tx.PutState(ctx, auth.RoomSecret(v))
		}
		return nil
	})
}
func (c *Control) PauseWithin(ctx context.Context, tx Transaction, ref launch.PlayerReference, expected uint64) (State, error) {
	s, e := c.ReadWithin(ctx, tx, ref)
	if e != nil {
		return State{}, e
	}
	v := s.StorageValue()
	if v.Revision != expected {
		return State{}, auth.ErrConflict
	}
	v, e = pauseState(v, "safety")
	if e != nil {
		return State{}, e
	}
	if e = tx.PutState(ctx, auth.RoomSecret(v)); e != nil {
		return State{}, auth.SafeError(e)
	}
	return auth.RoomSecret(v), nil
}
func (c *Control) ResumeWithin(ctx context.Context, tx Transaction, ref launch.PlayerReference, expected uint64) (State, error) {
	s, e := c.ReadWithin(ctx, tx, ref)
	if e != nil {
		return State{}, e
	}
	v := s.StorageValue()
	r := ref.StorageValue()
	if v.Revision != expected || !v.Paused || v.Quiescing || !r.OwnConfirmed || !r.Ready {
		return State{}, auth.ErrConflict
	}
	leases, e := tx.Leases(ctx, v.Binding)
	if e != nil {
		return State{}, auth.SafeError(e)
	}
	now, e := tx.Now(ctx)
	if e != nil {
		return State{}, auth.SafeError(e)
	}
	if !liveLease(v, leases, r.Access.Principal, now) {
		return State{}, auth.ErrDenied
	}
	v = ownState(v)
	if needed(v, r.Access.Principal) && !slices.Contains(v.Resumed, r.Access.Principal) {
		v.Resumed = append(v.Resumed, r.Access.Principal)
		slices.Sort(v.Resumed)
	}
	all := !disconnected(v, leases, now)
	for _, h := range v.Humans {
		if needed(v, h.Principal) && !slices.Contains(v.Resumed, h.Principal) {
			all = false
		}
	}
	if v.Revision+1 >= math.MaxInt64 {
		return State{}, auth.ErrConflict
	}
	v.Revision++
	if all {
		v.Paused = false
		v.Reason = "none"
	}
	if e = tx.PutState(ctx, auth.RoomSecret(v)); e != nil {
		return State{}, auth.SafeError(e)
	}
	return auth.RoomSecret(v), nil
}
func SafeError(e error) error {
	if errors.Is(e, ErrPaused) {
		return ErrPaused
	}
	if errors.Is(e, ErrConnectionExpired) {
		return ErrConnectionExpired
	}
	return auth.SafeError(e)
}
func Disconnected(v StateData, leases []Lease, now time.Time) bool {
	return disconnected(v, leases, now)
}
func PauseDisconnected(v StateData) (State, error) {
	x, e := pauseState(v, "disconnect")
	if e != nil {
		return State{}, e
	}
	return auth.RoomSecret(x), nil
}
