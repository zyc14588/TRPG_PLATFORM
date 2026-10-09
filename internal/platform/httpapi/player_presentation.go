// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package httpapi

import (
	"context"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
	"io"
	"net/http"
	"strings"
	"time"
)

type PlayerPresentationHandler struct {
	base         *PlayerHandler
	presentation *player.PresentationService
}

func (*PlayerPresentationHandler) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private player presentation HTTP composition>")
}
func (*PlayerPresentationHandler) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }

// NewPlayerPresentationHandler is the exported production composition entry.
// The original fifteen routes continue through the real original handler.
func NewPlayerPresentationHandler(players *player.Service, rooms *room.Service, presentation *player.PresentationService, origin string) (*PlayerPresentationHandler, error) {
	if !presentation.UsesPlayers(players) || presentation.Authentication() != rooms.Authentication() {
		return nil, auth.ErrInvalid
	}
	base, e := NewPlayerHandler(players, rooms, origin)
	if e != nil {
		return nil, e
	}
	return &PlayerPresentationHandler{base, presentation}, nil
}
func playerPresentationRoute(method, path string) (w, id string, owned, valid bool) {
	p := strings.Split(path, "/")
	if len(p) < 7 || p[0] != "" || p[1] != "api" || p[2] != "v1" || p[3] != "workspaces" || p[5] != "games" && p[5] != "rooms" {
		return
	}
	if len(p) >= 8 && p[7] == "presentation" {
		owned = true
		w = p[4]
		id = p[6]
		valid = len(p) == 8 && method == http.MethodGet
	}
	return
}
func (h *PlayerPresentationHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if h == nil || h.base == nil || h.base.data == nil || *h.base.data == nil || h.presentation == nil || r == nil || r.URL == nil {
		fail(w, auth.ErrUnavailable)
		return
	}
	workspace, id, owned, valid := playerPresentationRoute(r.Method, r.URL.Path)
	if !owned {
		h.base.ServeHTTP(w, r)
		return
	}
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("X-Content-Type-Options", "nosniff")
	w.Header().Set("Referrer-Policy", "no-referrer")
	base := *(*(*h.base.data).base.data).base.data
	if r.TLS == nil || r.Host != base.host || r.URL.RawQuery != "" || r.URL.ForceQuery || r.URL.RawPath != "" || r.URL.User != nil || r.URL.Fragment != "" || r.URL.Opaque != "" {
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
		if lower == "authorization" {
			playerFail(w, auth.ErrDenied)
			return
		}
		if lower == "content-type" || lower == "content-encoding" || lower == "x-csrf-token" || lower == "idempotency-key" {
			playerFail(w, auth.ErrInvalid)
			return
		}
		seen[lower] = true
		headerBytes += len(name) + len(values[0])
		if len(seen) > 64 || headerBytes > 32768 {
			playerFail(w, auth.ErrInvalid)
			return
		}
	}
	origin, fetch := r.Header.Get("Origin"), r.Header.Get("Sec-Fetch-Site")
	if origin != "" && origin != base.origin || fetch != "" && fetch != "same-origin" && fetch != "none" {
		playerFail(w, auth.ErrDenied)
		return
	}
	if !valid || r.ContentLength != 0 || len(r.TransferEncoding) != 0 {
		playerFail(w, auth.ErrInvalid)
		return
	}
	if r.Body != nil && r.Body != http.NoBody {
		controller := http.NewResponseController(w)
		if controller.SetReadDeadline(time.Now().Add(5*time.Second)) != nil {
			playerFail(w, auth.ErrUnavailable)
			return
		}
		defer func() { _ = controller.SetReadDeadline(time.Time{}) }()
		var b [1]byte
		n, e := r.Body.Read(b[:])
		if n != 0 || e != io.EOF {
			playerFail(w, auth.ErrInvalid)
			return
		}
	}
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()
	cookie, e := credential(r)
	if e != nil {
		playerFail(w, e)
		return
	}
	caller := auth.RoomSecret(launch.CallerData{Credential: cookie})
	var out auth.Outcome
	if strings.Split(r.URL.Path, "/")[5] == "rooms" {
		out, e = h.presentation.ReadRoom(ctx, caller, workspace, id)
	} else {
		out, e = h.presentation.Read(ctx, caller, workspace, id)
	}
	if e != nil {
		playerFail(w, e)
		return
	}
	send(w, out)
}
