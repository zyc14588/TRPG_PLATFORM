// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package httpapi

import (
	"context"
	"fmt"
	"io"
	"mime"
	"net"
	"net/http"
	"strings"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
)

type RoomHandler struct{ data **roomHandlerData }
type roomHandlerData struct {
	base  *Handler
	rooms *room.Service
}

func NewRoomHandler(service *room.Service, origin string) (*RoomHandler, error) {
	if service == nil {
		return nil, auth.ErrInvalid
	}
	base, e := NewHandler(service.Authentication(), origin)
	if e != nil {
		return nil, e
	}
	d := &roomHandlerData{base: base, rooms: service}
	return &RoomHandler{data: &d}, nil
}
func (*RoomHandler) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private room HTTP handler>")
}
func (*RoomHandler) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func roomRoute(method, path string) (action, workspace, id, resource string, owned bool) {
	p := strings.Split(strings.TrimPrefix(path, "/"), "/")
	if len(p) < 3 || p[0] != "api" || p[1] != "v1" {
		return
	}
	if p[2] == "room-admissions" {
		owned = true
		if len(p) == 3 && method == http.MethodPost {
			action = "request_admission"
		}
		if len(p) == 4 && method == http.MethodGet {
			action = "get_admission"
			resource = p[3]
		}
		if len(p) == 5 && p[4] == "guest-token" && method == http.MethodPost {
			action = "guest_token"
			resource = p[3]
		}
		return
	}
	if len(p) < 5 || p[2] != "workspaces" || p[4] != "rooms" {
		return
	}
	owned = true
	workspace = p[3]
	if len(p) == 5 && method == http.MethodPost {
		action = "create_room"
		return
	}
	if len(p) < 6 {
		return
	}
	id = p[5]
	if len(p) == 6 {
		switch method {
		case http.MethodGet:
			action = "get_room"
		case http.MethodDelete:
			action = "close_room"
		}
		return
	}
	if len(p) == 7 {
		switch p[6] {
		case "invitations":
			if method == http.MethodPost {
				action = "create_invite"
			}
		case "admissions":
			if method == http.MethodGet {
				action = "list_admissions"
			}
		case "leave":
			if method == http.MethodPost {
				action = "leave_room"
			}
		}
		return
	}
	resource = p[7]
	if len(p) == 8 && method == http.MethodDelete {
		switch p[6] {
		case "invitations":
			action = "revoke_invite"
		case "participants":
			action = "kick_participant"
		}
	}
	if len(p) == 9 {
		if p[6] == "admissions" && p[8] == "decision" && method == http.MethodPost {
			action = "decide_admission"
		}
		if p[6] == "participants" && p[8] == "role" && method == http.MethodPut {
			action = "set_role"
		}
	}
	return
}
func (h *RoomHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if h == nil || h.data == nil || *h.data == nil {
		fail(w, auth.ErrUnavailable)
		return
	}
	d := *h.data
	action, workspace, id, resource, owned := roomRoute(r.Method, r.URL.Path)
	if !owned {
		d.base.ServeHTTP(w, r)
		return
	}
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("X-Content-Type-Options", "nosniff")
	w.Header().Set("Referrer-Policy", "no-referrer")
	base := *d.base.data
	if r.TLS == nil || r.Host != base.host || r.URL.RawQuery != "" || r.URL.RawPath != "" || r.Header.Get("Authorization") != "" || r.Header.Get("Sec-Fetch-Site") == "cross-site" || len(r.Header.Values("Origin")) > 1 || r.Header.Get("Origin") != "" && r.Header.Get("Origin") != base.origin {
		fail(w, auth.ErrDenied)
		return
	}
	if action == "" {
		fail(w, auth.ErrInvalid)
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()
	cookie, e := credential(r)
	if e != nil {
		fail(w, e)
		return
	}
	network, _, e := net.SplitHostPort(r.RemoteAddr)
	if e != nil || len(network) > 256 {
		network = "unknown"
	}
	var body []byte
	if r.Method != http.MethodGet {
		if oneHeader(r, "Origin") != base.origin || oneHeader(r, "X-CSRF-Token") == "" {
			fail(w, auth.ErrDenied)
			return
		}
		typeName, parameters, e := mime.ParseMediaType(oneHeader(r, "Content-Type"))
		if e != nil || typeName != "application/json" || len(parameters) > 1 || len(parameters) == 1 && !strings.EqualFold(parameters["charset"], "utf-8") || r.Header.Get("Content-Encoding") != "" {
			fail(w, auth.ErrInvalid)
			return
		}
		if e = http.NewResponseController(w).SetReadDeadline(time.Now().Add(5 * time.Second)); e != nil {
			fail(w, auth.ErrUnavailable)
			return
		}
		body, e = io.ReadAll(io.LimitReader(r.Body, 16385))
		if e != nil || len(body) > 16384 {
			clear(body)
			fail(w, auth.ErrInvalid)
			return
		}
		defer clear(body)
	}
	request, e := d.rooms.Decode(action, workspace, id, resource, body)
	if e != nil {
		fail(w, e)
		return
	}
	out, e := d.rooms.Do(ctx, cookie, oneHeader(r, "X-CSRF-Token"), oneHeader(r, "Idempotency-Key"), network, request)
	if e != nil {
		fail(w, e)
	} else {
		send(w, out)
	}
}
