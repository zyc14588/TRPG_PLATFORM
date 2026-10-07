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
