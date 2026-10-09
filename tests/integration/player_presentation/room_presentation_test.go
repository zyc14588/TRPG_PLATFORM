//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package presentation_test

import (
	"bytes"
	"encoding/json"
	"io"
	"net/http"
	"reflect"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
	"github.com/zyc14588/TRPG_PLATFORM/tests/integration/player_presentation/fixture"
)

func roomData(t *testing.T, h *fixture.Harness, which, room string) map[string]any {
	t.Helper()
	v, s, headers, _ := h.RoomGET(t, which, room, nil)
	if headers.Get("Cache-Control") != "no-store" || headers.Get("Referrer-Policy") != "no-referrer" {
		t.Fatal("room presentation privacy headers missing")
	}
	return data(t, v, s)
}

func roomDenied(t *testing.T, v map[string]any, status int) {
	t.Helper()
	if status == http.StatusOK || v["data"] != nil {
		t.Fatal("room presentation retained invalid authority")
	}
	e, ok := v["error"].(map[string]any)
	if !ok || len(e) != 2 {
		t.Fatal("closed safe error missing")
	}
}

func TestRoomPresentationGuestClaimAndInvitedNonmemberContinueIndependentConsent(t *testing.T) {
	for _, kind := range []string{"guest", "claimed", "invited_nonmember"} {
		t.Run(kind, func(t *testing.T) {
			var h *fixture.Harness
			if kind != "invited_nonmember" {
				h = fixture.New(t, true)
			} else {
				h = fixture.NewWithDependency(t)
			}
			if kind == "claimed" {
				before := h.RoomRelationshipCounts(t)
				h.ClaimRoomGuest(t)
				if before != h.RoomRelationshipCounts(t) {
					t.Fatal("claim added membership, participation or preparation")
				}
			}
			if kind == "invited_nonmember" {
				foreign := h.ParticipantOwnWorkspace(t)
				if h.SQL(t, "SELECT count(*) FROM platform_core.memberships WHERE workspace_id='"+foreign+"'") != "1" {
					t.Fatal("cross-workspace invitation account lacks its independent workspace")
				}
			}
			if h.SQL(t, "SELECT count(*) FROM platform_core.memberships WHERE workspace_id='"+h.Workspace()+"'") != "1" {
				t.Fatal("nonmember flow acquired workspace membership")
			}
			before, counts := h.RoomBaseline(t), h.RoomRelationshipCounts(t)
			d := roomData(t, h, "participant", h.Scope().RoomID)
			if d["configuration_id"] != "ai-required" || d["game_id"] != "game" || len(d["packages"].([]any)) < 1 || len(d["model_selections"].([]any)) != 0 {
				t.Fatal("participant received incorrect packages or private model inventory")
			}
			if before != h.RoomBaseline(t) || h.ProviderCalls.Load() != 0 {
				t.Fatal("room presentation mutated or dispatched")
			}
			if kind != "guest" {
				v, s, _, _ := h.GET(t, "participant", "ai-required", nil)
				roomDenied(t, v, s)
			}
			lobby := h.RoomConsent(t, "participant")
			if lobby["configuration_hash"] != d["configuration_hash"] || lobby["graph_hash"] != d["graph_hash"] || !lobby["own_consent"].(map[string]any)["consent"].(bool) || lobby["can_manage"].(bool) {
				t.Fatal("room read could not continue existing independent consent")
			}
			if counts != h.RoomRelationshipCounts(t) || h.SQL(t, "SELECT count(*) FROM platform_launch.acknowledgments WHERE workspace_id='"+h.Workspace()+"'") != "1" {
				t.Fatal("consent expanded relationships or confirmed another participant")
			}
			h.RoomRuntimeProof(t)
		})
	}
}

