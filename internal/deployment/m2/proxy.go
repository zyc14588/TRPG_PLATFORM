// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package m2

import (
	"context"
	"crypto/tls"
	"io"
	"log"
	"net"
	"net/http"
	"net/http/httputil"
	"net/url"
	"os"
	"strings"
	"time"
)

type Proxy struct {
	handler   http.Handler
	root      *os.Root
	transport *http.Transport
}

func NewProxy(c Config) (*Proxy, error) {
	u, e := url.Parse(c.Upstream)
	origin, oe := url.Parse(c.Origin)
	if e != nil || oe != nil || u.Scheme != "https" || u.Host == "" || u.User != nil || u.Path != "" || u.RawQuery != "" || u.Fragment != "" || u.ForceQuery || origin.Scheme != "https" || origin.Host == "" || origin.User != nil || origin.Path != "" || origin.RawQuery != "" || origin.Fragment != "" || origin.ForceQuery {
		return nil, ErrConfiguration
	}
	tls, e := TLSConfig(c.TLS, "platformd", false)
	if e != nil {
		return nil, e
	}
	tr := &http.Transport{Proxy: nil, TLSClientConfig: tls, DisableCompression: true, MaxConnsPerHost: 64, ResponseHeaderTimeout: 5 * time.Second, TLSHandshakeTimeout: 2 * time.Second, MaxResponseHeaderBytes: 16384}
	root, e := os.OpenRoot(c.StaticRoot)
	if e != nil {
		return nil, ErrConfiguration
	}
	f, e := root.Open("index.html")
	if e != nil {
		root.Close()
		return nil, ErrConfiguration
	}
	f.Close()
	reverse := &httputil.ReverseProxy{Transport: tr, ErrorLog: log.New(io.Discard, "", 0), ErrorHandler: func(w http.ResponseWriter, _ *http.Request, _ error) { http.Error(w, "Service unavailable", 503) }}
	reverse.Rewrite = func(p *httputil.ProxyRequest) {
		p.Out.URL.Scheme = u.Scheme
		p.Out.URL.Host = u.Host
		p.Out.Host = p.In.Host
		for _, h := range []string{"Forwarded", "X-Forwarded-For", "X-Forwarded-Host", "X-Forwarded-Proto", "X-Real-IP"} {
			p.Out.Header.Del(h)
		}
	}
	static := http.FileServerFS(root.FS())
	h := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.TLS == nil || r.Host != origin.Host {
			http.Error(w, "Request denied", 403)
			return
		}
		for _, header := range []string{"Forwarded", "X-Forwarded-For", "X-Forwarded-Host", "X-Forwarded-Proto", "X-Real-IP"} {
			if len(r.Header.Values(header)) != 0 {
				http.Error(w, "Request denied", 403)
				return
			}
		}
		if strings.HasPrefix(r.URL.Path, "/api/v1/") {
			reverse.ServeHTTP(w, r)
			return
		}
		if strings.HasPrefix(r.URL.Path, "/api/") || strings.HasPrefix(r.URL.Path, "/health/") || r.Method != "GET" && r.Method != "HEAD" {
			http.NotFound(w, r)
			return
		}
		w.Header().Set("Referrer-Policy", "no-referrer")
		w.Header().Set("X-Content-Type-Options", "nosniff")
		static.ServeHTTP(w, r)
	})
	return &Proxy{h, root, tr}, nil
}
func (p *Proxy) Handler() http.Handler { return p.handler }
func (p *Proxy) Close() error          { p.transport.CloseIdleConnections(); return p.root.Close() }
func ExternalTLS(files TLSFiles) (*tls.Config, error) {
	cert, e := ReadSecret(files.Certificate, 16384)
	if e != nil {
		return nil, e
	}
	defer clear(cert)
	key, e := ReadSecret(files.Key, 16384)
	if e != nil {
		return nil, e
	}
	defer clear(key)
	pair, e := tls.X509KeyPair(cert, key)
	if e != nil {
		return nil, ErrConfiguration
	}
	return &tls.Config{MinVersion: tls.VersionTLS13, Certificates: []tls.Certificate{pair}}, nil
}
func ProbeDaemon(ctx context.Context, c Config, peer string) error {
	address, method, path := c.DaemonAddress, "GET", "/health/live"
	host := ""
	switch peer {
	case "platformd":
	case "reverse-proxy":
		// The existing static entry proves the proxy's own listener/handler is
		// responsive without publishing a health route or consulting upstream.
		address, method, path = c.ExternalAddress, "HEAD", "/"
		origin, e := url.Parse(c.Origin)
		if e != nil || origin.Scheme != "https" || origin.Host == "" || origin.User != nil || origin.Path != "" || origin.RawQuery != "" || origin.Fragment != "" || origin.ForceQuery {
			return ErrConfiguration
		}
		host = origin.Host
	default:
		return ErrConfiguration
	}
	bindHost, port, e := net.SplitHostPort(address)
	if e != nil || port == "" {
		return ErrConfiguration
	}
	// A wildcard listen address is reached through this daemon's loopback.
	if bindHost == "" || bindHost == "0.0.0.0" {
		bindHost = "127.0.0.1"
	} else if bindHost == "::" {
		bindHost = "::1"
	}
	tls, e := TLSConfig(c.TLS, peer, false)
	if e != nil {
		return e
	}
	client := &http.Client{Transport: &http.Transport{Proxy: nil, TLSClientConfig: tls, DisableCompression: true, TLSHandshakeTimeout: 2 * time.Second, ResponseHeaderTimeout: 2 * time.Second, MaxResponseHeaderBytes: 16384}, Timeout: 2 * time.Second, CheckRedirect: func(*http.Request, []*http.Request) error { return ErrPrivate }}
	defer client.CloseIdleConnections()
	r, e := http.NewRequestWithContext(ctx, method, "https://"+net.JoinHostPort(bindHost, port)+path, nil)
	if e != nil {
		return ErrPrivate
	}
	if host != "" {
		r.Host = host
	}
	reply, e := client.Do(r)
	if e != nil {
		return ErrPrivate
	}
	defer reply.Body.Close()
	if reply.StatusCode != 200 {
		return ErrPrivate
	}
	return nil
}
