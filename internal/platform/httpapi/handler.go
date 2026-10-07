// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package httpapi implements the approved HTTPS authentication transport.
package httpapi

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"mime"
	"net"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

const SessionCookie = "__Host-trpg_session"
const PreauthCookie = "__Host-trpg_preauth"

type Handler struct{ data **handlerData }
type handlerData struct {
	service      *auth.Service
	origin, host string
}

func NewHandler(service *auth.Service, origin string) (*Handler, error) {
	u, e := url.Parse(origin)
	if e != nil || u.Scheme != "https" || u.Host == "" || u.User != nil || u.Path != "" || u.RawQuery != "" || u.Fragment != "" || u.Opaque != "" || origin != "https://"+u.Host || service == nil {
		return nil, auth.ErrInvalid
	}
	d := &handlerData{service: service, origin: origin, host: u.Host}
	return &Handler{data: &d}, nil
}
func (*Handler) String() string             { return "<platform HTTP handler>" }
func (*Handler) GoString() string           { return "<platform HTTP handler>" }
func (*Handler) Format(f fmt.State, _ rune) { _, _ = io.WriteString(f, "<platform HTTP handler>") }

func credential(r *http.Request) (auth.BrowserCredential, error) {
	var session, preauth string
	seenSession, seenPreauth := false, false
	for _, c := range r.Cookies() {
		switch c.Name {
		case SessionCookie:
			if seenSession {
				return auth.BrowserCredential{}, auth.ErrInvalid
			}
			seenSession = true
			session = c.Value
		case PreauthCookie:
			if seenPreauth {
				return auth.BrowserCredential{}, auth.ErrInvalid
			}
			seenPreauth = true
			preauth = c.Value
		}
	}
	if seenSession {
		return auth.BrowserCookie(session), nil
	}
	if seenPreauth {
		return auth.BrowserCookie(preauth), nil
	}
	return auth.BrowserCredential{}, nil
}
func oneHeader(r *http.Request, key string) string {
	v := r.Header.Values(key)
	if len(v) != 1 {
		return ""
	}
	return v[0]
}
func clearCookie(w http.ResponseWriter, name string) {
	http.SetCookie(w, &http.Cookie{Name: name, Value: "", Path: "/", Secure: true, HttpOnly: true, SameSite: http.SameSiteStrictMode, MaxAge: -1, Expires: time.Unix(1, 0)})
}
func send(w http.ResponseWriter, out auth.Outcome) {
	v := out.StorageValue()
	if v.Cookie.StorageValue() != "" {
		name := SessionCookie
		if v.CookieKind == "preauth" {
			name = PreauthCookie
			clearCookie(w, SessionCookie)
		} else {
			clearCookie(w, PreauthCookie)
		}
		http.SetCookie(w, &http.Cookie{Name: name, Value: v.Cookie.StorageValue(), Path: "/", Secure: true, HttpOnly: true, SameSite: http.SameSiteStrictMode, Expires: v.ExpiresAt})
	}
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write(v.Body)
}
func fail(w http.ResponseWriter, e error) {
	e = auth.SafeError(e)
	status := http.StatusServiceUnavailable
	message := "Service unavailable"
	switch e {
	case auth.ErrInvalid:
		status = 400
		message = "Invalid request"
	case auth.ErrUnauthenticated:
		status = 401
		message = "Authentication required"
	case auth.ErrDenied:
		status = 403
		message = "Request denied"
	case auth.ErrConflict:
		status = 409
		message = "Request conflicts"
	case auth.ErrClaimRequired:
		status = 409
		message = "Guest claim required"
	case auth.ErrRateLimited:
		status = 429
		message = "Request limit reached"
	case auth.ErrOutcomeUnknown:
		message = "Commit outcome unknown"
	default:
		e = auth.ErrUnavailable
	}
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(map[string]any{"schema_version": 1, "request_id": "response", "error": map[string]any{"code": e.Error(), "message": message}})
}
func (h *Handler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("X-Content-Type-Options", "nosniff")
	w.Header().Set("Referrer-Policy", "no-referrer")
	if h == nil || h.data == nil || *h.data == nil {
		fail(w, auth.ErrUnavailable)
		return
	}
	d := *h.data
	if r.TLS == nil || r.Host != d.host || r.URL.RawQuery != "" || r.URL.RawPath != "" || r.Header.Get("Authorization") != "" {
		fail(w, auth.ErrDenied)
		return
	}
	if r.Header.Get("Sec-Fetch-Site") == "cross-site" || len(r.Header.Values("Origin")) > 1 || r.Header.Get("Origin") != "" && r.Header.Get("Origin") != d.origin {
		fail(w, auth.ErrDenied)
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()
	if r.Method == http.MethodGet && r.URL.Path == "/healthz" {
		ready := d.service.Ready(ctx)
		status := "ready"
		if !ready {
			status = "not_ready"
			w.WriteHeader(503)
		}
		_ = json.NewEncoder(w).Encode(map[string]any{"schema_version": 1, "status": status})
		return
	}
	cookie, e := credential(r)
	if e != nil {
		fail(w, e)
		return
	}
	network, _, e := net.SplitHostPort(r.RemoteAddr)
	if e != nil {
		network = "unknown"
	}
	if r.Method == http.MethodGet && r.URL.Path == "/api/v1/auth/context" {
		out, e := d.service.Context(ctx, cookie, network)
		if e != nil {
			fail(w, e)
		} else {
			send(w, out)
		}
		return
	}
	parts := strings.Split(strings.TrimPrefix(r.URL.Path, "/"), "/")
	workspace, account := "", ""
	action := ""
	if len(parts) == 4 && parts[0] == "api" && parts[1] == "v1" && parts[2] == "workspaces" {
		workspace = parts[3]
		if r.Method == http.MethodGet {
			out, e := d.service.Workspace(ctx, cookie, workspace)
			if e != nil {
				fail(w, e)
			} else {
				send(w, out)
			}
			return
		}
	}
	if r.Method == http.MethodPost {
		switch r.URL.Path {
		case "/api/v1/auth/register":
			action = "register"
		case "/api/v1/auth/login":
			action = "login"
		case "/api/v1/auth/logout":
			action = "logout"
		case "/api/v1/auth/guest/exchange":
			action = "exchange"
		case "/api/v1/auth/guest/claim":
			action = "claim"
		case "/api/v1/workspaces":
			action = "create_workspace"
		}
	}
	if len(parts) == 6 && parts[0] == "api" && parts[1] == "v1" && parts[2] == "workspaces" && parts[4] == "members" {
		workspace, account = parts[3], parts[5]
		if r.Method == http.MethodPut {
			action = "set_member"
		}
		if r.Method == http.MethodDelete {
			action = "remove_member"
		}
	}
	if action == "" {
		fail(w, auth.ErrInvalid)
		return
	}
	if oneHeader(r, "Origin") != d.origin || oneHeader(r, "X-CSRF-Token") == "" {
		fail(w, auth.ErrDenied)
		return
	}
	typeName, parameters, e := mime.ParseMediaType(oneHeader(r, "Content-Type"))
	if e != nil || typeName != "application/json" || len(parameters) > 1 || len(parameters) == 1 && !strings.EqualFold(parameters["charset"], "utf-8") || r.Header.Get("Content-Encoding") != "" {
		fail(w, auth.ErrInvalid)
		return
	}
	body, e := io.ReadAll(io.LimitReader(r.Body, 16385))
	if e != nil || len(body) > 16384 {
		clear(body)
		fail(w, auth.ErrInvalid)
		return
	}
	defer clear(body)
	request, e := d.service.Decode(action, workspace, account, body)
	if e != nil {
		fail(w, e)
		return
	}
	out, e := d.service.Mutate(ctx, cookie, oneHeader(r, "X-CSRF-Token"), oneHeader(r, "Idempotency-Key"), network, request)
	if e != nil {
		fail(w, e)
	} else {
		send(w, out)
	}
}