func TestRoomPresentationMembershipRemovalPreservesParticipationAndOldDenial(t *testing.T) {
	h := fixture.New(t, false)
	h.AddParticipantMembership(t)
	v, s, _, _ := h.GET(t, "participant", "ai-required", nil)
	_ = data(t, v, s)
	h.RemoveParticipantMembership(t)
	before := h.RoomBaseline(t)
	_ = roomData(t, h, "participant", h.Scope().RoomID)
	v, s, _, _ = h.GET(t, "participant", "ai-required", nil)
	roomDenied(t, v, s)
	if before != h.RoomBaseline(t) {
		t.Fatal("read restored membership or changed the room")
	}
	h.RevokeRoomBasis(t, "participant")
	v, s, _, _ = h.RoomGET(t, "participant", h.Scope().RoomID, nil)
	roomDenied(t, v, s)
}

func TestRoomPresentationExistingUnseatedManagerAndLostAllReadBases(t *testing.T) {
	h := fixture.New(t, false)
	h.UnseatedRoomManager(t)
	before := h.RoomBaseline(t)
	d := roomData(t, h, "participant", h.Scope().RoomID)
	if len(d["model_selections"].([]any)) != 0 || before != h.RoomBaseline(t) {
		t.Fatal("unseated manager acquired AI-seat data or a grant")
	}
	h.RemoveParticipantMembership(t)
	v, s, _, _ := h.RoomGET(t, "participant", h.Scope().RoomID, nil)
	roomDenied(t, v, s)
}

func TestRoomPresentationRevalidatesAllCurrentIdentityAndRoomGrounds(t *testing.T) {
	for _, fault := range []string{"participant", "account", "cookie", "cookie_expired", "guest_expired", "guest_revoked", "closed", "preparation", "malformed_preparation"} {
		t.Run(fault, func(t *testing.T) {
			h := fixture.New(t, strings.HasPrefix(fault, "guest_"))
			_ = roomData(t, h, "participant", h.Scope().RoomID)
			h.RevokeRoomBasis(t, fault)
			before := h.RoomBaseline(t)
			v, s, _, _ := h.RoomGET(t, "participant", h.Scope().RoomID, nil)
			roomDenied(t, v, s)
			if before != h.RoomBaseline(t) {
				t.Fatal("failed identity/source read changed business state")
			}
		})
	}
}

func TestRoomPresentationWrongRoomTenantAndInvitationOnlyDenied(t *testing.T) {
	h := fixture.New(t, true)
	for _, which := range []string{"unrelated", "participant"} {
		v, s, _, _ := h.RoomGET(t, which, h.Scope().RoomID, func(r *http.Request) {
			if which == "participant" {
				r.URL.Path = strings.Replace(r.URL.Path, h.Scope().RoomID, "wrong-room", 1)
			}
		})
		roomDenied(t, v, s)
	}
	v, s, _, _ := h.RoomGET(t, "participant", h.Scope().RoomID, func(r *http.Request) {
		r.URL.Path = strings.Replace(r.URL.Path, h.Workspace(), "different-tenant", 1)
	})
	roomDenied(t, v, s)
	pending := fixture.New(t, false)
	pending.PendingRoomInvitation(t)
	v, s, _, _ = pending.RoomGET(t, "participant", pending.Scope().RoomID, nil)
	roomDenied(t, v, s)
}

func TestRoomPresentationTwoRoomsKeepModelsSeatsCapabilitiesAndReadyIndependent(t *testing.T) {
	h := fixture.New(t, false)
	second := h.AddSecondRoom(t)
	h.Confirm(t)
	before := h.RoomBaseline(t)
	first := roomData(t, h, "owner", h.Scope().RoomID)
	other := roomData(t, h, "owner", second)
	a, b := first["model_selections"].([]any), other["model_selections"].([]any)
	if first["configuration_id"] != other["configuration_id"] || first["configuration_hash"] != other["configuration_hash"] || first["graph_hash"] != other["graph_hash"] || len(a) != 1 || len(b) != 1 {
		t.Fatal("same current configuration did not produce independent room results")
	}
	x, y := a[0].(map[string]any), b[0].(map[string]any)
	if x["selection_id"] != "selected" || y["selection_id"] != "selected" || !reflect.DeepEqual(x["seat_ids"], []any{"ai"}) || !reflect.DeepEqual(y["seat_ids"], []any{"ai-other"}) || x["ready"] != true || y["ready"] != false || reflect.DeepEqual(x["capabilities"], y["capabilities"]) {
		t.Fatal("room models, seats, capabilities or readiness were combined")
	}
	if h.SQL(t, "SELECT count(DISTINCT tuple_hash) FROM platform_model.configurations WHERE workspace_id='"+h.Workspace()+"'") != "2" {
		t.Fatal("two actual certified models were not independently configured")
	}
	if before != h.RoomBaseline(t) {
		t.Fatal("independent room previews changed business state")
	}
}

