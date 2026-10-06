// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package actor

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/realtime"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

// This bounded in-memory backend is only a unit double, never service evidence.
type memory struct {
	mu               sync.Mutex
	sessions         map[string]*memorySession
	entered, release chan struct{}
	once             sync.Once
	checkpoint       chan struct{}
}
type memorySession struct {
	version, cursor                                                  uint64
	receipts                                                         map[string]data.Receipt
	envelopes                                                        map[string]command.Envelope
	events                                                           []data.JournalEvent
	calls, open, active, maxActive, writers, maxWriters, checkpoints int
}
type memoryEngine struct {
	store   *memory
	binding data.Binding
	closed  bool
}

func (m *memory) session(b data.Binding) *memorySession {
	key := key(b)
	s := m.sessions[key]
	if s == nil {
		s = &memorySession{version: 1, receipts: map[string]data.Receipt{}, envelopes: map[string]command.Envelope{}}
		m.sessions[key] = s
	}
	return s
}
func (m *memory) Open(ctx context.Context, b data.Binding) (Engine, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	s := m.session(b)
	s.open++
	s.active++
	if s.active > s.maxActive {
		s.maxActive = s.active
	}
	return &memoryEngine{store: m, binding: b}, nil
}
func (m *memory) Resolve(ctx context.Context, i command.Identity, e command.Envelope) (data.Receipt, bool, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	s := m.session(i.Binding())
	r, ok := s.receipts[e.CommandID]
	if !ok {
		return data.Receipt{}, false, nil
	}
	one, _ := json.Marshal(e)
	two, _ := json.Marshal(s.envelopes[e.CommandID])
	if string(one) != string(two) || r.Header.Principal != i.Principal() {
		return data.Receipt{}, false, data.ErrConflict
	}
	r.Replayed = true
	return r, true, nil
}
func (m *memory) Journal(ctx context.Context, b data.Binding, after uint64, limit int) (data.JournalPage, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	s := m.session(b)
	p := data.JournalPage{Binding: b, Version: s.version, Cursor: s.cursor}
	if after > s.cursor {
		return p, data.ErrConflict
	}
	for _, e := range s.events {
		if e.Sequence > after {
			p.Events = append(p.Events, e)
		}
	}
	return p, nil
}
func (e *memoryEngine) Version() uint64 {
	e.store.mu.Lock()
	defer e.store.mu.Unlock()
	return e.store.session(e.binding).version
}
func (e *memoryEngine) Cursor() uint64 {
	e.store.mu.Lock()
	defer e.store.mu.Unlock()
	return e.store.session(e.binding).cursor
}
func (e *memoryEngine) Execute(ctx context.Context, i command.Identity, c command.Envelope) (data.Receipt, error) {
	m := e.store
	m.mu.Lock()
	s := m.session(e.binding)
	s.calls++
	s.writers++
	if s.writers > s.maxWriters {
		s.maxWriters = s.writers
	}
	m.mu.Unlock()
	defer func() { m.mu.Lock(); s.writers--; m.mu.Unlock() }()
	if c.Type == "panic" {
		panic("isolated unit failure")
	}
	if c.Type == "block" {
		m.once.Do(func() { close(m.entered) })
		select {
		case <-m.release:
		case <-ctx.Done():
			return data.Receipt{}, ctx.Err()
		}
	}
	m.mu.Lock()
	defer m.mu.Unlock()
	if s.version != c.ExpectedStateVersion {
		return data.Receipt{}, data.ErrConflict
	}
	if ctx.Err() != nil {
		return data.Receipt{}, ctx.Err()
	}
	s.version++
	s.cursor++
	event := data.Event{ID: "event-" + c.CommandID, Type: "fixture/change", Payload: checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(int64(s.version)), "secret": checkpoint.Text("private-unit-value")})}
	r := data.Receipt{Header: data.Header{Binding: e.binding, Principal: i.Principal(), CommandID: c.CommandID, ExpectedVersion: c.ExpectedStateVersion, Fingerprint: checkpoint.Hash([]byte(c.CommandID))}, Version: s.version, Cursor: s.cursor, Result: checkpoint.Int(int64(s.version)), Events: []data.Event{event}}
	s.receipts[c.CommandID] = r
	s.envelopes[c.CommandID] = c
	s.events = append(s.events, data.JournalEvent{Sequence: s.cursor, Version: s.version, CommandID: c.CommandID, Event: event})
	return r, nil
}
func (e *memoryEngine) Project(ctx context.Context, i command.Identity) (checkpoint.Value, error) {
	e.store.mu.Lock()
	defer e.store.mu.Unlock()
	return checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(int64(e.store.session(e.binding).version)), "secret": checkpoint.Text("private-unit-value")}), nil
}
func (e *memoryEngine) Checkpoint(ctx context.Context) error {
	e.store.mu.Lock()
	defer e.store.mu.Unlock()
	e.store.session(e.binding).checkpoints++
	select {
	case e.store.checkpoint <- struct{}{}:
	default:
	}
	return nil
}
func (e *memoryEngine) Close() error {
	e.store.mu.Lock()
	defer e.store.mu.Unlock()
	if !e.closed {
		e.store.session(e.binding).active--
		e.closed = true
	}
	return nil
}

