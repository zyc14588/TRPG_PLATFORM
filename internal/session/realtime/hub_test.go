// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package realtime

import (
	"context"
	"encoding/json"
	"errors"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

func TestQueuesFilterBeforeCopyRevalidateOnDeliveryAndCloseSlowConsumers(t *testing.T) {
	b := data.Binding{Workspace: "workspace", Session: "session", GraphHash: "sha256:" + strings.Repeat("a", 64)}
	token := "synthetic-player-0123456789abcdef"
	a, err := command.NewFixtureAuthority([]command.FixtureSeat{{Credential: token, Binding: b, Principal: "player", Seat: "player", Views: command.ViewPolicy{ViewFields: []string{"counter", "nested"}, EventFields: map[string][]string{"change": {"counter", "nested"}}}}})
	if err != nil {
		t.Fatal(err)
	}
	i, err := a.Authenticate(token, "session", "player")
	if err != nil {
		t.Fatal(err)
	}
	h, err := New(Options{Authority: a, Capacity: 2, PerSession: 2, Queue: 1})
	if err != nil {
		t.Fatal(err)
	}
	defer h.Close()
	c, err := h.Subscribe(i)
	if err != nil {
		t.Fatal(err)
	}
	raw := checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(1), "secret": checkpoint.Text("private"), "nested": checkpoint.Object(map[string]checkpoint.Value{"secret": checkpoint.Text("nested-private")})})
	frame := Frame{Kind: "committed", Session: "session", Version: 2, Cursor: 1, View: raw, Events: []data.JournalEvent{{Sequence: 1, Event: data.Event{Type: "change", Payload: raw}}, {Sequence: 2, Event: data.Event{Type: "hidden", Payload: raw}}}}
	if err = h.Enqueue(c, frame); err != nil {
		t.Fatal(err)
	}
	raw.Table["counter"] = checkpoint.Int(99)
	received, err := c.Next(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	bytes, _ := json.Marshal(received)
	if strings.Contains(string(bytes), "private") || len(received.Events) != 1 || received.View.Table["counter"].Number != "1" {
		t.Fatal("filter/copy violation", string(bytes))
	}
	if err = h.Enqueue(c, frame); err != nil {
		t.Fatal(err)
	}
	if err = h.Enqueue(c, frame); !errors.Is(err, ErrCapacity) {
		t.Fatal(err)
	}
	if _, err = c.Next(context.Background()); !errors.Is(err, ErrDisconnected) {
		t.Fatal("slow queue still delivered", err)
	}
	c, err = h.Subscribe(i)
	if err != nil {
		t.Fatal(err)
	}
	if err = h.Enqueue(c, frame); err != nil {
		t.Fatal(err)
	}
	if err = a.Revoke(token); err != nil {
		t.Fatal(err)
	}
	if _, err = c.Next(context.Background()); !errors.Is(err, command.ErrDenied) {
		t.Fatal("revoked queued frame", err)
	}
	if h.Connected(i) {
		t.Fatal("disconnect retained seat control")
	}
}
