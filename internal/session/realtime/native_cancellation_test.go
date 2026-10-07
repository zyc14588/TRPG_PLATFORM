// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package realtime

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"strings"
	"testing"
)

func TestNativePollCancellationPreservesIdentityWithoutDelivery(t *testing.T) {
	for _, stage := range []string{"before-verify", "during-verify", "during-delivery-policy"} {
		t.Run(stage, func(t *testing.T) {
			b := data.Binding{Workspace: "w", Session: "s", GraphHash: "sha256:" + strings.Repeat("a", 64)}
			a, err := command.NewFixtureAuthority([]command.FixtureSeat{{Credential: "bootstrap-0123456789abcdef", Binding: b, Principal: "system", Seat: "system"}})
			if err != nil {
				t.Fatal("authority fixture failed")
			}
			seat := command.NativeSeat{Binding: b, Principal: "player", Seat: "player", Views: command.ViewPolicy{ViewFields: []string{"counter"}}}
			armed := false
			calls := 0
			ctx, cancel := context.WithCancel(context.Background())
			defer cancel()
			resolve := func(context.Context) (command.NativeSeat, error) {
				if armed {
					calls++
					if stage == "during-verify" && calls == 1 || stage == "during-delivery-policy" && calls == 2 {
						cancel()
					}
				}
				return seat, nil
			}
			i, err := a.IssueNative(context.Background(), resolve)
			if err != nil {
				t.Fatal("issue failed")
			}
			h, err := New(Options{Authority: a, Capacity: 1, PerSession: 1, Queue: 2})
			if err != nil {
				t.Fatal("hub failed")
			}
			defer h.Close()
			c, err := h.Subscribe(i)
			if err != nil {
				t.Fatal("subscribe failed")
			}
			queued := Frame{Kind: "committed", Session: "s", Version: 2, Cursor: 1, View: checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(1)})}
			if err = h.Enqueue(c, queued); err != nil {
				t.Fatal("enqueue failed")
			}
			armed = true
			if stage == "before-verify" {
				cancel()
			}
			frame, err := c.Next(ctx)
			armed = false
			if err != context.Canceled || frame.Session != "" {
				t.Fatal("canceled request returned identity denial or delivered a frame")
			}
			if c.closed.Load() || a.Verify(i) != nil || !h.Connected(i) {
				t.Fatal("request cancellation revoked a valid connection")
			}
			if stage == "during-delivery-policy" {
				// This frame was withheld after dequeue. Reconnect can fetch its durable
				// cursor; delivery of a subsequent frame must still use current policy.
				if err = h.Enqueue(c, queued); err != nil {
					t.Fatal("recovery enqueue failed")
				}
			}
			frame, err = c.Next(context.Background())
			if err != nil || frame.Cursor != 1 || frame.View.Table["counter"].Number != "1" {
				t.Fatal("valid connection did not resume after canceled poll")
			}
		})
	}
}