func TestRoomPresentationRoomAndTenantPredicatesPrecedeCandidateLimit(t *testing.T) {
	h := fixture.New(t, false)
	second := h.AddSecondRoom(t)
	h.SeedRoomCandidates(t, second, "ai-other", 70, nil)
	foreign := fixture.New(t, false)
	identity := h.RoomModel(t)
	foreign.SeedRoomCandidates(t, foreign.Scope().RoomID, "ai", 70, &identity)
	if foreign.SQL(t, "SELECT count(*) FROM platform_model.configurations WHERE workspace_id='"+foreign.Workspace()+"' AND configuration_hash='"+identity.ConfigurationHash+"' AND graph_hash='"+identity.GraphHash+"'") != "70" {
		t.Fatal("cross-tenant identical-hash candidates were not physically present")
	}
	d := roomData(t, h, "owner", h.Scope().RoomID)
	if len(d["model_selections"].([]any)) != 1 {
		t.Fatal("other-room/tenant rows consumed this room's result bound")
	}
	h.SeedRoomCandidates(t, h.Scope().RoomID, "ai", 63, nil)
	if h.SQL(t, "SELECT count(*) FROM platform_model.configurations WHERE workspace_id='"+h.Workspace()+"' AND room_id='"+h.Scope().RoomID+"'") != "64" {
		t.Fatal("exact 64-candidate boundary not exercised")
	}
	d = roomData(t, h, "owner", h.Scope().RoomID)
	if len(d["model_selections"].([]any)) != 1 {
		t.Fatal("exact 64 candidates lost the actual current model")
	}
	_ = h.SQL(t, "INSERT INTO platform_model.configurations SELECT workspace_id,room_id,game_id,seat_id,'overflow',version,credential_id,credential_version,primary_id,primary_hash,tuple_hash,configuration_id,configuration_hash,graph_hash,preparation_revision,owner_kind,owner_id,convert_to(replace(convert_from(body,'UTF8'),'\"Selection\":\"selected\"','\"Selection\":\"overflow\"'),'UTF8'),revoked FROM platform_model.configurations WHERE workspace_id='"+h.Workspace()+"' AND room_id='"+h.Scope().RoomID+"' AND selection='selected'")
	if h.SQL(t, "SELECT count(*) FROM platform_model.configurations WHERE workspace_id='"+h.Workspace()+"' AND room_id='"+h.Scope().RoomID+"'") != "65" {
		t.Fatal("65-candidate overflow boundary not exercised")
	}
	before := h.RoomBaseline(t)
	v, s, _, _ := h.RoomGET(t, "owner", h.Scope().RoomID, nil)
	roomDenied(t, v, s)
	if s != http.StatusServiceUnavailable || before != h.RoomBaseline(t) {
		t.Fatal("overflow was truncated or changed state")
	}
}

func TestRoomPresentationPackageGraphIsActualCompleteAndMappingIsCurrent(t *testing.T) {
	h := fixture.NewWithDependency(t)
	d := roomData(t, h, "owner", h.Scope().RoomID)
	if len(d["packages"].([]any)) != 2 {
		t.Fatal("actual resolved dependency missing")
	}
	for _, c := range h.Configurations() {
		x := c.StorageValue()
		if x.ID != "ai-required" {
			continue
		}
		r := x.Request
		r.Session = h.Scope().GameID
		hash, actual, e := x.Factory.Presentation(h.Context(), r)
		if e != nil || hash != d["graph_hash"] {
			t.Fatal("actual authenticated graph mismatch")
		}
		raw, _ := json.Marshal(actual)
		var normalized any
		_ = json.Unmarshal(raw, &normalized)
		if !reflect.DeepEqual(normalized, d["packages"]) {
			t.Fatal("native package IDs, versions, rights, license or capabilities changed")
		}
	}
	h.ChangeConfiguration(t, "minimal")
	next := roomData(t, h, "participant", h.Scope().RoomID)
	if next["configuration_id"] != "minimal" || next["configuration_hash"] == d["configuration_hash"] || len(next["model_selections"].([]any)) != 0 {
		t.Fatal("current room mapping fell back to prior configuration/model")
	}
	for _, missing := range []bool{false, true} {
		broken := fixture.NewWithDependency(t)
		broken.BreakGraphMapping(t, missing)
		v, s, _, _ := broken.RoomGET(t, "owner", broken.Scope().RoomID, nil)
		roomDenied(t, v, s)
	}
}

