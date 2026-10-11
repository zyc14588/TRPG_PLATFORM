// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package m2

import (
	"context"
	"io"
	"log"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"sync/atomic"
	"testing"
	"time"
)

func TestProxyPreservesRealBrowserGuardsAndCancellationOverMutualTLS(t *testing.T) {
	dir := privateDirectory(t)
	files := pkiFixture(t, dir, "platformd", "reverse-proxy")
	static := filepath.Join(dir, "static")
	if e := os.Mkdir(static, 0700); e != nil {
		t.Fatal(e)
	}
	if e := os.WriteFile(filepath.Join(static, "index.html"), []byte("unchanged static fixture"), 0400); e != nil {
		t.Fatal(e)
	}
	secret := filepath.Join(dir, "private")
	if e := os.WriteFile(secret, []byte("outside root marker"), 0400); e != nil {
		t.Fatal(e)
	}
	if e := os.Symlink(secret, filepath.Join(static, "outside")); e != nil {
		t.Fatal(e)
	}
	var calls atomic.Uint64
	entered, cancelled := make(chan struct{}, 1), make(chan struct{}, 1)
	upstream := httptest.NewUnstartedServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		if r.URL.Path == "/api/v1/wait" {
			entered <- struct{}{}
			<-r.Context().Done()
			cancelled <- struct{}{}
			return
		}
		cookie, e := r.Cookie("actual-session-fixture")
		if e != nil || cookie.Value != "owned-cookie" || r.Header.Get("X-CSRF-Token") != "owned-csrf" || r.Header.Get("Idempotency-Key") != "owned-key" || r.Header.Get("Origin") != "https://"+r.Host || r.Header.Get("Sec-Fetch-Site") != "same-origin" || len(r.TLS.PeerCertificates) != 1 || r.TLS.PeerCertificates[0].VerifyHostname("reverse-proxy") != nil {
			t.Error("browser guard or client role changed")
			w.WriteHeader(403)
			return
		}
		for _, h := range []string{"Forwarded", "X-Forwarded-For", "X-Forwarded-Host", "X-Forwarded-Proto", "X-Real-IP"} {
			if r.Header.Get(h) != "" {
				t.Error("proxy issued forwarding identity")
			}
		}
		_, _ = io.WriteString(w, "original authenticated route")
	}))
	serverTLS, e := TLSConfig(files["platformd"], "reverse-proxy", true)
	if e != nil {
		t.Fatal(e)
	}
	upstream.TLS = serverTLS
	upstream.Config.ErrorLog = log.New(io.Discard, "", 0)
	upstream.StartTLS()
	defer upstream.Close()
	public := httptest.NewUnstartedServer(nil)
	origin := "https://" + public.Listener.Addr().String()
	c := Config{Origin: origin, Upstream: upstream.URL, StaticRoot: static, TLS: files["reverse-proxy"]}
	p, e := NewProxy(c)
	if e != nil {
		t.Fatal(e)
	}
	defer p.Close()
	public.Config.Handler = p.Handler()
	public.Config.ErrorLog = log.New(io.Discard, "", 0)
	public.TLS, e = ExternalTLS(files["reverse-proxy"])
	if e != nil {
		t.Fatal(e)
	}
	public.StartTLS()
	defer public.Close()
	client := public.Client()
	client.Transport.(*http.Transport).TLSClientConfig.ServerName = "reverse-proxy"
	client.Timeout = 2 * time.Second
	request := func(path string, mutate func(*http.Request)) (int, string) {
		t.Helper()
		r, e := http.NewRequest("GET", origin+path, nil)
		if e != nil {
			t.Fatal(e)
		}
		r.AddCookie(&http.Cookie{Name: "actual-session-fixture", Value: "owned-cookie"})
		r.Header.Set("Origin", origin)
		r.Header.Set("X-CSRF-Token", "owned-csrf")
		r.Header.Set("Idempotency-Key", "owned-key")
		r.Header.Set("Sec-Fetch-Site", "same-origin")
		if mutate != nil {
			mutate(r)
		}
		response, e := client.Do(r)
		if e != nil {
			t.Fatal(e)
		}
		defer response.Body.Close()
		body, e := io.ReadAll(io.LimitReader(response.Body, 4096))
		if e != nil {
			t.Fatal(e)
		}
		return response.StatusCode, string(body)
	}
	if status, body := request("/api/v1/current", nil); status != 200 || body != "original authenticated route" || calls.Load() != 1 {
		t.Fatal("mutual TLS proxy changed original request", status)
	}
	for _, h := range []string{"Forwarded", "X-Forwarded-For", "X-Forwarded-Host", "X-Forwarded-Proto", "X-Real-IP"} {
		if status, _ := request("/api/v1/current", func(r *http.Request) { r.Header.Set(h, "forged") }); status != 403 || calls.Load() != 1 {
			t.Fatal("forged forwarding crossed boundary", h)
		}
	}
	if status, _ := request("/api/v1/current", func(r *http.Request) { r.Host = "other.invalid" }); status != 403 || calls.Load() != 1 {
		t.Fatal("unconfigured Host crossed boundary")
	}
	for _, path := range []string{"/health/ready", "/api/unversioned"} {
		if status, _ := request(path, nil); status != 404 || calls.Load() != 1 {
			t.Fatal("private path published", path)
		}
	}
	if status, body := request("/", nil); status != 200 || body != "unchanged static fixture" {
		t.Fatal("static artifact changed")
	}
	if _, body := request("/outside", nil); strings.Contains(body, "outside root marker") {
		t.Fatal("static file server escaped its root")
	}
	plain := httptest.NewRecorder()
	plaintext := httptest.NewRequest("GET", origin+"/api/v1/current", nil)
	plaintext.TLS = nil
	p.Handler().ServeHTTP(plain, plaintext)
	if plain.Code != 403 || calls.Load() != 1 {
		t.Fatal("plaintext forwarded")
	}
	ctx, cancel := context.WithCancel(context.Background())
	r, e := http.NewRequestWithContext(ctx, "GET", origin+"/api/v1/wait", nil)
	if e != nil {
		t.Fatal(e)
	}
	finished := make(chan error, 1)
	go func() {
		reply, e := client.Do(r)
		if reply != nil {
			reply.Body.Close()
		}
		finished <- e
	}()
	select {
	case <-entered:
	case <-time.After(time.Second):
		cancel()
		t.Fatal("real upstream request did not enter")
	}
	cancel()
	select {
	case e = <-finished:
		if e == nil {
			t.Fatal("cancellation returned success")
		}
	case <-time.After(time.Second):
		t.Fatal("browser cancellation was not joined")
	}
	select {
	case <-cancelled:
	case <-time.After(time.Second):
		t.Fatal("upstream I/O survived browser cancellation")
	}
}

