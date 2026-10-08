// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package session composes the authenticated native game transport. Its typed
// handles are internal server seams; no new HTTP/wire or export Schema is defined.
package session

import (
	"context"
	"errors"
	"fmt"
	"io"
	"math"
	"sync/atomic"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/actor"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/realtime"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

var ErrBackpressure = errors.New("SESSION_BACKPRESSURE")

type ExportKind string

const (
	Public        ExportKind = "public"
	Personal      ExportKind = "personal"
	Host          ExportKind = "host"
	Administrator ExportKind = "administrator"
)

// Policies are current, trusted package policies. Callers cannot select their
// commands, filters, random inputs or management rights through an envelope.
type SeatPolicy struct {
	Commands      map[string]func(checkpoint.Value) error
	View          command.ViewPolicy
	Inputs        command.NativeInputSource
	RecoveryPoint bool
	Exports       map[ExportKind]command.ViewPolicy
}
type PolicyProvider interface {
	Current(context.Context, launch.SessionAccess) (SeatPolicy, error)
}
type PointData struct {
	Scope           core.Scope
	Binding         data.Binding
	Version, Cursor uint64
}
type Point = auth.Secret[PointData]
type PageData struct {
	Binding                     data.Binding
	Version, Cursor, NextCursor uint64
	More, Ended                 bool
	Events                      []data.JournalEvent
}
type Page = auth.Secret[PageData]
type Storage interface {
	ReadPoint(context.Context, core.Scope, data.Binding) (Point, error)
	ReadPage(context.Context, core.Scope, data.Binding, uint64, int) (Page, error)
}
type Options struct {
	Launch   *launch.Service
	Storage  Storage
	Policies PolicyProvider
}
type Service struct{ data **serviceData }
type serviceData struct {
	launch   *launch.Service
	storage  Storage
	policies PolicyProvider
}
type Connection struct{ data **connectionData }
type connectionData struct {
	owner     *Service
	caller    launch.Caller
	basis     launch.SessionAccess
	transport *launch.SessionTransport
	closed    atomic.Bool
}
type ResultData struct {
	CommandID       string
	Version, Cursor uint64
	Replayed        bool
	Result          checkpoint.Value
}
type Result = auth.Secret[ResultData]
type Frame = auth.Secret[realtime.Frame]
type ExportData struct {
	FormatVersion               int
	Kind                        ExportKind
	Scope                       core.Scope
	Binding                     data.Binding
	Seat                        string
	Version, Cursor, NextCursor uint64
	More, Ended                 bool
	View                        checkpoint.Value
	Events                      []data.JournalEvent
}
type Export = auth.Secret[ExportData]

func (Service) Format(f fmt.State, _ rune)      { _, _ = io.WriteString(f, "<private game service>") }
func (Service) MarshalJSON() ([]byte, error)    { return nil, auth.ErrDenied }
func (Connection) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<private game connection>") }
func (Connection) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (s *Service) state() *serviceData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}
func (c *Connection) state() *connectionData {
	if c == nil || c.data == nil {
		return nil
	}
	return *c.data
}
func New(o Options) (*Service, error) {
	if o.Launch == nil || o.Storage == nil || o.Policies == nil {
		return nil, auth.ErrInvalid
	}
	d := &serviceData{launch: o.Launch, storage: o.Storage, policies: o.Policies}
	return &Service{data: &d}, nil
}
func (s *Service) UsesLaunch(l *launch.Service) bool {
	return s.state() != nil && l != nil && s.state().launch == l
}
func (c *Connection) CurrentAccess(ctx context.Context) (launch.SessionAccess, error) {
	a, _, e := c.current(ctx, false)
	return a, e
}

