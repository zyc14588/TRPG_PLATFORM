// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/gorilla/websocket"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/actor"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/persistence"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/realtime"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/session/testdata"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

// This explicit internal Linux M1 fixture command exercises the real daemon.
// Its operator file supplies synthetic credentials and all package permissions.
// It creates no accounts, rooms, campaigns or automatic seat transfer.
type fixtureConfig struct {
	DSN           string `json:"dsn"`
	Objects       string `json:"objects"`
	Staging       string `json:"staging"`
	Runner        string `json:"runner"`
	RunnerHash    string `json:"runner_hash"`
	Listen        string `json:"listen"`
	Workspace     string `json:"workspace"`
	Session       string `json:"session"`
	GMToken       string `json:"gm_token"`
	PlayerToken   string `json:"player_token"`
	CommitBarrier bool   `json:"commit_barrier,omitempty"`
}

func runFixture(ctx context.Context, args []string, out, errOut io.Writer) int {
	flags := flag.NewFlagSet("m1-fixture", flag.ContinueOnError)
	flags.SetOutput(errOut)
	path := flags.String("config", "", "private operator configuration")
	if flags.Parse(args) != nil || flags.NArg() != 0 || *path == "" {
		return 2
	}
	info, err := os.Stat(*path)
	if err != nil || !info.Mode().IsRegular() || info.Mode().Perm()&0077 != 0 {
		fmt.Fprintln(errOut, "FIXTURE_CONFIG_REJECTED")
		return 1
	}
	f, err := os.Open(*path)
	if err != nil {
		return 1
	}
	raw, err := io.ReadAll(io.LimitReader(f, 64<<10+1))
	f.Close()
	var cfg fixtureConfig
	if err != nil || checkpoint.StrictDecode(raw, &cfg, 64<<10) != nil || !store.ValidID(cfg.Workspace) || !store.ValidID(cfg.Session) || len(cfg.GMToken) < 24 || len(cfg.GMToken) > 256 || len(cfg.PlayerToken) < 24 || len(cfg.PlayerToken) > 256 || cfg.GMToken == cfg.PlayerToken {
		fmt.Fprintln(errOut, "FIXTURE_CONFIG_REJECTED")
		return 1
	}
	host, _, err := net.SplitHostPort(cfg.Listen)
	if err != nil || host != "127.0.0.1" {
		fmt.Fprintln(errOut, "FIXTURE_LOOPBACK_REQUIRED")
		return 1
	}
	for _, p := range []string{cfg.Objects, cfg.Staging, cfg.Runner} {
		if !filepath.IsAbs(p) {
			fmt.Fprintln(errOut, "FIXTURE_CONFIG_REJECTED")
			return 1
		}
	}
	binary, err := os.ReadFile(cfg.Runner)
	if err != nil || checkpoint.Hash(binary) != cfg.RunnerHash {
		fmt.Fprintln(errOut, "FIXTURE_RUNNER_REJECTED")
		return 1
	}
	for _, p := range []string{cfg.Objects, cfg.Staging} {
		if err = os.MkdirAll(p, 0700); err != nil {
			return 1
		}
		s, err := os.Stat(p)
		if err != nil || !s.IsDir() || s.Mode().Perm()&0077 != 0 {
			fmt.Fprintln(errOut, "FIXTURE_STORAGE_REJECTED")
			return 1
		}
	}
	if err = serveFixture(ctx, cfg, out); err != nil {
		fmt.Fprintln(errOut, "FIXTURE_START_OR_SERVE_FAILED")
		return 1
	}
	return 0
}
func serveFixture(ctx context.Context, cfg fixtureConfig, out io.Writer) error {
	startup, cancel := context.WithTimeout(ctx, 30*time.Second)
	defer cancel()
	runtime := install.RuntimeConfig{Runner: cfg.Runner, SHA256: cfg.RunnerHash, Limits: profile.DefaultLimits()}
	pkg, config, err := fixture.Build(runtime, "")
	if err != nil {
		return err
	}
	policy, err := install.NewPolicy(config)
	if err != nil {
		return err
	}
	credential := store.Credential("m1-fixture-install-operator-credential")
	access, err := store.NewAccess(map[store.Credential][]store.Membership{credential: {{Principal: "operator", Workspace: cfg.Workspace, Read: true, Install: true}}})
	if err != nil {
		return err
	}
	var logMu sync.Mutex
	observe := func(v install.Execution) error {
		raw, err := json.Marshal(v)
		if err != nil {
			return err
		}
		logMu.Lock()
		defer logMu.Unlock()
		_, err = fmt.Fprintf(out, "FIXTURE_EXECUTION %s\n", raw)
		return err
	}
	fault := func(_ context.Context, point string) error {
		if cfg.CommitBarrier && point == "after-commit" {
			logMu.Lock()
			_, err := fmt.Fprintln(out, "FIXTURE_COMMIT_BARRIER")
			logMu.Unlock()
			if err != nil {
				return err
			}
			<-ctx.Done()
			return ctx.Err()
		}
		return nil
	}
	services, err := openPackageServices(startup, cfg.DSN, cfg.Objects, install.Options{StagingRoot: cfg.Staging, Policy: policy, Access: access, Runtime: runtime, Observe: func(string) error { return nil }, Execution: observe}, sessionValidation{Audit: func(data.Audit) error { return nil }, Validate: func(context.Context, data.Commit) error { return nil }, Fault: fault})
	if err != nil {
		return err
	}
	defer services.Close()
	// Provisioning is explicit and confined to the operator-selected fixture DB.
	if err = services.installation.Bootstrap(startup); err != nil {
		return err
	}
	if err = services.installation.ProvisionWorkspace(startup, cfg.Workspace); err != nil {
		return err
	}
	if err = services.HostCommands.Bootstrap(startup); err != nil {
		return err
	}
	exported, err := pkg.Export()
	if err != nil {
		return err
	}
	if _, err = services.Installer.Install(startup, install.Request{Credential: credential, Workspace: cfg.Workspace, ID: "fixture-install", Root: install.Input{Archive: bytes.NewReader(exported.Bytes())}}); err != nil {
		return err
	}
	request := install.SessionRequest{Credential: credential, Workspace: cfg.Workspace, Session: cfg.Session, Root: string(pkg.ArtifactIdentity().Digest())}
	existing, err := services.HostCommands.InspectGraphSession(startup, cfg.Workspace, cfg.Session)
	if err != nil {
		return err
	}
	var binding data.Binding
	if existing.Sessions == 0 {
		s, err := services.Sessions.Create(startup, request)
		if err != nil {
			return err
		}
		binding = s.Binding
		if err = s.Close(); err != nil {
			return err
		}
	} else {
		g, err := services.Reader.LoadGraph(startup, credential, cfg.Workspace, request.Root, nil)
		if err != nil {
			return err
		}
		lockHash, err := g.Root().ExactLock().Digest()
		if err != nil {
			return err
		}
		binding = data.Binding{Workspace: cfg.Workspace, Session: cfg.Session, GraphHash: string(lockHash)}
	}
	seats := []command.FixtureSeat{}
	validate := func(v checkpoint.Value) error {
		if v.Kind != "table" || len(v.Table) != 1 || v.Table["delta"].Kind != "integer" {
			return command.ErrEnvelope
		}
		n, err := strconv.ParseInt(v.Table["delta"].Number, 10, 64)
		if err != nil || n < 1 || n > 1000 {
			return command.ErrEnvelope
		}
		return nil
	}
	for _, seat := range []string{"gm", "player"} {
		token := cfg.PlayerToken
		fields := []string{"counter"}
		types := map[string]func(checkpoint.Value) error{"increment": validate}
		if seat == "gm" {
			token = cfg.GMToken
			fields = append(fields, "secret")
			types["end"] = validate
			types["fail"] = validate
		}
		seats = append(seats, command.FixtureSeat{Credential: token, Binding: binding, Principal: seat, Seat: seat, Commands: types, Views: command.ViewPolicy{ViewFields: fields, EventFields: map[string][]string{fixture.PackageID + "/change": fields}, ScalarResult: true}})
	}
	authority, err := command.NewFixtureAuthority(seats)
	if err != nil {
		return err
	}
	hub, err := realtime.New(realtime.Options{Authority: authority, Capacity: 8, PerSession: 8, Queue: 8})
	if err != nil {
		return err
	}
	defer hub.Close()
	backend, err := persistence.New(persistence.Options{Factory: services.Sessions, Repository: services.HostCommands, Request: func(b data.Binding) (install.SessionRequest, error) {
		if b != binding {
			return install.SessionRequest{}, command.ErrDenied
		}
		return request, nil
	}, Time: 1000, Random: []int64{7}, ToolResults: []checkpoint.Value{checkpoint.Int(7)}})
	if err != nil {
		return err
	}
	registry, err := actor.New(ctx, actor.Options{Authority: authority, Hub: hub, Backend: backend, MaxSessions: 1, Mailbox: 8, Idle: time.Minute, CommandTimeout: 5 * time.Second})
	if err != nil {
		return err
	}
	defer registry.Close()
	transport := &fixtureTransport{cfg: cfg, authority: authority, hub: hub, registry: registry, ctx: ctx}
	defer func() {
		transport.mu.Lock()
		transport.closing = true
		transport.mu.Unlock()
		_ = registry.Close()
		transport.wg.Wait()
	}()
	mux := http.NewServeMux()
	mux.HandleFunc("GET /ready", func(w http.ResponseWriter, r *http.Request) {
		writeFixtureJSON(w, http.StatusOK, map[string]string{"status": "ready"})
	})
	mux.HandleFunc("POST /command", transport.submit)
	mux.HandleFunc("GET /stream", transport.stream)
	mux.HandleFunc("POST /sleep", transport.sleep)
	mux.HandleFunc("POST /revoke", transport.revoke)
	listener, err := net.Listen("tcp", cfg.Listen)
	if err != nil {
		return err
	}
	defer listener.Close()
	listener = &fixtureListener{Listener: listener, slots: make(chan struct{}, 32)}
	server := &http.Server{Handler: mux, ReadHeaderTimeout: 2 * time.Second, ReadTimeout: 10 * time.Second, WriteTimeout: 10 * time.Second, IdleTimeout: 20 * time.Second, MaxHeaderBytes: 8 << 10, BaseContext: func(net.Listener) context.Context { return ctx }}
	done := make(chan error, 1)
	go func() { done <- server.Serve(listener) }()
	logMu.Lock()
	_, logErr := fmt.Fprintf(out, "FIXTURE_READY http://%s\n", listener.Addr())
	logMu.Unlock()
	if logErr != nil {
		_ = server.Close()
		return logErr
	}
	select {
	case <-ctx.Done():
	case err = <-done:
		if !errors.Is(err, http.ErrServerClosed) {
			_ = registry.Close()
			return err
		}
	}
	transport.mu.Lock()
	transport.closing = true
	transport.mu.Unlock()
	_ = registry.Close()
	shutdown, stop := context.WithTimeout(context.Background(), 2*time.Second)
	defer stop()
	err = server.Shutdown(shutdown)
	transport.wg.Wait()
	return err
}