func TestDaemonHealthUsesOwnLivenessWithoutBackendReadiness(t *testing.T) {
	dir := privateDirectory(t)
	files := pkiFixture(t, dir, "platformd", "reverse-proxy")
	var live, ready atomic.Uint64
	var dependencyReady atomic.Bool
	upstream := httptest.NewUnstartedServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/health/live":
			live.Add(1)
		case "/health/ready":
			ready.Add(1)
			if !dependencyReady.Load() {
				w.WriteHeader(http.StatusServiceUnavailable)
			}
		default:
			t.Error("health reached a non-health upstream route")
			w.WriteHeader(http.StatusNotFound)
		}
	}))
	var e error
	upstream.TLS, e = TLSConfig(files["platformd"], "reverse-proxy|platformd", true)
	if e != nil {
		t.Fatal(e)
	}
	upstream.Config.ErrorLog = log.New(io.Discard, "", 0)
	upstream.StartTLS()
	defer upstream.Close()
	platform := Config{DaemonAddress: upstream.Listener.Addr().String(), TLS: files["platformd"]}
	if e = ProbeDaemon(context.Background(), platform, "platformd"); e != nil || live.Load() != 1 || ready.Load() != 0 {
		t.Fatal("live platform was coupled to dependent readiness", e)
	}
	static := filepath.Join(dir, "static")
	if e = os.Mkdir(static, 0700); e != nil {
		t.Fatal(e)
	}
	if e = os.WriteFile(filepath.Join(static, "index.html"), []byte("unchanged static fixture"), 0400); e != nil {
		t.Fatal(e)
	}
	public := httptest.NewUnstartedServer(nil)
	c := Config{Origin: "https://" + public.Listener.Addr().String(), Upstream: upstream.URL, ExternalAddress: public.Listener.Addr().String(), StaticRoot: static, TLS: files["reverse-proxy"]}
	p, e := NewProxy(c)
	if e != nil {
		t.Fatal(e)
	}
	defer p.Close()
	public.Config.Handler = p.Handler()
	public.Config.ErrorLog = log.New(io.Discard, "", 0)
	public.TLS, e = ExternalTLS(c.TLS)
	if e != nil {
		t.Fatal(e)
	}
	public.StartTLS()
	defer public.Close()
	// The actual static/Host/TLS path responds even when its upstream is unready.
	if e = ProbeDaemon(context.Background(), c, "reverse-proxy"); e != nil || ready.Load() != 0 || live.Load() != 1 {
		t.Fatal("live proxy probed or required upstream readiness", e)
	}
	badHost := c
	badHost.Origin = "https://wrong.invalid"
	if e = ProbeDaemon(context.Background(), badHost, "reverse-proxy"); e == nil {
		t.Fatal("health bypassed configured Host guard")
	}
	wrongPeer := c
	wrongPeer.ExternalAddress = upstream.Listener.Addr().String()
	if e = ProbeDaemon(context.Background(), wrongPeer, "reverse-proxy"); e == nil {
		t.Fatal("health accepted a different service TLS identity")
	}
	dependencyReady.Store(true)
	public.Close()
	if e = ProbeDaemon(context.Background(), c, "reverse-proxy"); e == nil || ready.Load() != 0 {
		t.Fatal("healthy upstream concealed an unavailable proxy listener", e)
	}
	upstream.Close()
	if e = ProbeDaemon(context.Background(), platform, "platformd"); e == nil {
		t.Fatal("unavailable platform listener was alive")
	}
}