type rig struct {
	registry   *Registry
	store      *memory
	authority  *command.Authority
	hub        *realtime.Hub
	identities map[string]command.Identity
}

func newRig(t *testing.T, mailbox int, idle time.Duration) *rig {
	t.Helper()
	seats := []command.FixtureSeat{}
	for _, sid := range []string{"one", "two"} {
		for _, seat := range []string{"gm", "player"} {
			fields := []string{"counter"}
			if seat == "gm" {
				fields = append(fields, "secret")
			}
			types := map[string]func(checkpoint.Value) error{}
			for _, name := range []string{"increment", "block", "panic", "end"} {
				types[name] = func(v checkpoint.Value) error {
					if v.Kind != "integer" {
						return command.ErrEnvelope
					}
					return nil
				}
			}
			seats = append(seats, command.FixtureSeat{Credential: "fixture-" + sid + "-" + seat + "-0123456789abcdef", Binding: data.Binding{Workspace: "workspace", Session: sid, GraphHash: "sha256:" + strings.Repeat("a", 64)}, Principal: seat, Seat: seat, Commands: types, Views: command.ViewPolicy{ViewFields: fields, EventFields: map[string][]string{"fixture/change": fields}, ScalarResult: true}})
		}
	}
	a, err := command.NewFixtureAuthority(seats)
	if err != nil {
		t.Fatal(err)
	}
	hub, err := realtime.New(realtime.Options{Authority: a, Capacity: 8, PerSession: 4, Queue: 8})
	if err != nil {
		t.Fatal(err)
	}
	m := &memory{sessions: map[string]*memorySession{}, entered: make(chan struct{}), release: make(chan struct{}), checkpoint: make(chan struct{}, 16)}
	r, err := New(context.Background(), Options{Authority: a, Hub: hub, Backend: m, MaxSessions: 2, Mailbox: mailbox, Idle: idle, CommandTimeout: time.Second})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = r.Close() })
	identities := map[string]command.Identity{}
	for _, s := range seats {
		i, err := a.Authenticate(s.Credential, s.Binding.Session, s.Seat)
		if err != nil {
			t.Fatal(err)
		}
		identities[s.Binding.Session+"-"+s.Seat] = i
	}
	return &rig{r, m, a, hub, identities}
}
func envelope(sid, id, typ string, version uint64) command.Envelope {
	return command.Envelope{CommandID: id, SessionID: sid, SeatID: "gm", Type: typ, Payload: checkpoint.Int(1), ExpectedStateVersion: version, CorrelationID: "correlation-" + id}
}
func waitQueue(t *testing.T, r *Registry, i command.Identity, n int) {
	t.Helper()
	deadline := time.NewTimer(time.Second)
	defer deadline.Stop()
	ticker := time.NewTicker(time.Millisecond)
	defer ticker.Stop()
	for {
		r.mu.Lock()
		e := r.entries[key(i.Binding())]
		ready := e != nil && len(e.queue) == n
		r.mu.Unlock()
		if ready {
			return
		}
		select {
		case <-deadline.C:
			t.Fatal("submission did not reach bounded mailbox")
		case <-ticker.C:
		}
	}
}
func TestBoundedMailboxSerializesWrites(t *testing.T) {
	rig := newRig(t, 1, time.Hour)
	i := rig.identities["one-gm"]
	answers := make(chan error, 2)
	go func() {
		_, err := rig.registry.Submit(context.Background(), i, envelope("one", "first", "block", 1))
		answers <- err
	}()
	<-rig.store.entered
	go func() {
		_, err := rig.registry.Submit(context.Background(), i, envelope("one", "second", "increment", 2))
		answers <- err
	}()
	waitQueue(t, rig.registry, i, 1)
	if _, err := rig.registry.Submit(context.Background(), i, envelope("one", "third", "increment", 3)); !errors.Is(err, ErrBackpressure) {
		t.Fatal(err)
	}
	close(rig.store.release)
	for range 2 {
		if err := <-answers; err != nil {
			t.Fatal(err)
		}
	}
	rig.store.mu.Lock()
	defer rig.store.mu.Unlock()
	s := rig.store.session(i.Binding())
	if s.version != 3 || s.calls != 2 || s.maxWriters != 1 || s.maxActive != 1 {
		t.Fatalf("single writer/backpressure violated: %+v", s)
	}
}
func TestCommitPrecedesFilteredBroadcastAndDuplicateReturnsOriginal(t *testing.T) {
	rig := newRig(t, 4, time.Hour)
	gm := rig.identities["one-gm"]
	player := rig.identities["one-player"]
	g, err := rig.hub.Subscribe(gm)
	if err != nil {
		t.Fatal(err)
	}
	p, err := rig.hub.Subscribe(player)
	if err != nil {
		t.Fatal(err)
	}
	c := envelope("one", "commit", "block", 1)
	answer := make(chan data.Receipt, 1)
	fail := make(chan error, 1)
	go func() { r, err := rig.registry.Submit(context.Background(), gm, c); answer <- r; fail <- err }()
	<-rig.store.entered
	rig.store.mu.Lock()
	before := rig.store.session(gm.Binding()).version
	rig.store.mu.Unlock()
	if before != 1 {
		t.Fatal("memory advanced before commit")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 50*time.Millisecond)
	if _, err = p.Next(ctx); !errors.Is(err, context.DeadlineExceeded) {
		t.Fatal("precommit frame exposed", err)
	}
	cancel()
	close(rig.store.release)
	receipt := <-answer
	if err := <-fail; err != nil {
		t.Fatal(err)
	}
	if receipt.Version != 2 {
		t.Fatal(receipt)
	}
	for _, conn := range []*realtime.Connection{g, p} {
		ctx, cancel := context.WithTimeout(context.Background(), time.Second)
		frame, err := conn.Next(ctx)
		cancel()
		if err != nil {
			t.Fatal(err)
		}
		raw, _ := json.Marshal(frame)
		has := strings.Contains(string(raw), "private-unit-value")
		if has != (conn.Identity().Seat() == "gm") {
			t.Fatal("seat privacy violation", string(raw))
		}
	}
	again, err := rig.registry.Submit(context.Background(), gm, c)
	if err != nil || !again.Replayed || again.Version != receipt.Version || again.Result.Number != receipt.Result.Number {
		t.Fatal(again, err)
	}
	ctx, cancel = context.WithTimeout(context.Background(), 50*time.Millisecond)
	if _, err = p.Next(ctx); !errors.Is(err, context.DeadlineExceeded) {
		t.Fatal("duplicate broadcast", err)
	}
	cancel()
	rig.store.mu.Lock()
	defer rig.store.mu.Unlock()
	s := rig.store.session(gm.Binding())
	if s.calls != 1 || s.cursor != 1 || len(s.receipts) != 1 {
		t.Fatal("duplicate executed", s.calls, s.cursor)
	}
}
func TestPanicAndCancellationDoNotAffectOtherSessions(t *testing.T) {
	rig := newRig(t, 4, time.Hour)
	one := rig.identities["one-gm"]
	two := rig.identities["two-gm"]
	if _, err := rig.registry.Submit(context.Background(), one, envelope("one", "panic", "panic", 1)); !errors.Is(err, ErrPanic) {
		t.Fatal(err)
	}
	if _, err := rig.registry.Submit(context.Background(), two, envelope("two", "other", "increment", 1)); err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	done := make(chan error, 1)
	go func() { _, err := rig.registry.Submit(ctx, one, envelope("one", "cancel", "block", 1)); done <- err }()
	<-rig.store.entered
	cancel()
	if err := <-done; !errors.Is(err, context.Canceled) {
		t.Fatal(err)
	}
	if _, err := rig.registry.Submit(context.Background(), one, envelope("one", "recovered", "increment", 1)); err != nil {
		t.Fatal(err)
	}
	rig.store.mu.Lock()
	defer rig.store.mu.Unlock()
	for _, id := range []command.Identity{one, two} {
		s := rig.store.session(id.Binding())
		if s.version != 2 || s.cursor != 1 || s.maxActive != 1 || s.maxWriters != 1 {
			t.Fatal("failure escaped Session", s.version, s.maxActive, s.maxWriters)
		}
	}
}
func TestSleepReactivateReconnectAndRevocation(t *testing.T) {
	rig := newRig(t, 4, time.Hour)
	i := rig.identities["one-gm"]
	c := envelope("one", "first", "increment", 1)
	if _, err := rig.registry.Submit(context.Background(), i, c); err != nil {
		t.Fatal(err)
	}
	if err := rig.registry.Sleep(context.Background(), i); err != nil {
		t.Fatal(err)
	}
	if _, err := rig.registry.Submit(context.Background(), i, c); err != nil {
		t.Fatal(err)
	}
	rig.store.mu.Lock()
	open := rig.store.session(i.Binding()).open
	rig.store.mu.Unlock()
	if open != 1 {
		t.Fatal("duplicate restarted Lua")
	}
	c.CorrelationID = "substituted"
	if _, err := rig.registry.Submit(context.Background(), i, c); !errors.Is(err, data.ErrConflict) {
		t.Fatal(err)
	}
	if _, err := rig.registry.Submit(context.Background(), i, envelope("one", "second", "increment", 2)); err != nil {
		t.Fatal(err)
	}
	player := rig.identities["one-player"]
	frame, err := rig.registry.Reconnect(context.Background(), player, 1, nil)
	if err != nil || frame.Version != 3 || frame.Cursor != 2 || len(frame.Events) != 1 {
		t.Fatal(frame, err)
	}
	if _, ok := frame.View.Table["secret"]; ok {
		t.Fatal("reconnect secret")
	}
	if _, err := rig.registry.Reconnect(context.Background(), player, 999, nil); !errors.Is(err, data.ErrConflict) {
		t.Fatal(err)
	}
	if err := rig.authority.Revoke("fixture-one-player-0123456789abcdef"); err != nil {
		t.Fatal(err)
	}
	if _, err := rig.registry.Reconnect(context.Background(), player, 0, nil); !errors.Is(err, command.ErrDenied) {
		t.Fatal(err)
	}
	if err := rig.registry.Close(); err != nil {
		t.Fatal(err)
	}
	rig.store.mu.Lock()
	defer rig.store.mu.Unlock()
	s := rig.store.session(i.Binding())
	if s.open != 2 || s.active != 0 || s.checkpoints != 1 {
		t.Fatal("activation/cleanup", s.open, s.active, s.checkpoints)
	}
}
func TestIdleCreatesCheckpointAndReleasesRuntime(t *testing.T) {
	rig := newRig(t, 4, 20*time.Millisecond)
	i := rig.identities["one-gm"]
	if _, err := rig.registry.Submit(context.Background(), i, envelope("one", "first", "increment", 1)); err != nil {
		t.Fatal(err)
	}
	select {
	case <-rig.store.checkpoint:
	case <-time.After(time.Second):
		t.Fatal("idle checkpoint not captured")
	}
	if _, err := rig.registry.Submit(context.Background(), i, envelope("one", "second", "increment", 2)); err != nil {
		t.Fatal(err)
	}
	if err := rig.registry.Close(); err != nil {
		t.Fatal(err)
	}
	rig.store.mu.Lock()
	defer rig.store.mu.Unlock()
	s := rig.store.session(i.Binding())
	if s.open != 2 || s.active != 0 || s.maxActive != 1 {
		t.Fatal("idle runtime not released", s.open, s.active, s.maxActive)
	}
}
func TestConcurrentSessionsRemainIsolated(t *testing.T) {
	rig := newRig(t, 8, time.Hour)
	var wg sync.WaitGroup
	fail := make(chan error, 2)
	for _, sid := range []string{"one", "two"} {
		wg.Go(func() {
			for n := uint64(1); n <= 12; n++ {
				_, err := rig.registry.Submit(context.Background(), rig.identities[sid+"-gm"], envelope(sid, fmt.Sprintf("command-%d", n), "increment", n))
				if err != nil {
					fail <- err
					return
				}
			}
		})
	}
	wg.Wait()
	close(fail)
	for err := range fail {
		t.Fatal(err)
	}
	rig.store.mu.Lock()
	defer rig.store.mu.Unlock()
	for _, sid := range []string{"one", "two"} {
		s := rig.store.session(rig.identities[sid+"-gm"].Binding())
		if s.version != 13 || s.calls != 12 || s.maxWriters != 1 || s.maxActive != 1 {
			t.Fatal("cross Session interference", sid, s.version, s.calls)
		}
	}
}

