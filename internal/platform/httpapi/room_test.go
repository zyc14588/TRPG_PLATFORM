// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package httpapi

import (
	"bytes"
	"context"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
)

type roomOfflineRepo struct{}

func (roomOfflineRepo) Transact(context.Context, func(auth.Transaction) error) error {
	return auth.ErrUnauthenticated
}

type roomOfflineStorage struct{}

func (roomOfflineStorage) Bind(core.Transaction) (room.Transaction, error) {
	return nil, auth.ErrDenied
}
func roomTransportService(t *testing.T) *room.Service {
	t.Helper()
	key := bytes.Repeat([]byte{3}, 32)
	store := roomOfflineStorage{}
	verifier, e := room.NewAdmissionVerifier(store, key)
	if e != nil {
		t.Fatal("verifier composition")
	}
	as, e := os.ReadFile("../../../schemas/platform/platform-auth-api-v1.schema.json")
	if e != nil {
		t.Fatal("auth Schema read")
	}
	rs, e := os.ReadFile("../../../schemas/platform/platform-room-api-v1.schema.json")
	if e != nil {
		t.Fatal("room Schema read")
	}
	a, e := auth.NewRoomAuthority(roomOfflineRepo{}, bytes.Repeat([]byte{1}, 32), bytes.Repeat([]byte{2}, 32), key, as, verifier, verifier)
	if e != nil {
		t.Fatal("authority composition")
	}
	s, e := room.NewService(a, store, key, rs)
	if e != nil {
		t.Fatal("room composition")
	}
	return s
}
func TestRoomRouteInventory(t *testing.T) {
	paths := []struct{ method, path, action string }{
		{"POST", "/api/v1/workspaces/w/rooms", "create_room"}, {"GET", "/api/v1/workspaces/w/rooms/r", "get_room"}, {"DELETE", "/api/v1/workspaces/w/rooms/r", "close_room"},
		{"POST", "/api/v1/workspaces/w/rooms/r/invitations", "create_invite"}, {"DELETE", "/api/v1/workspaces/w/rooms/r/invitations/i", "revoke_invite"},
		{"POST", "/api/v1/room-admissions", "request_admission"}, {"GET", "/api/v1/room-admissions/a", "get_admission"}, {"POST", "/api/v1/room-admissions/a/guest-token", "guest_token"},
		{"GET", "/api/v1/workspaces/w/rooms/r/admissions", "list_admissions"}, {"POST", "/api/v1/workspaces/w/rooms/r/admissions/a/decision", "decide_admission"},
		{"POST", "/api/v1/workspaces/w/rooms/r/leave", "leave_room"}, {"DELETE", "/api/v1/workspaces/w/rooms/r/participants/p", "kick_participant"}, {"PUT", "/api/v1/workspaces/w/rooms/r/participants/p/role", "set_role"},
	}
	for _, p := range paths {
		a, _, _, _, owned := roomRoute(p.method, p.path)
		if !owned || a != p.action {
			t.Fatal("approved endpoint not routed")
		}
	}
	for _, p := range []string{"/api/v1/workspaces/w/rooms/", "/api/v1/workspaces/w/rooms/r/seats", "/api/v1/workspaces/w/rooms/r/launch", "/api/v1/rooms"} {
		a, _, _, _, _ := roomRoute("POST", p)
		if a != "" {
			t.Fatal("unapproved endpoint routed")
		}
	}
}
func TestRoomTLSInheritedGuards(t *testing.T) {
	s := roomTransportService(t)
	server := httptest.NewUnstartedServer(nil)
	origin := "https://" + server.Listener.Addr().String()
	h, e := NewRoomHandler(s, origin)
	if e != nil {
		t.Fatal("handler composition")
	}
	server.Config.Handler = h
	server.StartTLS()
	defer server.Close()
	for _, tc := range []struct {
		name, method, path, body string
		status                   int
		alter                    func(*http.Request)
	}{
		{"base_get", "GET", "/api/v1/workspaces/w/rooms/r", "", 401, nil},
		{"query", "GET", "/api/v1/workspaces/w/rooms/r?token=synthetic", "", 403, nil},
		{"host", "GET", "/api/v1/workspaces/w/rooms/r", "", 403, func(r *http.Request) { r.Host = "other.invalid" }},
		{"origin", "GET", "/api/v1/workspaces/w/rooms/r", "", 403, func(r *http.Request) { r.Header.Set("Origin", "https://other.invalid") }},
		{"bearer", "GET", "/api/v1/workspaces/w/rooms/r", "", 403, func(r *http.Request) { r.Header.Set("Authorization", "Bearer synthetic") }},
		{"cross_site", "GET", "/api/v1/workspaces/w/rooms/r", "", 403, func(r *http.Request) { r.Header.Set("Sec-Fetch-Site", "cross-site") }},
		{"duplicate_origin", "GET", "/api/v1/workspaces/w/rooms/r", "", 403, func(r *http.Request) { r.Header.Add("Origin", origin); r.Header.Add("Origin", origin) }},
		{"duplicate_cookie", "GET", "/api/v1/workspaces/w/rooms/r", "", 400, func(r *http.Request) { r.Header.Set("Cookie", SessionCookie+"=a; "+SessionCookie+"=b") }},
		{"unknown_field", "POST", "/api/v1/workspaces/w/rooms", `{"schema_version":1,"name":"room","game_id":"g","owner":"a"}`, 400, nil},
		{"duplicate_key", "POST", "/api/v1/workspaces/w/rooms", `{"schema_version":1,"schema_version":1,"name":"room","game_id":"g"}`, 400, nil},
		{"body_bound", "POST", "/api/v1/workspaces/w/rooms", strings.Repeat(" ", 16385), 400, nil},
		{"compression", "POST", "/api/v1/workspaces/w/rooms", `{}`, 400, func(r *http.Request) { r.Header.Set("Content-Encoding", "gzip") }},
		{"missing_origin", "POST", "/api/v1/workspaces/w/rooms", `{}`, 403, func(r *http.Request) { r.Header.Del("Origin") }},
		{"no_public_list", "GET", "/api/v1/rooms", "", 400, nil},
	} {
		t.Run(tc.name, func(t *testing.T) {
			r, e := http.NewRequest(tc.method, origin+tc.path, strings.NewReader(tc.body))
			if e != nil {
				t.Fatal("request creation")
			}
			if tc.method != "GET" {
				r.Header.Set("Origin", origin)
				r.Header.Set("X-CSRF-Token", strings.Repeat("A", 43))
				r.Header.Set("Idempotency-Key", "transport-key-001")
				r.Header.Set("Content-Type", "application/json")
			}
			if tc.alter != nil {
				tc.alter(r)
			}
			response, e := server.Client().Do(r)
			if e != nil {
				t.Fatal("TLS request")
			}
			defer response.Body.Close()
			if response.StatusCode != tc.status {
				t.Fatalf("HTTP status %d, expected %d", response.StatusCode, tc.status)
			}
			if response.Header.Get("Cache-Control") != "no-store" || response.Header.Get("Referrer-Policy") != "no-referrer" || response.Header.Get("Access-Control-Allow-Origin") != "" {
				t.Fatal("transport boundary headers")
			}
			_, _ = io.Copy(io.Discard, response.Body)
		})
	}
	r := httptest.NewRequest("GET", origin+"/api/v1/workspaces/w/rooms/r", nil)
	r.TLS = nil
	w := httptest.NewRecorder()
	h.ServeHTTP(w, r)
	if w.Code != 403 {
		t.Fatal("plaintext request accepted")
	}
}