type fixtureTransport struct {
	cfg       fixtureConfig
	authority *command.Authority
	hub       *realtime.Hub
	registry  *actor.Registry
	ctx       context.Context
	wg        sync.WaitGroup
	mu        sync.Mutex
	closing   bool
}

func writeFixtureJSON(w http.ResponseWriter, status int, value any) {
	w.Header().Set("Content-Type", "application/json")
	w.Header().Set("Cache-Control", "no-store")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(value)
}
func fixtureFailure(w http.ResponseWriter, err error) {
	status, code := http.StatusServiceUnavailable, "SESSION_EXECUTION_FAILED"
	switch {
	case errors.Is(err, command.ErrDenied):
		status, code = http.StatusForbidden, "SESSION_SEAT_DENIED"
	case errors.Is(err, command.ErrEnvelope):
		status, code = http.StatusBadRequest, "SESSION_ENVELOPE_REJECTED"
	case errors.Is(err, actor.ErrBackpressure), errors.Is(err, realtime.ErrCapacity):
		status, code = http.StatusTooManyRequests, "SESSION_BACKPRESSURE"
	case errors.Is(err, data.ErrConflict):
		status, code = http.StatusConflict, "SESSION_VERSION_OR_ID_CONFLICT"
	case errors.Is(err, actor.ErrEnded):
		status, code = http.StatusGone, "SESSION_ENDED"
	}
	writeFixtureJSON(w, status, map[string]string{"error": code})
}
func (f *fixtureTransport) identity(r *http.Request) (command.Identity, error) {
	authorization := r.Header.Get("Authorization")
	if !strings.HasPrefix(authorization, "Bearer ") {
		return command.Identity{}, command.ErrDenied
	}
	return f.authority.Authenticate(strings.TrimPrefix(authorization, "Bearer "), f.cfg.Session, r.Header.Get("X-Fixture-Seat"))
}
func (f *fixtureTransport) submit(w http.ResponseWriter, r *http.Request) {
	i, err := f.identity(r)
	if err != nil {
		fixtureFailure(w, err)
		return
	}
	raw, err := io.ReadAll(http.MaxBytesReader(w, r.Body, 128<<10))
	if err != nil {
		fixtureFailure(w, command.ErrEnvelope)
		return
	}
	e, err := command.Decode(raw)
	if err != nil {
		fixtureFailure(w, err)
		return
	}
	receipt, err := f.registry.Submit(r.Context(), i, e)
	if err != nil {
		fixtureFailure(w, err)
		return
	}
	policy, err := f.authority.Policy(i)
	if err != nil {
		fixtureFailure(w, err)
		return
	}
	writeFixtureJSON(w, http.StatusOK, struct {
		Kind        string           `json:"kind"`
		ID          string           `json:"command_id"`
		Correlation string           `json:"correlation_id"`
		Version     uint64           `json:"state_version"`
		Cursor      uint64           `json:"event_cursor"`
		Replayed    bool             `json:"replayed"`
		Result      checkpoint.Value `json:"result"`
	}{"committed", e.CommandID, e.CorrelationID, receipt.Version, receipt.Cursor, receipt.Replayed, realtime.FilterResult(receipt.Result, policy)})
}
func (f *fixtureTransport) sleep(w http.ResponseWriter, r *http.Request) {
	i, err := f.identity(r)
	if err != nil || i.Seat() != "gm" {
		fixtureFailure(w, command.ErrDenied)
		return
	}
	if err = f.registry.Sleep(r.Context(), i); err != nil {
		fixtureFailure(w, err)
		return
	}
	writeFixtureJSON(w, http.StatusOK, map[string]string{"status": "sleeping"})
}
func (f *fixtureTransport) revoke(w http.ResponseWriter, r *http.Request) {
	i, err := f.identity(r)
	if err != nil || i.Seat() != "gm" {
		fixtureFailure(w, command.ErrDenied)
		return
	}
	if err = f.authority.Revoke(f.cfg.PlayerToken); err != nil {
		fixtureFailure(w, err)
		return
	}
	writeFixtureJSON(w, http.StatusOK, map[string]string{"status": "revoked"})
}
func (f *fixtureTransport) stream(w http.ResponseWriter, r *http.Request) {
	f.mu.Lock()
	if f.closing {
		f.mu.Unlock()
		fixtureFailure(w, actor.ErrClosed)
		return
	}
	f.wg.Add(1)
	f.mu.Unlock()
	defer f.wg.Done()
	i, err := f.identity(r)
	if err != nil {
		fixtureFailure(w, err)
		return
	}
	after, err := strconv.ParseUint(r.URL.Query().Get("after"), 10, 63)
	if err != nil {
		fixtureFailure(w, command.ErrEnvelope)
		return
	}
	connection, err := f.hub.Subscribe(i)
	if err != nil {
		fixtureFailure(w, err)
		return
	}
	defer connection.Close()
	if _, err = f.registry.Reconnect(r.Context(), i, after, connection); err != nil {
		fixtureFailure(w, err)
		return
	}
	upgrader := websocket.Upgrader{HandshakeTimeout: 2 * time.Second, ReadBufferSize: 1024, WriteBufferSize: 4096}
	socket, err := upgrader.Upgrade(w, r, nil)
	if err != nil {
		return
	}
	defer socket.Close()
	ctx, cancel := context.WithCancel(f.ctx)
	defer cancel()
	readerDone := make(chan struct{})
	socket.SetReadLimit(1024)
	_ = socket.SetReadDeadline(time.Now().Add(30 * time.Second))
	socket.SetPongHandler(func(string) error { return socket.SetReadDeadline(time.Now().Add(30 * time.Second)) })
	go func() {
		defer close(readerDone)
		defer cancel()
		for {
			if _, _, err := socket.ReadMessage(); err != nil {
				return
			}
		}
	}()
	defer func() { _ = socket.Close(); <-readerDone }()
	pingAt := time.Now().Add(10 * time.Second)
	for {
		wait, stop := context.WithTimeout(ctx, time.Second)
		frame, err := connection.Next(wait)
		stop()
		if errors.Is(err, context.DeadlineExceeded) {
			if time.Now().After(pingAt) {
				_ = socket.SetWriteDeadline(time.Now().Add(2 * time.Second))
				if err = socket.WriteMessage(websocket.PingMessage, nil); err != nil {
					return
				}
				pingAt = time.Now().Add(10 * time.Second)
			}
			continue
		}
		if err != nil {
			return
		}
		if f.authority.Verify(i) != nil {
			return
		}
		_ = socket.SetWriteDeadline(time.Now().Add(2 * time.Second))
		if err = socket.WriteJSON(frame); err != nil {
			return
		}
	}
}

// Bound accepted HTTP and upgraded sockets together; no unlimited wait queue.
type fixtureListener struct {
	net.Listener
	slots chan struct{}
}
type fixtureConnection struct {
	net.Conn
	slots chan struct{}
	once  sync.Once
}

func (l *fixtureListener) Accept() (net.Conn, error) {
	for {
		c, err := l.Listener.Accept()
		if err != nil {
			return nil, err
		}
		select {
		case l.slots <- struct{}{}:
			return &fixtureConnection{Conn: c, slots: l.slots}, nil
		default:
			_ = c.Close()
		}
	}
}
func (c *fixtureConnection) Close() error {
	err := c.Conn.Close()
	c.once.Do(func() { <-c.slots })
	return err
}
