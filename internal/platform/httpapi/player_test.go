// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package httpapi

import (
	"crypto/tls"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
)

func TestPlayerOwnsExactlyApprovedFifteenRoutes(t *testing.T) {
	prefix := "/api/v1/workspaces/workspace/rooms/room/"
	routes := []struct{ method, path, action string }{{"GET", "/api/v1/workspaces/workspace/games", "catalog"}, {"GET", prefix + "preparation", "lobby"}, {"POST", prefix + "preparation", "configure"}, {"POST", prefix + "consent", "consent"}, {"GET", prefix + "readiness", "readiness"}, {"POST", prefix + "launch", "launch"}, {"POST", prefix + "session/connect", "connect"}, {"POST", prefix + "session/snapshot", "snapshot"}, {"POST", prefix + "session/commands", "command"}, {"POST", prefix + "session/disconnect", "disconnect"}, {"POST", prefix + "session/pause", "pause"}, {"POST", prefix + "session/resume", "resume"}, {"POST", prefix + "session/export", "export"}, {"POST", prefix + "session/recovery-point", "create_point"}, {"GET", prefix + "session/recovery-point", "read_point"}}
	for _, r := range routes {
		t.Run(r.action, func(t *testing.T) {
			action, w, _, owned := playerRoute(r.method, r.path)
			if action != r.action || !owned || w != "workspace" {
				t.Fatal("approved route missing")
			}
			if action, _, _, owned = playerRoute("DELETE", r.path); action != "" || !owned {
				t.Fatal("unapproved method accepted")
			}
			if action, _, _, owned = playerRoute(r.method, r.path+"/extra"); action != "" || !owned {
				t.Fatal("unapproved tail accepted")
			}
		})
	}
	for _, path := range []string{"/api/v1/auth/context", prefix + "invitations", prefix + "admissions", prefix + "participants/one/role"} {
		if _, _, _, owned := playerRoute("GET", path); owned {
			t.Fatal("player transport swallowed existing endpoint")
		}
	}
}
func TestPlayerTransportRejectsDuplicateHeadersAndCrossOriginBeforeDispatch(t *testing.T) {
	baseData := &handlerData{origin: "https://player.local", host: "player.local"}
	base := &Handler{data: &baseData}
	roomData := &roomHandlerData{base: base}
	rooms := &RoomHandler{data: &roomData}
	pd := &playerHandlerData{base: rooms}
	handler := &PlayerHandler{data: &pd}
	for _, name := range []string{"cleartext", "host", "query", "encoded", "authorization", "cross-site", "same-site", "origin", "duplicate-origin", "duplicate-csrf", "duplicate-cookie", "case-alias", "header-control", "header-total", "get-body"} {
		t.Run(name, func(t *testing.T) {
			r := httptest.NewRequest(http.MethodGet, "https://player.local/api/v1/workspaces/workspace/games", nil)
			r.TLS = &tls.ConnectionState{}
			r.RemoteAddr = "127.0.0.1:1"
			switch name {
			case "cleartext":
				r.TLS = nil
			case "host":
				r.Host = "other.local"
			case "query":
				r.URL.RawQuery = "token=untrusted"
			case "encoded":
				r.URL.RawPath = r.URL.Path
			case "authorization":
				r.Header.Set("Authorization", "Bearer untrusted")
			case "cross-site":
				r.Header.Set("Sec-Fetch-Site", "cross-site")
			case "same-site":
				r.Header.Set("Sec-Fetch-Site", "same-site")
			case "origin":
				r.Header.Set("Origin", "https://other.local")
			case "duplicate-origin":
				r.Header["Origin"] = []string{"https://player.local", "https://player.local"}
			case "duplicate-csrf":
				r.Header["X-Csrf-Token"] = []string{"one", "two"}
			case "duplicate-cookie":
				r.Header["Cookie"] = []string{"one=1", "two=2"}
			case "case-alias":
				r.Header["Origin"] = []string{"https://player.local"}
				r.Header["origin"] = []string{"https://player.local"}
			case "header-control":
				r.Header.Set("X-Test", "bad\x01header")
			case "header-total":
				for _, key := range []string{"X-One", "X-Two", "X-Three", "X-Four", "X-Five"} {
					r.Header.Set(key, strings.Repeat("x", 8192))
				}
			case "get-body":
				r.ContentLength = 2
				r.Body = http.NoBody
			}
			w := httptest.NewRecorder()
			handler.ServeHTTP(w, r)
			if w.Code != http.StatusBadRequest && w.Code != http.StatusForbidden {
				t.Fatal("unsafe transport reached facade")
			}
			if w.Header().Get("Cache-Control") != "no-store" || w.Header().Get("Referrer-Policy") != "no-referrer" {
				t.Fatal("private response headers missing")
			}
			if strings.Contains(w.Body.String(), "untrusted") {
				t.Fatal("transport echoed request material")
			}
		})
	}
}
func TestPlayerPauseErrorsUseOnlyApprovedSafePairs(t *testing.T) {
	for _, e := range []error{player.ErrPaused, player.ErrConnectionExpired, auth.ErrDenied} {
		w := httptest.NewRecorder()
		playerFail(w, e)
		var m map[string]any
		if json.Unmarshal(w.Body.Bytes(), &m) != nil {
			t.Fatal("error response invalid")
		}
		err := m["error"].(map[string]any)
		if err["code"] != e.Error() {
			t.Fatal("error code changed")
		}
		if strings.Contains(err["message"].(string), "private") {
			t.Fatal("private error detail exposed")
		}
	}
}
