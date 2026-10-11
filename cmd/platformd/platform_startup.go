// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"context"
	"crypto/tls"
	"errors"
	"flag"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/gateway"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/deployment/m2"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
)

func runM2(ctx context.Context, args []string, out, stderr io.Writer, health bool) int {
	flags := flag.NewFlagSet("m2-serve", flag.ContinueOnError)
	flags.SetOutput(stderr)
	path := flags.String("config", "", "operator configuration file reference")
	ownerAction := flags.String("owner-model-action", "", "explicit one-time owner model action file reference")
	if flags.Parse(args) != nil || flags.NArg() != 0 {
		return 2
	}
	c, e := m2.LoadConfig(*path)
	if e != nil || m2.RequireLinux() != nil {
		fmt.Fprintln(stderr, "M2 configuration rejected")
		return 1
	}
	if health {
		if *ownerAction != "" {
			return 2
		}
		if m2.ProbeDaemon(ctx, c, "platformd") != nil {
			return 1
		}
		return 0
	}
	if e = serveM2(ctx, c, out, *ownerAction); e != nil {
		var failure *startupFailure
		if errors.As(e, &failure) {
			fmt.Fprintln(stderr, "M2 platform unavailable at", failure.stage)
		} else {
			fmt.Fprintln(stderr, "M2 platform unavailable")
		}
		return 1
	}
	return 0
}
func serveM2(parent context.Context, c m2.Config, out io.Writer, ownerAction string) error {
	ctx, cancel := context.WithCancel(parent)
	defer cancel()
	// Validate all referenced secrets before creating a listener. Every service
	// has only its own keys, and each value is cleared after construction.
	for _, f := range []string{c.DSNFile, c.CookieKeyFile, c.ReplayKeyFile, c.InvitationKeyFile, c.VaultKeyFile} {
		b, e := m2.ReadSecret(f, 16384)
		if e != nil {
			return e
		}
		clear(b)
	}
	objects, e := m2.NewObjectClient(c)
	if e != nil {
		return startupRejected("object client", e)
	}
	defer objects.Close()
	if e = objects.Ready(ctx); e != nil {
		return startupRejected("objects readiness", e)
	}
	launcher, e := m2.NewSupervisorLauncher(c)
	if e != nil {
		return startupRejected("supervisor client", e)
	}
	defer launcher.Close()
	if e = launcher.Ready(ctx); e != nil {
		return startupRejected("supervisor readiness", e)
	}
	plan, e := m2.ReadOperatorPlan(c.PackagesFile)
	if e != nil {
		return startupRejected("operator plan", e)
	}
	rooms, e := platformRoomCore(ctx, c.Origin, c.DSNFile, c.CookieKeyFile, c.ReplayKeyFile, c.InvitationKeyFile, "/app/schemas/platform/platform-auth-api-v1.schema.json", "/app/schemas/platform/platform-room-api-v1.schema.json", c.SeedFile, c.GrantsFile)
	if e != nil {
		return startupRejected("room authority", e)
	}
	defer rooms.repo.Close()
	core, e := platformPlayerCore(ctx, rooms.repo)
	if e != nil {
		return startupRejected("player stores", e)
	}
	packages, e := openM2Packages(ctx, c, core, objects, launcher, plan)
	if e != nil {
		return startupRejected("installed packages", e)
	}
	defer packages.repository.Close()
	policies, e := newInstalledPolicies(ctx, plan, packages)
	if e != nil {
		return startupRejected("installed policies", e)
	}
	certs := make([]model.Certification, 0, len(plan.Certificates))
	for _, v := range plan.Certificates {
		certs = append(certs, model.NewCertification(v))
	}
	models, vault, e := platformModels(ctx, rooms.authority, rooms.storage, core.guarded, rooms.repo, c.VaultKeyFile, []model.Endpoint{model.NewEndpoint(c.Provider.Endpoint)}, certs, plan.Defaults, plan.WorkspaceLimits)
	if e != nil {
		return startupRejected("models", e)
	}
	defer vault.Close()
	if ownerAction != "" {
		if e := applyOwnerModelAction(ctx, c, plan, models, ownerAction); e != nil {
			return startupRejected("explicit owner action", e)
		}
	}
	old, e := os.ReadFile("/app/schemas/platform/platform-player-api-v1.schema.json")
	if e != nil {
		return e
	}
	presentation, e := os.ReadFile("/app/schemas/platform/platform-player-presentation-api-v1.schema.json")
	if e != nil {
		return e
	}
	// B014's accepted constructor is used on the actual shared production
	// components and listener. This creates one launch registry and control.
	players, e := platformPlayersWithPresentation(platformPlayerOptions{ctx: ctx, repo: rooms.repo, authority: rooms.authority, roomStorage: rooms.storage, rooms: rooms.rooms, configurations: packages.configs, models: models, policies: policies, descriptions: packages.descriptions, schema: old, origin: c.Origin, maxSessions: 8, maxConnections: 64, shared: core}, models, plan.Labels, presentation)
	if e != nil {
		return startupRejected("player presentation", e)
	}
	defer players.close()
	broker, e := m2.NewBroker(gateway.TransportBinding(c.Provider.Adapter()))
	if e != nil {
		return startupRejected("provider broker", e)
	}
	defer broker.Close()
	workerTLS, e := m2.TLSConfig(c.TLS, "workerd", true)
	if e != nil {
		return e
	}
	private, e := m2.ServeUnix(ctx, c.WorkerSocket, workerTLS, c.PeerUID, broker.Handler())
	if e != nil {
		return startupRejected("worker socket", e)
	}
	defer func() {
		closeCtx, stop := context.WithTimeout(context.Background(), 3*time.Second)
		defer stop()
		_ = private.Close(closeCtx)
	}()
	gw, e := openM2Gateway(ctx, c, plan, rooms, players, models, vault, policies, broker, certs)
	if e != nil {
		return startupRejected("gateway orchestration", e)
	}
	defer gw.adapter.Close()
	done := make(chan struct{})
	go func() { defer close(done); _ = gw.runtime.Run(ctx) }()
	defer func() { cancel(); <-done }()
	tlsConfig, e := m2.TLSConfig(c.TLS, "reverse-proxy|platformd", true)
	if e != nil {
		return e
	}
	mux := http.NewServeMux()
	mux.Handle("/api/", http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gw.callers.capture(r)
		players.state().handler.ServeHTTP(w, r)
	}))
	mux.HandleFunc("/health/live", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "GET" {
			http.NotFound(w, r)
			return
		}
		w.Header().Set("Cache-Control", "no-store")
		_, _ = io.WriteString(w, "alive\n")
	})
	mux.HandleFunc("/health/ready", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "GET" || !broker.Ready() || gw.worker.Check(r.Context(), plan.Games[0].Workspace) != nil || objects.Ready(r.Context()) != nil || launcher.Ready(r.Context()) != nil {
			http.Error(w, "unavailable", 503)
			return
		}
		// Reload current source and verify the actual authorized object graph.
		for _, g := range plan.Games {
			raw, e := m2.ReadSecret(g.InstallCredentialFile, 256)
			if e != nil {
				http.Error(w, "unavailable", 503)
				return
			}
			_, e = packages.reader.Load(r.Context(), stringCredential(raw), g.Workspace, g.Root)
			clear(raw)
			if e != nil {
				http.Error(w, "unavailable", 503)
				return
			}
		}
		w.Header().Set("Cache-Control", "no-store")
		_, _ = io.WriteString(w, "ready\n")
	})
	l, e := net.Listen("tcp", c.DaemonAddress)
	if e != nil {
		return m2.ErrPrivate
	}
	s := &http.Server{Handler: mux, TLSConfig: tlsConfig, ReadHeaderTimeout: 2 * time.Second, ReadTimeout: 7 * time.Second, WriteTimeout: 7 * time.Second, IdleTimeout: 10 * time.Second, MaxHeaderBytes: 16384, ErrorLog: log.New(io.Discard, "", 0), BaseContext: func(net.Listener) context.Context { return ctx }}
	serverDone := make(chan error, 1)
	go func() { serverDone <- s.Serve(tls.NewListener(l, tlsConfig)) }()
	if !filepath.IsAbs(c.StagingRoot) || !checkpoint.IsDigest(c.Source) {
		_ = s.Close()
		return m2.ErrConfiguration
	}
	_, _ = fmt.Fprintln(out, "M2 platform started; one authority; six-service entry; provider calls remain explicit")
	select {
	case <-ctx.Done():
		closeCtx, stop := context.WithTimeout(context.Background(), 3*time.Second)
		defer stop()
		if e = s.Shutdown(closeCtx); e != nil {
			_ = s.Close()
		}
		<-serverDone
		return players.close()
	case <-serverDone:
		return m2.ErrPrivate
	}
}

// Safe diagnostics identify only a fixed startup stage.
type startupFailure struct {
	stage string
	cause error
}

func (e *startupFailure) Error() string           { return "M2 startup stage unavailable" }
func (e *startupFailure) Unwrap() error           { return e.cause }
func startupRejected(stage string, e error) error { return &startupFailure{stage, e} }
