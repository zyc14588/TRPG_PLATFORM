// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package realtime

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"strings"
	"sync/atomic"
	"testing"
)

func TestQueuedNativeFrameUsesNarrowedCurrentViewAndEventPolicy(t *testing.T) {
	b := data.Binding{Workspace: "w", Session: "s", GraphHash: "sha256:" + strings.Repeat("a", 64)}
	a, e := command.NewFixtureAuthority([]command.FixtureSeat{{Credential: "bootstrap-0123456789abcdef", Binding: b, Principal: "system", Seat: "system"}})
	if e != nil {
		t.Fatal("authority")
	}
	var private atomic.Bool
	private.Store(true)
	resolve := func(context.Context) (command.NativeSeat, error) {
		fields := []string{"counter"}
		if private.Load() {
			fields = append(fields, "secret")
		}
		return command.NativeSeat{Binding: b, Principal: "player", Seat: "player", Views: command.ViewPolicy{ViewFields: fields, EventFields: map[string][]string{"change": fields}}}, nil
	}
	i, e := a.IssueNative(context.Background(), resolve)
	if e != nil {
		t.Fatal("issue")
	}
	h, e := New(Options{Authority: a, Capacity: 1, PerSession: 1, Queue: 2})
	if e != nil {
		t.Fatal("hub")
	}
	defer h.Close()
	c, e := h.Subscribe(i)
	if e != nil {
		t.Fatal("subscribe")
	}
	view := checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(1), "secret": checkpoint.Text("queued-private-marker")})
	f := Frame{Kind: "committed", Session: "s", Version: 2, Cursor: 1, View: view, Events: []data.JournalEvent{{Sequence: 1, Event: data.Event{Type: "change", Payload: view}}}}
	if e = h.Enqueue(c, f); e != nil {
		t.Fatal("enqueue")
	}
	private.Store(false)
	got, e := c.Next(context.Background())
	if e != nil {
		t.Fatal("dequeue")
	}
	if _, ok := got.View.Table["secret"]; ok {
		t.Fatal("stale queued private view")
	}
	if len(got.Events) != 1 {
		t.Fatal("authorized event missing")
	}
	if _, ok := got.Events[0].Event.Payload.Table["secret"]; ok {
		t.Fatal("stale queued private event")
	}
	c.Close()
	if a.Verify(i) != command.ErrDenied {
		t.Fatal("disconnect retained native identity")
	}
	replacement, e := a.IssueNative(context.Background(), resolve)
	if e != nil {
		t.Fatal("native capacity not reclaimed")
	}
	if _, e = h.Subscribe(replacement); e != nil {
		t.Fatal("hub capacity not reclaimed")
	}
}