func TestRoomPresentationPackageLimitIsComplete64AndRejects65(t *testing.T) {
	for _, count := range []int{64, 65} {
		t.Run(strconv.Itoa(count), func(t *testing.T) {
			h := fixture.NewRoomPackageGraph(t, count)
			before := h.RoomBaseline(t)
			v, s, _, _ := h.RoomGET(t, "owner", h.Scope().RoomID, nil)
			if count == 64 {
				if len(data(t, v, s)["packages"].([]any)) != 64 {
					t.Fatal("complete legal graph truncated")
				}
			} else {
				roomDenied(t, v, s)
			}
			if before != h.RoomBaseline(t) {
				t.Fatal("bounded graph read instantiated or wrote authority")
			}
		})
	}
}

func TestRoomPresentationStaleIncompatibleUncertifiedAndOverbudgetModelsNeverBecomeReady(t *testing.T) {
	for _, fault := range []string{"stale_revision", "configuration_hash", "graph_hash", "incompatible", "uncertified", "overbudget"} {
		t.Run(fault, func(t *testing.T) {
			h := fixture.New(t, false)
			h.Confirm(t)
			h.ChangeRoomModelRecord(t, fault)
			before := h.RoomBaseline(t)
			d := roomData(t, h, "owner", h.Scope().RoomID)
			if len(d["model_selections"].([]any)) != 0 || before != h.RoomBaseline(t) {
				t.Fatal("invalid current qualification became visible/ready or wrote state")
			}
		})
	}
}

func TestRoomPresentationCurrentModelQualificationBudgetAndUnknownUsage(t *testing.T) {
	for _, fault := range []string{"model", "credential", "certificate", "budget", "unknown_billing", "malformed_budget", "malformed_model"} {
		t.Run(fault, func(t *testing.T) {
			h := fixture.New(t, false)
			h.Confirm(t)
			d := roomData(t, h, "owner", h.Scope().RoomID)
			if d["model_selections"].([]any)[0].(map[string]any)["ready"] != true {
				t.Fatal("qualified current model was not ready")
			}
			switch fault {
			case "model":
				h.RevokeModel(t)
			case "credential":
				h.RevokeCredential(t)
			case "certificate":
				h.RevokeCertificate(t)
			case "budget":
				h.ExhaustBudget(t)
			case "unknown_billing":
				h.MarkUnknownBilling(t)
			case "malformed_budget":
				h.CorruptBudget(t)
			case "malformed_model":
				h.RevokeRoomBasis(t, "malformed_model")
			}
			before := h.RoomBaseline(t)
			v, s, _, _ := h.RoomGET(t, "owner", h.Scope().RoomID, nil)
			if strings.HasPrefix(fault, "malformed_") {
				roomDenied(t, v, s)
			} else {
				x := data(t, v, s)["model_selections"].([]any)
				if fault == "budget" || fault == "unknown_billing" {
					if len(x) != 1 || x[0].(map[string]any)["ready"] != false {
						t.Fatal("unfunded or uncertain model became ready")
					}
				} else if len(x) != 0 {
					t.Fatal("revoked model authority remained visible")
				}
			}
			if before != h.RoomBaseline(t) || h.ProviderCalls.Load() != 0 {
				t.Fatal("model preview dispatched, resumed or mutated")
			}
		})
	}
}

