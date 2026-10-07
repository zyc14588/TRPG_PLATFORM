// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package httpapi

import (
	"bytes"
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

type observedStorage struct{ calls atomic.Int32 }

func (r *observedStorage) Transact(context.Context, func(auth.Transaction) error) error {
	r.calls.Add(1)
	return auth.ErrUnavailable
}
func transport(t *testing.T) (*Handler, *observedStorage) {
	t.Helper()
	schema, e := os.ReadFile("../../../schemas/platform/platform-auth-api-v1.schema.json")
	if e != nil {
		t.Fatal("schema fixture absent")
	}
	r := &observedStorage{}
	s, e := auth.NewService(r, bytes.Repeat([]byte{1}, 32), bytes.Repeat([]byte{2}, 32), schema, nil)
	if e != nil {
		t.Fatal(e)
	}
	h, e := NewHandler(s, "https://platform.example")
	if e != nil {
		t.Fatal(e)
	}
	return h, r
}
func post(body string) *http.Request {
	r := httptest.NewRequest(http.MethodPost, "https://platform.example/api/v1/auth/login", strings.NewReader(body))
	r.Header.Set("Content-Type", "application/json; charset=utf-8")
	r.Header.Set("Origin", "https://platform.example")
	r.Header.Set("X-CSRF-Token", strings.Repeat("A", 43))
	r.Header.Set("Idempotency-Key", "a-valid-key-123456")
	r.AddCookie(&http.Cookie{Name: PreauthCookie, Value: strings.Repeat("B", 43)})
	return r
}
func TestHTTPSOriginAndWireGuardsRunBeforeStorage(t *testing.T) {
	for _, test := range []struct {
		name   string
		change func(*http.Request)
		status int
	}{
		{"http", func(r *http.Request) { r.TLS = nil }, 403},
		{"wrong_host", func(r *http.Request) { r.Host = "attacker.example" }, 403},
		{"cross_origin", func(r *http.Request) { r.Header.Set("Origin", "https://attacker.example") }, 403},
		{"missing_origin", func(r *http.Request) { r.Header.Del("Origin") }, 403},
		{"duplicate_origin", func(r *http.Request) { r.Header.Add("Origin", "https://platform.example") }, 403},
		{"missing_csrf", func(r *http.Request) { r.Header.Del("X-CSRF-Token") }, 403},
		{"duplicate_csrf", func(r *http.Request) { r.Header.Add("X-CSRF-Token", strings.Repeat("A", 43)) }, 403},
		{"cross_site", func(r *http.Request) { r.Header.Set("Sec-Fetch-Site", "cross-site") }, 403},
		{"bearer", func(r *http.Request) { r.Header.Set("Authorization", "Bearer private-marker") }, 403},
		{"query_token", func(r *http.Request) { r.URL.RawQuery = "session_token=private-marker" }, 403},
		{"duplicate_cookie", func(r *http.Request) { r.AddCookie(&http.Cookie{Name: PreauthCookie, Value: "other"}) }, 400},
		{"wrong_type", func(r *http.Request) { r.Header.Set("Content-Type", "text/plain") }, 400},
		{"compressed", func(r *http.Request) { r.Header.Set("Content-Encoding", "gzip") }, 400},
		{"missing_idempotency", func(r *http.Request) { r.Header.Del("Idempotency-Key") }, 400},
		{"unknown_version", func(r *http.Request) {
			r.Body = ioBody(`{"schema_version":2,"login_name":"local_user","password":"private-marker"}`)
		}, 400},
		{"duplicate_field", func(r *http.Request) {
			r.Body = ioBody(`{"schema_version":2,"schema_version":1,"login_name":"local_user","password":"private-marker"}`)
		}, 400},
		{"identity_injection", func(r *http.Request) {
			r.Body = ioBody(`{"schema_version":1,"login_name":"local_user","password":"private-marker","account_id":"other"}`)
		}, 400},
		{"oversized", func(r *http.Request) { r.Body = ioBody(strings.Repeat("x", 16385)) }, 400},
	} {
		t.Run(test.name, func(t *testing.T) {
			h, storage := transport(t)
			r := post(`{"schema_version":1,"login_name":"local_user","password":"private-marker"}`)
			test.change(r)
			w := httptest.NewRecorder()
			h.ServeHTTP(boundedRecorder{w}, r)
			if w.Code != test.status || storage.calls.Load() != 0 {
				t.Fatalf("guard status=%d storage calls=%d", w.Code, storage.calls.Load())
			}
			if strings.Contains(w.Body.String(), "private-marker") || w.Header().Get("Access-Control-Allow-Origin") != "" {
				t.Fatal("private data or permissive CORS exposed")
			}
			var value map[string]any
			if json.Unmarshal(w.Body.Bytes(), &value) != nil || len(value) != 3 {
				t.Fatal("error envelope invalid")
			}
		})
	}
}

type bodyReader struct{ *strings.Reader }

func (bodyReader) Close() error  { return nil }
func ioBody(s string) bodyReader { return bodyReader{strings.NewReader(s)} }
func TestStorageFailureAndReadinessHaveSafeEnvelope(t *testing.T) {
	h, storage := transport(t)
	r := post(`{"schema_version":1,"login_name":"local_user","password":"private-marker"}`)
	w := httptest.NewRecorder()
	h.ServeHTTP(boundedRecorder{w}, r)
	if w.Code != 503 || storage.calls.Load() != 1 || strings.Contains(w.Body.String(), "private-marker") {
		t.Fatal("storage failure not bounded")
	}
	w = httptest.NewRecorder()
	h.ServeHTTP(boundedRecorder{w}, httptest.NewRequest(http.MethodGet, "https://platform.example/healthz", nil))
	if w.Code != 503 || strings.TrimSpace(w.Body.String()) != `{"schema_version":1,"status":"not_ready"}` {
		t.Fatal("readiness discloses diagnostics")
	}
}
func TestCookieAttributesAndClearing(t *testing.T) {
	w := httptest.NewRecorder()
	clearCookie(w, SessionCookie)
	c := w.Result().Cookies()
	if len(c) != 1 || !c[0].Secure || !c[0].HttpOnly || c[0].SameSite != http.SameSiteStrictMode || c[0].Path != "/" || c[0].Domain != "" || c[0].MaxAge != -1 {
		t.Fatal("host cookie attributes differ")
	}
}

// Pure transport mocks provide the capability of the real stdlib HTTPS writer.
type boundedRecorder struct{ *httptest.ResponseRecorder }

func (boundedRecorder) SetReadDeadline(time.Time) error { return nil }
func TestMissingReadDeadlineCapabilityFailsClosed(t *testing.T) {
	h, storage := transport(t)
	w := httptest.NewRecorder()
	h.ServeHTTP(w, post(`{"schema_version":1,"login_name":"local_user","password":"private-marker"}`))
	if w.Code != 503 || storage.calls.Load() != 0 {
		t.Fatal("unsupported body deadline did not fail closed")
	}
}
