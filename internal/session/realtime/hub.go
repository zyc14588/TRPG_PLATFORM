// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package realtime owns bounded delivery queues. Raw state and events are
// filtered before enqueue; identities are revalidated again before dequeue.
package realtime

import (
	"context"
	"encoding/json"
	"errors"
	"sync"
	"sync/atomic"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/outbox"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

var ErrDisconnected = errors.New("SESSION_CONNECTION_CLOSED")
var ErrCapacity = errors.New("SESSION_CONNECTION_BACKPRESSURE")

type Frame struct {
	Kind    string              `json:"kind"`
	Session string              `json:"session_id"`
	Version uint64              `json:"state_version"`
	Cursor  uint64              `json:"event_cursor"`
	View    checkpoint.Value    `json:"view"`
	Events  []data.JournalEvent `json:"events,omitempty"`
}
type Options struct {
	Authority                   *command.Authority
	Capacity, PerSession, Queue int
}
type Hub struct {
	mu          sync.Mutex
	options     Options
	next        uint64
	connections map[uint64]*Connection
	closed      bool
}
type Connection struct {
	hub      *Hub
	id       uint64
	identity command.Identity
	queue    chan Frame
	closed   atomic.Bool
}

func New(o Options) (*Hub, error) {
	if o.Authority == nil || o.Capacity < 1 || o.Capacity > 128 || o.PerSession < 1 || o.PerSession > 32 || o.Queue < 1 || o.Queue > 64 {
		return nil, ErrCapacity
	}
	return &Hub{options: o, connections: map[uint64]*Connection{}}, nil
}
func (h *Hub) Subscribe(i command.Identity) (*Connection, error) {
	return h.SubscribeContext(context.Background(), i)
}
func (h *Hub) SubscribeContext(ctx context.Context, i command.Identity) (*Connection, error) {
	if h.options.Authority.VerifyContext(ctx, i) != nil {
		return nil, command.ErrDenied
	}
	h.mu.Lock()
	defer h.mu.Unlock()
	if h.closed {
		return nil, ErrDisconnected
	}
	count := 0
	for _, c := range h.connections {
		if c.identity.Binding() == i.Binding() {
			count++
		}
	}
	if len(h.connections) >= h.options.Capacity || count >= h.options.PerSession {
		return nil, ErrCapacity
	}
	h.next++
	c := &Connection{hub: h, id: h.next, identity: i, queue: make(chan Frame, h.options.Queue)}
	h.connections[c.id] = c
	return c, nil
}
func (h *Hub) drop(c *Connection) {
	if _, ok := h.connections[c.id]; ok {
		c.closed.Store(true)
		delete(h.connections, c.id)
		close(c.queue)
		_ = h.options.Authority.ReleaseNative(c.identity)
	}
}
func (c *Connection) Close()                     { c.hub.mu.Lock(); defer c.hub.mu.Unlock(); c.hub.drop(c) }
func (c *Connection) Identity() command.Identity { return c.identity }
func (c *Connection) Next(ctx context.Context) (Frame, error) {
	if ctx == nil || c.closed.Load() {
		return Frame{}, ErrDisconnected
	}
	if err := ctx.Err(); err != nil {
		return Frame{}, err
	}
	if c.hub.options.Authority.VerifyContext(ctx, c.identity) != nil {
		// Canceling this poll does not revoke its native credential. No frame
		// is delivered; a later poll still checks the current authority.
		if err := ctx.Err(); err != nil {
			return Frame{}, err
		}
		c.Close()
		return Frame{}, command.ErrDenied
	}
	select {
	case <-ctx.Done():
		return Frame{}, ctx.Err()
	case f, ok := <-c.queue:
		if !ok || c.closed.Load() {
			return Frame{}, ErrDisconnected
		}
		p, err := c.hub.options.Authority.PolicyContext(ctx, c.identity)
		if canceled := ctx.Err(); canceled != nil {
			return Frame{}, canceled
		}
		if err != nil {
			c.Close()
			return Frame{}, command.ErrDenied
		}
		f.View = Filter(f.View, p.ViewFields)
		f.Events = FilterEvents(f.Events, p)
		return f, nil
	}
}
func (h *Hub) Close() {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.closed = true
	for _, c := range h.connections {
		h.drop(c)
	}
}
func (h *Hub) Connected(i command.Identity) bool {
	if h.options.Authority.Verify(i) != nil {
		return false
	}
	h.mu.Lock()
	defer h.mu.Unlock()
	for _, c := range h.connections {
		if c.identity.Binding() == i.Binding() && c.identity.Principal() == i.Principal() && c.identity.Seat() == i.Seat() {
			return true
		}
	}
	return false
}

// Leaf allowlists are sufficient for the minimal fixture. Unreviewed nested
// objects/arrays are omitted rather than granting all their hidden children.
func Filter(v checkpoint.Value, fields []string) checkpoint.Value {
	r := checkpoint.Value{Kind: "table", Table: map[string]checkpoint.Value{}}
	if v.Kind != "table" || checkpoint.Validate(v) != nil {
		return r
	}
	for _, name := range fields {
		if x, ok := v.Table[name]; ok && x.Kind != "table" && x.Kind != "array" {
			r.Table[name] = x
		}
	}
	return r
}
func FilterResult(v checkpoint.Value, p command.ViewPolicy) checkpoint.Value {
	if v.Kind == "table" {
		return Filter(v, p.ResultFields)
	}
	if p.ScalarResult && v.Kind != "array" && checkpoint.Validate(v) == nil {
		return v
	}
	return checkpoint.Value{Kind: "nil"}
}
func FilterEvents(events []data.JournalEvent, p command.ViewPolicy) []data.JournalEvent {
	filtered := []data.JournalEvent{}
	for _, e := range events {
		fields, ok := p.EventFields[e.Event.Type]
		if !ok {
			continue
		}
		e.Event.Payload = Filter(e.Event.Payload, fields)
		filtered = append(filtered, e)
	}
	return filtered
}
func (h *Hub) Enqueue(c *Connection, f Frame) error {
	return h.EnqueueContext(context.Background(), c, f)
}
func (h *Hub) EnqueueContext(ctx context.Context, c *Connection, f Frame) error {
	if c == nil || c.hub != h || f.Session != c.identity.Binding().Session || len(f.Events) > 128 || checkpoint.Validate(f.View) != nil {
		return ErrDisconnected
	}
	p, err := h.options.Authority.PolicyContext(ctx, c.identity)
	if err != nil {
		c.Close()
		return err
	}
	f.View = Filter(f.View, p.ViewFields)
	f.Events = FilterEvents(f.Events, p)
	raw, err := json.Marshal(f)
	if err != nil || len(raw) > 256<<10 {
		c.Close()
		return ErrCapacity
	}
	var owned Frame
	if json.Unmarshal(raw, &owned) != nil {
		return ErrCapacity
	}
	h.mu.Lock()
	defer h.mu.Unlock()
	if c.closed.Load() || h.closed {
		return ErrDisconnected
	}
	select {
	case c.queue <- owned:
		return nil
	default:
		h.drop(c)
		return ErrCapacity
	}
}
func (h *Hub) Publish(ctx context.Context, n outbox.Notification, project func(context.Context, command.Identity) (checkpoint.Value, error)) error {
	h.mu.Lock()
	list := []*Connection{}
	for _, c := range h.connections {
		if c.identity.Binding() == n.Binding {
			list = append(list, c)
		}
	}
	h.mu.Unlock()
	var combined error
	for _, c := range list {
		if err := h.options.Authority.VerifyContext(ctx, c.identity); err != nil {
			c.Close()
			continue
		}
		view, err := project(ctx, c.identity)
		if err != nil {
			c.Close()
			combined = errors.Join(combined, err)
			continue
		}
		if err = h.EnqueueContext(ctx, c, Frame{Kind: "committed", Session: n.Binding.Session, Version: n.Version, Cursor: n.Cursor, View: view, Events: n.Events}); err != nil {
			combined = errors.Join(combined, err)
		}
	}
	return combined
}
func (h *Hub) CloseSession(b data.Binding) {
	h.mu.Lock()
	defer h.mu.Unlock()
	for _, c := range h.connections {
		if c.identity.Binding() == b {
			h.drop(c)
		}
	}
}
