//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package main

import (
	"net/http"
	"strconv"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/tests/integration/player_presentation/fixture"
)

func serveRoomProduction(t *testing.T, h *fixture.Harness) {
	t.Helper()
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
}

func TestRoomPresentationActualProductionCompositionGuestClaimAndNonmemberConsent(t *testing.T) {
	for _, kind := range []string{"guest", "claimed", "invited_nonmember", "manager"} {
		t.Run(kind, func(t *testing.T) {
			h := fixture.New(t, kind == "guest" || kind == "claimed")
			if kind == "claimed" {
				h.ClaimRoomGuest(t)
			}
			which := "participant"
			if kind == "manager" {
				h.UnseatHost(t)
				which = "owner"
			}
			if kind == "invited_nonmember" {
				h.ParticipantOwnWorkspace(t)
			}
			serveRoomProduction(t, h)
			before := h.RoomBaseline(t)
			v, status, _, _ := h.RoomGET(t, which, h.Scope().RoomID, nil)
			if status != http.StatusOK || v["data"] == nil || len(v["data"].(map[string]any)["model_selections"].([]any)) != 0 {
				t.Fatal("production current-room participant/manager composition unavailable")
			}
			if before != h.RoomBaseline(t) || h.ProviderCalls.Load() != 0 {
				t.Fatal("production room read mutated, instantiated or dispatched")
			}
			if kind == "claimed" || kind == "invited_nonmember" {
				old, s, _, _ := h.GET(t, "participant", "ai-required", nil)
				if s == http.StatusOK || old["data"] != nil {
					t.Fatal("production original endpoint admitted a nonmember")
				}
			}
			if kind != "manager" {
				lobby := h.RoomConsent(t, "participant")
				if !lobby["own_consent"].(map[string]any)["consent"].(bool) || lobby["can_manage"].(bool) {
					t.Fatal("production original consent/preparation delegation failed")
				}
			}
			h.RoomRuntimeProof(t)
		})
	}
}

func TestRoomPresentationProductionMembershipLossAndRoomRevocation(t *testing.T) {
	h := fixture.New(t, false)
	h.AddParticipantMembership(t)
	serveRoomProduction(t, h)
	h.RemoveParticipantMembership(t)
	v, s, _, _ := h.RoomGET(t, "participant", h.Scope().RoomID, nil)
	if s != http.StatusOK || v["data"] == nil {
		t.Fatal("production membership loss erased independent room participation")
	}
	v, s, _, _ = h.GET(t, "participant", "ai-required", nil)
	if s == http.StatusOK || v["data"] != nil {
		t.Fatal("production original endpoint lost its membership guard")
	}
	h.RevokeRoomBasis(t, "participant")
	v, s, _, _ = h.RoomGET(t, "participant", h.Scope().RoomID, nil)
	if s == http.StatusOK || v["data"] != nil {
		t.Fatal("production room revocation retained cached presentation")
	}
}

func TestRoomPresentationProductionSameSelectionRemainsRoomScoped(t *testing.T) {
	h := fixture.New(t, false)
	second := h.AddSecondRoom(t)
	h.Confirm(t)
	serveRoomProduction(t, h)
	for _, row := range []struct {
		room, seat string
		ready      bool
	}{{h.Scope().RoomID, "ai", true}, {second, "ai-other", false}} {
		v, s, _, _ := h.RoomGET(t, "owner", row.room, nil)
		if s != http.StatusOK || v["data"] == nil {
			t.Fatal("production scoped presentation unavailable")
		}
		x := v["data"].(map[string]any)["model_selections"].([]any)
		if len(x) != 1 {
			t.Fatal("production merged or omitted room models")
		}
		m := x[0].(map[string]any)
		seats := m["seat_ids"].([]any)
		if m["selection_id"] != "selected" || m["label"] != "Production bound AI" || m["ready"] != row.ready || len(seats) != 1 || seats[0] != row.seat {
			t.Fatal("production room model/label/seat/ready binding lost")
		}
	}
}

func TestRoomPresentationProductionPackageLimitComplete64AndRejects65(t *testing.T) {
	for _, count := range []int{64, 65} {
		t.Run(strconv.Itoa(count), func(t *testing.T) {
			h := fixture.NewRoomPackageGraph(t, count)
			serveRoomProduction(t, h)
			before := h.RoomBaseline(t)
			v, status, _, _ := h.RoomGET(t, "owner", h.Scope().RoomID, nil)
			if count == 64 {
				if status != http.StatusOK || v["data"] == nil || len(v["data"].(map[string]any)["packages"].([]any)) != 64 {
					t.Fatal("production complete legal package graph unavailable or truncated")
				}
			} else if status == http.StatusOK || v["data"] != nil || v["error"] == nil {
				t.Fatal("production overbound graph returned presentation")
			}
			if before != h.RoomBaseline(t) || h.ProviderCalls.Load() != 0 {
				t.Fatal("production bounded graph read mutated, instantiated or dispatched")
			}
			h.RoomRuntimeProof(t)
		})
	}
}