func TestIdleReclaimsCapacityBeforeAnotherSessionActivates(t *testing.T) {
	rig := newRig(t, 2, 10*time.Millisecond)
	rig.registry.options.MaxSessions = 1
	one, two := rig.identities["one-gm"], rig.identities["two-gm"]
	if _, err := rig.registry.Submit(context.Background(), one, envelope("one", "first", "increment", 1)); err != nil {
		t.Fatal(err)
	}
	deadline := time.NewTimer(time.Second)
	defer deadline.Stop()
	tick := time.NewTicker(time.Millisecond)
	defer tick.Stop()
	for {
		rig.registry.mu.Lock()
		empty := len(rig.registry.entries) == 0
		rig.registry.mu.Unlock()
		if empty {
			break
		}
		select {
		case <-deadline.C:
			t.Fatal("idle registry capacity retained")
		case <-tick.C:
		}
	}
	if _, err := rig.registry.Submit(context.Background(), two, envelope("two", "second", "increment", 1)); err != nil {
		t.Fatal("idle capacity was not reusable", err)
	}
	rig.store.mu.Lock()
	defer rig.store.mu.Unlock()
	s := rig.store.session(one.Binding())
	if s.active != 0 || s.checkpoints != 1 {
		t.Fatal("idle VM was not closed before capacity reuse", s.active, s.checkpoints)
	}
}
