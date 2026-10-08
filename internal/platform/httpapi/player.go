// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package httpapi

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"mime"
	"net"
	"net/http"
	"strings"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
)

type PlayerHandler struct{ data **playerHandlerData }
type playerHandlerData struct {
	base    *RoomHandler
	players *player.Service
}

func (PlayerHandler) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private player HTTP handler>")
}
func (PlayerHandler) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func NewPlayerHandler(players *player.Service, rooms *room.Service, origin string) (*PlayerHandler, error) {
	if players == nil || rooms == nil || players.Authentication() != rooms.Authentication() {
		return nil, auth.ErrInvalid
	}
	base, e := NewRoomHandler(rooms, origin)
	if e != nil {
		return nil, e
	}
	d := &playerHandlerData{base, players}
	return &PlayerHandler{data: &d}, nil
}
func playerRoute(method, path string) (action, w, r string, owned bool) {
	p := strings.Split(strings.TrimPrefix(path, "/"), "/")
	if len(p) < 5 || p[0] != "api" || p[1] != "v1" || p[2] != "workspaces" {
		return
	}
	w = p[3]
	if p[4] == "games" {
		owned = true
		if len(p) == 5 && method == http.MethodGet {
			action = "catalog"
		}
		return
	}
	if len(p) < 7 || p[4] != "rooms" {
		return
	}
	r = p[5]
	switch p[6] {
	case "preparation", "consent", "readiness", "launch":
		owned = true
		if len(p) != 7 {
			return
		}
		if method == http.MethodGet {
			if p[6] == "preparation" {
				action = "lobby"
			}
			if p[6] == "readiness" {
				action = "readiness"
			}
		}
		if method == http.MethodPost {
			switch p[6] {
			case "preparation":
				action = "configure"
			case "consent":
				action = "consent"
			case "launch":
				action = "launch"
			}
		}
	case "session":
		owned = true
		if len(p) != 8 {
			return
		}
		if method == http.MethodGet && p[7] == "recovery-point" {
			action = "read_point"
		}
		if method == http.MethodPost {
			switch p[7] {
			case "connect", "snapshot", "disconnect", "pause", "resume", "export":
				action = p[7]
			case "commands":
				action = "command"
			case "recovery-point":
				action = "create_point"
			}
		}
	}
	return
}
func playerFail(w http.ResponseWriter, e error) {
	e = player.SafeError(e)
	if e != player.ErrPaused && e != player.ErrConnectionExpired {
		fail(w, e)
		return
	}
	message := "Game paused"
	if e == player.ErrConnectionExpired {
		message = "Reconnect required"
	}
	w.WriteHeader(http.StatusConflict)
	_ = json.NewEncoder(w).Encode(map[string]any{"schema_version": 1, "request_id": "response", "error": map[string]any{"code": e.Error(), "message": message}})
}
func (h *PlayerHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if h == nil || h.data == nil || *h.data == nil {
		fail(w, auth.ErrUnavailable)
		return
	}
	d := *h.data
	action, workspace, id, owned := playerRoute(r.Method, r.URL.Path)
	if !owned {
		d.base.ServeHTTP(w, r)
		return
	}
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("X-Content-Type-Options", "nosniff")
	w.Header().Set("Referrer-Policy", "no-referrer")
	base := *(*d.base.data).base.data
	if r.TLS == nil || r.Host != base.host || r.URL.RawQuery != "" || r.URL.ForceQuery || r.URL.RawPath != "" || len(r.Header.Values("Authorization")) != 0 || r.URL.User != nil || r.URL.Fragment != "" {
		playerFail(w, auth.ErrDenied)
		return
	}
	seen := map[string]bool{}
	headerBytes := 0
	for name, values := range r.Header {
		lower := strings.ToLower(name)
		if len(values) != 1 || len(name) == 0 || len(name) > 128 || seen[lower] || len(values[0]) > 8192 || strings.ContainsAny(values[0], "\r\n\x00") {
			playerFail(w, auth.ErrInvalid)
			return
		}
		for _, c := range name {
			if !(c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= '0' && c <= '9' || strings.ContainsRune("!#$%&'*+-.^_`|~", c)) {
				playerFail(w, auth.ErrInvalid)
				return
			}
		}
		for _, c := range values[0] {
			if c < 32 && c != '\t' || c == 127 {
				playerFail(w, auth.ErrInvalid)
				return
			}
		}
		seen[lower] = true
		headerBytes += len(name) + len(values[0])
		if len(seen) > 64 || headerBytes > 32768 {
			playerFail(w, auth.ErrInvalid)
			return
		}
	}
	origin := r.Header.Get("Origin")
	fetch := r.Header.Get("Sec-Fetch-Site")
	if origin != "" && origin != base.origin || fetch != "" && fetch != "same-origin" && fetch != "none" {
		playerFail(w, auth.ErrDenied)
		return
	}
	if action == "" {
		playerFail(w, auth.ErrInvalid)
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()
	cookie, e := credential(r)
	if e != nil {
		playerFail(w, e)
		return
	}
	network, _, e := net.SplitHostPort(r.RemoteAddr)
	if e != nil || len(network) > 256 {
		network = "unknown"
	}
	var body []byte
	if r.Method != http.MethodGet {
		if oneHeader(r, "Origin") != base.origin || oneHeader(r, "X-CSRF-Token") == "" {
			playerFail(w, auth.ErrDenied)
			return
		}
		media, params, e := mime.ParseMediaType(oneHeader(r, "Content-Type"))
		if e != nil || media != "application/json" || len(params) > 1 || len(params) == 1 && !strings.EqualFold(params["charset"], "utf-8") || len(r.Header.Values("Content-Encoding")) != 0 || oneHeader(r, "Idempotency-Key") == "" || r.ContentLength > player.MaxRequestBytes {
			playerFail(w, auth.ErrInvalid)
			return
		}
		controller := http.NewResponseController(w)
		if e = controller.SetReadDeadline(time.Now().Add(5 * time.Second)); e != nil {
			playerFail(w, auth.ErrUnavailable)
			return
		}
		defer func() { _ = controller.SetReadDeadline(time.Time{}) }()
		body, e = io.ReadAll(io.LimitReader(r.Body, player.MaxRequestBytes+1))
		if e != nil || len(body) > player.MaxRequestBytes {
			clear(body)
			playerFail(w, auth.ErrInvalid)
			return
		}
		defer clear(body)
	} else if r.ContentLength != 0 || len(r.TransferEncoding) != 0 {
		playerFail(w, auth.ErrInvalid)
		return
	}
	request, e := d.players.Decode(action, workspace, id, body)
	if e != nil {
		playerFail(w, e)
		return
	}
	out, e := d.players.Do(ctx, auth.RoomSecret(launch.CallerData{Credential: cookie, CSRF: oneHeader(r, "X-CSRF-Token"), IdempotencyKey: oneHeader(r, "Idempotency-Key"), Network: network}), request)
	if e != nil {
		playerFail(w, e)
	} else {
		send(w, out)
	}
}