func TestRoomPresentationSingleInspectionCancellationTimeoutAndUnknownOutcome(t *testing.T) {
	for _, kind := range []string{"read", "canceled", "timeout", "unknown"} {
		t.Run(kind, func(t *testing.T) {
			h := fixture.New(t, false)
			probe := h.ProbeRoomRead(t)
			before := h.RoomBaseline(t)
			if kind == "canceled" {
				if h.RoomCanceled(t) == nil || probe.Calls.Load() != 0 {
					t.Fatal("canceled read opened authority")
				}
			} else {
				probe.Delay.Store(kind == "timeout")
				if kind == "unknown" {
					probe.Unknown.Store(true)
				}
				start := time.Now()
				v, s, _, _ := h.RoomGET(t, "owner", h.Scope().RoomID, nil)
				if kind == "read" {
					_ = data(t, v, s)
				} else {
					roomDenied(t, v, s)
				}
				if kind == "unknown" && v["error"].(map[string]any)["code"] != "OUTCOME_UNKNOWN" || kind == "timeout" && time.Since(start) > 6*time.Second || probe.Calls.Load() != 1 {
					t.Fatal("single inspection deadline or safe unknown outcome failed")
				}
			}
			if before != h.RoomBaseline(t) || h.OpenTransactionCount(t) != "0" {
				t.Fatal("inspection wrote, dispatched or retained a transaction")
			}
		})
	}
}

func TestRoomPresentationInheritedClosedHTTPSFormsAndPublicPrivacy(t *testing.T) {
	h := fixture.New(t, false)
	before := h.RoomBaseline(t)
	cases := map[string]func(*http.Request){
		"query":            func(r *http.Request) { r.URL.RawQuery = "configuration=minimal" },
		"empty_query":      func(r *http.Request) { r.URL.ForceQuery = true },
		"post":             func(r *http.Request) { r.Method = http.MethodPost },
		"authorization":    func(r *http.Request) { r.Header.Set("Authorization", "Bearer synthetic") },
		"foreign_origin":   func(r *http.Request) { r.Header.Set("Origin", "https://foreign.invalid") },
		"foreign_fetch":    func(r *http.Request) { r.Header.Set("Sec-Fetch-Site", "cross-site") },
		"duplicate_origin": func(r *http.Request) { r.Header.Add("Origin", r.Header.Get("Origin")) },
		"duplicate_cookie": func(r *http.Request) { r.Header.Add("Cookie", r.Header.Get("Cookie")) },
		"body":             func(r *http.Request) { r.Body = io.NopCloser(strings.NewReader("{}")); r.ContentLength = 2 },
		"extra_path":       func(r *http.Request) { r.URL.Path += "/extra" },
		"content_type":     func(r *http.Request) { r.Header.Set("Content-Type", "application/json") },
		"csrf_override":    func(r *http.Request) { r.Header.Set("X-CSRF-Token", "synthetic") },
		"header_bound": func(r *http.Request) {
			for i := 0; i < 65; i++ {
				r.Header.Set("X-Bound-"+strings.Repeat("a", i+1), "bounded")
			}
		},
	}
	for name, mutate := range cases {
		t.Run(name, func(t *testing.T) {
			v, s, _, raw := h.RoomGET(t, "owner", h.Scope().RoomID, mutate)
			roomDenied(t, v, s)
			if len(raw) > player.MaxResponseBytes {
				t.Fatal("failed form exceeded response bound")
			}
		})
	}
	v, s, _, raw := h.RoomGET(t, "owner", h.Scope().RoomID, nil)
	_ = data(t, v, s)
	for _, private := range []string{"owned-B014-synthetic-private-provider-key", "owned-B015-other-room-synthetic-private-key", "endpoint", "CredentialID", "ciphertext", "fixture-gm-private-value", "Controller", "Boundaries", "certificate", "127.0.0.1:", "SELECT "} {
		if bytes.Contains(raw, []byte(private)) {
			t.Fatal("private source leaked; body withheld")
		}
	}
	if len(raw) > player.MaxResponseBytes || before != h.RoomBaseline(t) {
		t.Fatal("closed request/read mutated or exceeded the response bound")
	}
}
