// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package m2

import (
	"context"
	"crypto/tls"
	"errors"
	"io"
	"log"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"sync"
	"time"
)

type Service struct {
	server   *http.Server
	listener net.Listener
	path     string
	info     os.FileInfo
	once     sync.Once
	done     chan struct{}
	err      error
	lock     *os.File
}
type peerListener struct {
	*net.UnixListener
	uid uint32
}

func (p *peerListener) Accept() (net.Conn, error) {
	for {
		c, e := p.AcceptUnix()
		if e != nil {
			return nil, e
		}
		if checkPeer(c, p.uid) != nil {
			_ = c.Close()
			continue
		}
		return c, nil
	}
}
func ServeUnix(ctx context.Context, path string, config *tls.Config, uid uint32, h http.Handler) (*Service, error) {
	if RequireLinux() != nil || ctx == nil || config == nil || h == nil || !filepath.IsAbs(path) {
		return nil, ErrConfiguration
	}
	dir, e := os.Lstat(filepath.Dir(path))
	if e != nil || !dir.IsDir() || dir.Mode().Perm()&0077 != 0 {
		return nil, ErrConfiguration
	}
	lock, e := ownSocket(path)
	if e != nil {
		return nil, e
	}
	ok := false
	defer func() {
		if !ok {
			releaseSocket(lock)
		}
	}()
	l, e := net.ListenUnix("unix", &net.UnixAddr{Name: path, Net: "unix"})
	if e != nil {
		return nil, ErrPrivate
	}
	l.SetUnlinkOnClose(false)
	if e = os.Chmod(path, 0600); e != nil {
		_ = l.Close()
		return nil, ErrPrivate
	}
	info, e := os.Lstat(path)
	if e != nil {
		_ = l.Close()
		return nil, ErrPrivate
	}
	s := &Service{path: path, info: info, done: make(chan struct{}), lock: lock}
	s.listener = tls.NewListener(&peerListener{l, uid}, config)
	s.server = &http.Server{Handler: h, ErrorLog: log.New(io.Discard, "", 0), ReadHeaderTimeout: 2 * time.Second, ReadTimeout: 5 * time.Second, WriteTimeout: 5 * time.Second, IdleTimeout: 5 * time.Second, MaxHeaderBytes: 8192, BaseContext: func(net.Listener) context.Context { return ctx }}
	go func() { s.err = s.server.Serve(s.listener); close(s.done) }()
	ok = true
	return s, nil
}
func (s *Service) Close(ctx context.Context) error {
	if s == nil {
		return nil
	}
	s.once.Do(func() {
		if e := s.server.Shutdown(ctx); e != nil {
			_ = s.server.Close()
		}
		<-s.done
		if i, e := os.Lstat(s.path); e == nil && os.SameFile(i, s.info) {
			_ = os.Remove(s.path)
		}
		releaseSocket(s.lock)
	})
	if s.err != nil && !errors.Is(s.err, http.ErrServerClosed) {
		return ErrPrivate
	}
	return nil
}
func UnixClient(path string, config *tls.Config, uid uint32) (*http.Client, error) {
	if RequireLinux() != nil || !filepath.IsAbs(path) || config == nil {
		return nil, ErrConfiguration
	}
	tr := &http.Transport{Proxy: nil, DisableCompression: true, ForceAttemptHTTP2: false, MaxConnsPerHost: 4, MaxIdleConns: 4, MaxIdleConnsPerHost: 4, IdleConnTimeout: 5 * time.Second, ResponseHeaderTimeout: 4 * time.Second, TLSClientConfig: config, MaxResponseHeaderBytes: 8192}
	tr.DialContext = func(ctx context.Context, network, address string) (net.Conn, error) {
		if network != "tcp" || address != config.ServerName+":443" {
			return nil, ErrPrivate
		}
		c, e := (&net.Dialer{Timeout: 2 * time.Second}).DialContext(ctx, "unix", path)
		if e != nil {
			return nil, ErrPrivate
		}
		uc, ok := c.(*net.UnixConn)
		if !ok || checkPeer(uc, uid) != nil {
			_ = c.Close()
			return nil, ErrPrivate
		}
		return c, nil
	}
	return &http.Client{Transport: tr, Timeout: 5 * time.Second, CheckRedirect: func(*http.Request, []*http.Request) error { return ErrPrivate }}, nil
}
func privateRequest(ctx context.Context, c *http.Client, peer, path string, body io.Reader) (*http.Response, error) {
	r, e := http.NewRequestWithContext(ctx, http.MethodPost, "https://"+peer+path, body)
	if e != nil {
		return nil, ErrPrivate
	}
	r.Header.Set("Content-Type", "application/json")
	reply, e := c.Do(r)
	if e != nil {
		return nil, ErrPrivate
	}
	if reply.StatusCode != 200 || reply.Header.Get("Content-Encoding") != "" {
		reply.Body.Close()
		return nil, ErrPrivate
	}
	return reply, nil
}
func privateReply(w http.ResponseWriter, v any) {
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("Content-Type", "application/json")
	b, e := encode(v)
	if e != nil {
		http.Error(w, "private service unavailable", 503)
		return
	}
	defer clear(b)
	_, _ = w.Write(b)
}
func privateError(w http.ResponseWriter) {
	http.Error(w, "private service unavailable", http.StatusServiceUnavailable)
}
func privateMethod(w http.ResponseWriter, r *http.Request) bool {
	if r.Method != "POST" || r.URL.RawQuery != "" || r.Header.Get("Content-Type") != "application/json" {
		privateError(w)
		return false
	}
	return true
}