func TestQueuedNestedViewUsesCurrentChildAndElementGrants(t *testing.T) {
	b := data.Binding{Workspace: "w", Session: "s", GraphHash: "sha256:" + strings.Repeat("a", 64)}
	a, err := command.NewFixtureAuthority([]command.FixtureSeat{{Credential: "bootstrap-0123456789abcdef", Binding: b, Principal: "system", Seat: "system"}})
	if err != nil {
		t.Fatal(err)
	}
	var phase atomic.Int32
	resolve := func(context.Context) (command.NativeSeat, error) {
		current := phase.Load()
		if current == 3 {
			return command.NativeSeat{}, command.ErrDenied
		}
		fields := map[string]*command.ViewSelector{"visible": {Kind: "scalar"}}
		if current != 1 {
			fields["private"] = &command.ViewSelector{Kind: "scalar"}
		}
		if current == 2 {
			fields["late"] = &command.ViewSelector{Kind: "scalar"}
		}
		element := &command.ViewSelector{Kind: "table", Fields: fields}
		return command.NativeSeat{Binding: b, Principal: "player", Seat: "player", Views: command.ViewPolicy{ViewProjection: &command.ViewSelector{Kind: "table", Fields: map[string]*command.ViewSelector{
			"record": element, "list": {Kind: "array", Element: element},
		}}}}, nil
	}
	i, err := a.IssueNative(context.Background(), resolve)
	if err != nil {
		t.Fatal(err)
	}
	h, err := New(Options{Authority: a, Capacity: 1, PerSession: 1, Queue: 2})
	if err != nil {
		t.Fatal(err)
	}
	defer h.Close()
	c, err := h.Subscribe(i)
	if err != nil {
		t.Fatal(err)
	}
	item := checkpoint.Object(map[string]checkpoint.Value{"visible": checkpoint.Int(1), "private": checkpoint.Text("queued-private"), "late": checkpoint.Text("not-queued")})
	f := Frame{Kind: "committed", Session: "s", Version: 2, Cursor: 1, View: checkpoint.Object(map[string]checkpoint.Value{"record": item, "list": checkpoint.Array(item, item), "other-seat": checkpoint.Text("root-secret")})}
	if err = h.Enqueue(c, f); err != nil {
		t.Fatal(err)
	}
	phase.Store(1)
	got, err := c.Next(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	check := func(f Frame) {
		t.Helper()
		if len(f.View.Table) != 2 || len(f.View.Table["list"].Array) != 2 {
			t.Fatal("nested containers or array order lost", f)
		}
		for _, v := range []checkpoint.Value{f.View.Table["record"], f.View.Table["list"].Array[0], f.View.Table["list"].Array[1]} {
			if len(v.Table) != 1 || v.Table["visible"].Number != "1" {
				t.Fatal("stale or unqueued child delivered", v)
			}
		}
	}
	check(got)
	if err = h.Enqueue(c, f); err != nil {
		t.Fatal(err)
	}
	item.Table["visible"] = checkpoint.Int(9)
	phase.Store(2)
	got, err = c.Next(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	check(got)
	if err = h.Enqueue(c, f); err != nil {
		t.Fatal(err)
	}
	phase.Store(3)
	if _, err = c.Next(context.Background()); err != command.ErrDenied {
		t.Fatal("revoked nested queued view delivered", err)
	}
	if a.Verify(i) != command.ErrDenied {
		t.Fatal("revoked queued identity remained available")
	}
}

func TestNestedViewCannotExceedFinalFrameSerializationLimit(t *testing.T) {
	b := data.Binding{Workspace: "w", Session: "s", GraphHash: "sha256:" + strings.Repeat("a", 64)}
	p := command.ViewPolicy{ViewProjection: &command.ViewSelector{Kind: "table", Fields: map[string]*command.ViewSelector{"record": {Kind: "table", Fields: map[string]*command.ViewSelector{"text": {Kind: "scalar"}}}}}}
	a, err := command.NewFixtureAuthority([]command.FixtureSeat{{Credential: "fixture-0123456789abcdef", Binding: b, Principal: "player", Seat: "player", Views: p}})
	if err != nil {
		t.Fatal(err)
	}
	i, err := a.Authenticate("fixture-0123456789abcdef", "s", "player")
	if err != nil {
		t.Fatal(err)
	}
	h, err := New(Options{Authority: a, Capacity: 1, PerSession: 1, Queue: 1})
	if err != nil {
		t.Fatal(err)
	}
	defer h.Close()
	c, err := h.Subscribe(i)
	if err != nil {
		t.Fatal(err)
	}
	// JSON escapes each '<' into six bytes, beyond the unchanged frame budget.
	view := checkpoint.Object(map[string]checkpoint.Value{"record": checkpoint.Object(map[string]checkpoint.Value{"text": checkpoint.Text(strings.Repeat("<", 50000))})})
	if checkpoint.Validate(view) != nil {
		t.Fatal("fixture exceeded the checkpoint budget")
	}
	if err = h.Enqueue(c, Frame{Session: "s", View: view}); err != ErrCapacity {
		t.Fatal("oversize final serialized frame accepted", err)
	}
	if _, err = c.Next(context.Background()); err != ErrDisconnected {
		t.Fatal("oversize frame delivered or connection retained", err)
	}
}