func TestDaemonHealthRejectsRedirectInvalidTLSAndUnresponsiveListener(t *testing.T) {
	dir := privateDirectory(t)
	files := pkiFixture(t, dir, "reverse-proxy")
	var destination atomic.Uint64
	for _, fault := range []string{"redirect", "untrusted-ca", "unresponsive", "cancelled", "wildcard-bind"} {
		t.Run(fault, func(t *testing.T) {
			server := httptest.NewUnstartedServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				if r.URL.Path != "/" {
					destination.Add(1)
					return
				}
				if fault == "redirect" {
					http.Redirect(w, r, "/destination", http.StatusFound)
				} else if fault == "unresponsive" {
					<-r.Context().Done()
				}
			}))
			server.Config.ErrorLog = log.New(io.Discard, "", 0)
			var e error
			server.TLS, e = ExternalTLS(files["reverse-proxy"])
			if e != nil {
				t.Fatal(e)
			}
			server.StartTLS()
			defer server.Close()
			c := Config{Origin: server.URL, ExternalAddress: server.Listener.Addr().String(), TLS: files["reverse-proxy"]}
			if fault == "untrusted-ca" {
				c.TLS.CA = pkiFixture(t, privateDirectory(t), "other")["other"].CA
			}
			if fault == "wildcard-bind" {
				c.ExternalAddress = strings.TrimPrefix(c.ExternalAddress, "127.0.0.1")
			}
			ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
			defer cancel()
			if fault == "cancelled" {
				cancel()
			}
			e = ProbeDaemon(ctx, c, "reverse-proxy")
			if (fault == "wildcard-bind") != (e == nil) || destination.Load() != 0 {
				t.Fatal("health lost its bounded authenticated own-listener behavior", e)
			}
		})
	}
	if e := ProbeDaemon(context.Background(), Config{}, "other"); e == nil {
		t.Fatal("arbitrary daemon role accepted")
	}
}