// SnapshotPage uses the actual current seat view policy and raw scan cursor,
// so even pages containing only hidden events make bounded forward progress.
func (c *Connection) SnapshotPage(ctx context.Context, after uint64, limit int) (Export, error) {
	if after >= math.MaxInt64 || limit < 1 || limit > 128 {
		return Export{}, auth.ErrInvalid
	}
	a, p, e := c.current(ctx, false)
	if e != nil {
		return Export{}, e
	}
	v := a.StorageValue()
	page, e := c.state().owner.state().storage.ReadPage(ctx, v.Scope, v.Binding, after, limit)
	if e != nil {
		return Export{}, SafeError(e)
	}
	d := page.StorageValue()
	if d.Binding != v.Binding || d.Version < 1 || d.Version >= math.MaxInt64 || d.Cursor >= math.MaxInt64 || d.NextCursor < after || d.NextCursor > d.Cursor || len(d.Events) > limit {
		return Export{}, auth.ErrDenied
	}
	view := checkpoint.Object(map[string]checkpoint.Value{})
	if !d.Ended {
		frame, e := c.state().transport.Snapshot(ctx, after)
		if e != nil {
			return Export{}, SafeError(e)
		}
		if frame.Version != d.Version || frame.Cursor != d.Cursor {
			return Export{}, auth.ErrConflict
		}
		view = frame.View
	}
	a, p, e = c.current(ctx, false)
	if e != nil {
		return Export{}, e
	}
	v = a.StorageValue()
	return auth.RoomSecret(ExportData{FormatVersion: 1, Kind: Personal, Scope: v.Scope, Binding: v.Binding, Seat: v.Seat, Version: d.Version, Cursor: d.Cursor, NextCursor: d.NextCursor, More: d.More, Ended: d.Ended, View: realtime.Filter(view, p.View.ViewFields), Events: realtime.FilterEvents(d.Events, p.View)}), nil
}
func SafeError(e error) error {
	switch {
	case e == nil:
		return nil
	case errors.Is(e, launch.ErrPlayerPaused):
		return launch.ErrPlayerPaused
	case errors.Is(e, command.ErrDenied), errors.Is(e, data.ErrDenied):
		return auth.ErrDenied
	case errors.Is(e, command.ErrEnvelope):
		return auth.ErrInvalid
	case errors.Is(e, data.ErrConflict):
		return auth.ErrConflict
	case errors.Is(e, data.ErrUnknownCommit):
		return auth.ErrOutcomeUnknown
	case errors.Is(e, actor.ErrBackpressure), errors.Is(e, realtime.ErrCapacity), errors.Is(e, ErrBackpressure):
		return ErrBackpressure
	default:
		return auth.SafeError(e)
	}
}
func ownPolicy(p SeatPolicy) (SeatPolicy, error) {
	if len(p.Commands) > 32 || len(p.Exports) > 4 {
		return SeatPolicy{}, auth.ErrDenied
	}
	q := p
	q.Commands = map[string]func(checkpoint.Value) error{}
	q.Exports = map[ExportKind]command.ViewPolicy{}
	for k, v := range p.Commands {
		if v == nil {
			return SeatPolicy{}, auth.ErrDenied
		}
		q.Commands[k] = v
	}
	var e error
	q.View, e = command.OwnPolicy(p.View)
	if e != nil {
		return SeatPolicy{}, auth.ErrDenied
	}
	for k, v := range p.Exports {
		if k != Public && k != Personal && k != Host && k != Administrator {
			return SeatPolicy{}, auth.ErrDenied
		}
		q.Exports[k], e = command.OwnPolicy(v)
		if e != nil {
			return SeatPolicy{}, auth.ErrDenied
		}
	}
	return q, nil
}
func sameAccess(a, b launch.SessionAccessData) bool {
	return a.Scope == b.Scope && a.Binding == b.Binding && a.Principal == b.Principal && a.Seat == b.Seat && a.ConfigurationID == b.ConfigurationID && a.ConfigurationHash == b.ConfigurationHash
}
func (c *Connection) current(ctx context.Context, mutation bool) (launch.SessionAccess, SeatPolicy, error) {
	d := c.state()
	if d == nil || d.closed.Load() || ctx == nil || ctx.Err() != nil {
		return launch.SessionAccess{}, SeatPolicy{}, auth.ErrDenied
	}
	base := d.basis.StorageValue()
	access, e := d.owner.state().launch.CurrentSessionAccess(ctx, d.caller, base.Scope.WorkspaceID, base.Scope.RoomID, mutation)
	if e != nil {
		return launch.SessionAccess{}, SeatPolicy{}, SafeError(e)
	}
	if !sameAccess(base, access.StorageValue()) {
		return launch.SessionAccess{}, SeatPolicy{}, auth.ErrDenied
	}
	policy, e := d.owner.state().policies.Current(ctx, access)
	if e != nil {
		return launch.SessionAccess{}, SeatPolicy{}, SafeError(e)
	}
	policy, e = ownPolicy(policy)
	return access, policy, e
}
func (s *Service) Connect(ctx context.Context, caller launch.Caller, w, r string, after uint64) (*Connection, error) {
	return s.connect(ctx, caller, w, r, after, false)
}
func (s *Service) connect(ctx context.Context, caller launch.Caller, w, r string, after uint64, polling bool) (*Connection, error) {
	if s.state() == nil || ctx == nil || ctx.Err() != nil || after >= math.MaxInt64 {
		return nil, auth.ErrInvalid
	}
	access, e := s.state().launch.AdmitSession(ctx, caller, w, r)
	if e != nil {
		return nil, SafeError(e)
	}
	d := &connectionData{owner: s, caller: caller, basis: access}
	c := &Connection{data: &d}
	current := func(ctx context.Context) (command.NativeSeat, error) {
		a, p, e := c.current(ctx, false)
		if e != nil {
			return command.NativeSeat{}, e
		}
		v := a.StorageValue()
		if !v.Host {
			delete(p.Commands, "end")
		}
		return command.NativeSeat{Binding: v.Binding, Principal: v.Principal, Seat: v.Seat, Commands: p.Commands, Views: p.View, Inputs: p.Inputs, RecoveryPoint: v.Host && p.RecoveryPoint}, nil
	}
	if polling {
		d.transport, e = s.state().launch.ConnectSessionPolling(ctx, access, current)
	} else {
		d.transport, e = s.state().launch.ConnectSession(ctx, access, current)
	}
	if e != nil {
		return nil, SafeError(e)
	}
	if e = d.transport.Reconnect(ctx, after); e != nil {
		c.Close()
		return nil, SafeError(e)
	}
	return c, nil
}
func (s *Service) ConnectPolling(ctx context.Context, caller launch.Caller, w, r string, after uint64) (*Connection, error) {
	return s.connect(ctx, caller, w, r, after, true)
}
func (c *Connection) Close() {
	if c.state() == nil || c.state().closed.Swap(true) {
		return
	}
	if c.state().transport != nil {
		c.state().transport.Close()
	}
}
func (c *Connection) Submit(ctx context.Context, e command.Envelope) (Result, error) {
	if _, _, err := c.current(ctx, true); err != nil {
		return Result{}, err
	}
	r, err := c.state().transport.Submit(ctx, e)
	if err != nil {
		return Result{}, SafeError(err)
	}
	_, p, err := c.current(ctx, false)
	if err != nil {
		return Result{}, err
	}
	return auth.RoomSecret(ResultData{CommandID: e.CommandID, Version: r.Version, Cursor: r.Cursor, Replayed: r.Replayed, Result: realtime.FilterResult(r.Result, p.View)}), nil
}
func (c *Connection) Reconnect(ctx context.Context, after uint64) (Frame, error) {
	if after >= math.MaxInt64 {
		return Frame{}, auth.ErrInvalid
	}
	if _, _, e := c.current(ctx, false); e != nil {
		return Frame{}, e
	}
	f, e := c.state().transport.Snapshot(ctx, after)
	if e != nil {
		return Frame{}, SafeError(e)
	}
	_, p, e := c.current(ctx, false)
	if e != nil {
		return Frame{}, e
	}
	f.View = realtime.Filter(f.View, p.View.ViewFields)
	f.Events = realtime.FilterEvents(f.Events, p.View)
	return auth.RoomSecret(f), nil
}
func (c *Connection) Next(ctx context.Context) (Frame, error) {
	if c.state() == nil || c.state().closed.Load() {
		return Frame{}, auth.ErrDenied
	}
	f, e := c.state().transport.Next(ctx)
	if e != nil {
		return Frame{}, SafeError(e)
	}
	_, p, e := c.current(ctx, false)
	if e != nil {
		return Frame{}, e
	}
	f.View = realtime.Filter(f.View, p.View.ViewFields)
	f.Events = realtime.FilterEvents(f.Events, p.View)
	return auth.RoomSecret(f), nil
}
func allowedExport(a launch.SessionAccessData, p SeatPolicy, k ExportKind) (command.ViewPolicy, error) {
	v, ok := p.Exports[k]
	if !ok || (k == Host && !a.Host) || (k == Administrator && !a.Administrator) {
		return command.ViewPolicy{}, auth.ErrDenied
	}
	return v, nil
}
func (c *Connection) Export(ctx context.Context, kind ExportKind, after uint64, limit int) (Export, error) {
	if after >= math.MaxInt64 || limit < 1 || limit > 128 {
		return Export{}, auth.ErrInvalid
	}
	access, p, e := c.current(ctx, false)
	if e != nil {
		return Export{}, e
	}
	a := access.StorageValue()
	if _, e = allowedExport(a, p, kind); e != nil {
		return Export{}, e
	}
	page, e := c.state().owner.state().storage.ReadPage(ctx, a.Scope, a.Binding, after, limit)
	if e != nil {
		return Export{}, SafeError(e)
	}
	v := page.StorageValue()
	if v.Binding != a.Binding || v.Version == 0 || v.Cursor < after || len(v.Events) > limit {
		return Export{}, auth.ErrDenied
	}
	view := checkpoint.Object(map[string]checkpoint.Value{})
	if !v.Ended {
		f, e := c.state().transport.Snapshot(ctx, v.Cursor)
		if e != nil {
			return Export{}, SafeError(e)
		}
		if f.Version != v.Version || f.Cursor != v.Cursor {
			return Export{}, auth.ErrConflict
		}
		view = f.View
	}
	access, p, e = c.current(ctx, false)
	if e != nil {
		return Export{}, e
	}
	filter, e := allowedExport(access.StorageValue(), p, kind)
	if e != nil {
		return Export{}, e
	}
	return auth.RoomSecret(ExportData{FormatVersion: 1, Kind: kind, Scope: a.Scope, Binding: a.Binding, Seat: a.Seat, Version: v.Version, Cursor: v.Cursor, NextCursor: v.NextCursor, More: v.More, Ended: v.Ended, View: realtime.Filter(view, filter.ViewFields), Events: realtime.FilterEvents(v.Events, filter)}), nil
}
func (c *Connection) CreateRecoveryPoint(ctx context.Context) (Point, error) {
	access, p, e := c.current(ctx, true)
	if e != nil {
		return Point{}, e
	}
	a := access.StorageValue()
	if !a.Host || !p.RecoveryPoint {
		return Point{}, auth.ErrDenied
	}
	created, e := c.state().transport.CreateRecoveryPoint(ctx)
	if e != nil {
		return Point{}, SafeError(e)
	}
	point, e := c.state().owner.state().storage.ReadPoint(ctx, a.Scope, a.Binding)
	if e != nil {
		return Point{}, SafeError(e)
	}
	v := point.StorageValue()
	if v.Binding != a.Binding || v.Scope != a.Scope || v.Version != created.Version || v.Cursor != created.Cursor {
		return Point{}, auth.ErrConflict
	}
	access, p, e = c.current(ctx, false)
	if e != nil {
		return Point{}, e
	}
	if !access.StorageValue().Host || !p.RecoveryPoint {
		return Point{}, auth.ErrDenied
	}
	return point, nil
}
func (c *Connection) RecoveryPoint(ctx context.Context) (Point, error) {
	access, _, e := c.current(ctx, false)
	if e != nil {
		return Point{}, e
	}
	a := access.StorageValue()
	point, e := c.state().owner.state().storage.ReadPoint(ctx, a.Scope, a.Binding)
	if e != nil {
		return Point{}, SafeError(e)
	}
	if v := point.StorageValue(); v.Scope != a.Scope || v.Binding != a.Binding {
		return Point{}, auth.ErrDenied
	}
	if _, _, e = c.current(ctx, false); e != nil {
		return Point{}, e
	}
	return point, nil
}
