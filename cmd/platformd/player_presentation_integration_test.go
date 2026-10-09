//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package main

import (
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/tests/integration/player_presentation/fixture"
	"net/http"
	"testing"
)

func TestMain(m *testing.M) { fixture.Run(m) }
func TestPresentationActualProductionCompositionOverHTTPS(t *testing.T) {
	h := fixture.New(t, false)
	old, e := fixture.ReadProductFile("schemas/platform/platform-player-api-v1.schema.json")
	if e != nil {
		t.Fatal(auth.SafeError(e))
	}
	schema, e := fixture.ReadProductFile("schemas/platform/platform-player-presentation-api-v1.schema.json")
	if e != nil {
		t.Fatal(auth.SafeError(e))
	}
	var c *platformPlayerComponents
	h.Serve(t, func(origin string) http.Handler {
		var e error
		c, e = platformPlayersWithPresentation(platformPlayerOptions{ctx: h.Context(), repo: h.Repository(), authority: h.Authority(), roomStorage: h.RoomStorage(), rooms: h.Rooms(), configurations: h.Configurations(), models: h.Models, policies: h.Policies(), descriptions: h.Descriptions(), schema: old, origin: origin, maxSessions: 8, maxConnections: 64}, h.Models, map[string]string{h.Workspace() + "/selected": "Production bound AI"}, schema)
		if e != nil {
			t.Fatal(auth.SafeError(e))
		}
		return c.state().handler
	})
	t.Cleanup(func() {
		if e := c.close(); e != nil {
			t.Fatal(auth.SafeError(e))
		}
	})
	before := h.Baseline(t)
	v, status, _, _ := h.GET(t, "owner", "ai-required", nil)
	if status != 200 || v["data"] == nil {
		t.Fatal("production composition unavailable")
	}
	if before != h.Baseline(t) || h.ProviderCalls.Load() != 0 {
		t.Fatal("production read dispatched or mutated")
	}
}
