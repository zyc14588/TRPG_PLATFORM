// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// This Linux-only certification harness owns one disposable Compose project.
// All runtime code is built from the exact clean candidate. It is not a
// production deployment tool or a public Session/package API.
package main

import (
	"bytes"
	"context"
	"crypto/rand"
	"debug/buildinfo"
	"encoding/hex"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"syscall"
	"time"

	"github.com/gorilla/websocket"
	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/migration"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/realtime"
	minimal "github.com/zyc14588/TRPG_PLATFORM/tests/fixture-minimal"
	"github.com/zyc14588/TRPG_PLATFORM/tests/m1/evidence"
)

const pgImage = "sha256:3a82e1f56c8f0f5616a11103ac3d47e632c3938698946a7ad26da0df1334744a"

type phase struct {
	Name, Candidate, Verdict string
	Facts                    map[string]any
	Artifact                 string
}
type result struct {
	Candidate, Platform, Project, Verdict string
	Phases                                []phase
	CleanupVerified                       bool
	Error                                 string `json:",omitempty"`
	Qualifications                        []string
}
type harness struct {
	root, compose, sha, artifact, project, image, temp, fixture, base, gm, player, runnerHash, platformHash string
	env                                                                                                     []string
	result                                                                                                  result
	cfg                                                                                                     map[string]string
	counter                                                                                                 int
}

func main() {
	flags := flag.NewFlagSet("m1_lifecycle", flag.ContinueOnError)
	compose := flags.String("compose-file", "deploy/compose.yaml", "M1 test topology")
	candidate := flags.String("candidate-sha", "HEAD", "exact clean current source")
	output := flags.String("evidence", "tests/smoke/.artifacts/m1-lifecycle.json", "atomic machine artifact")
	if flags.Parse(os.Args[1:]) != nil || flags.NArg() != 0 {
		os.Exit(2)
	}
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	ctx, cancel := context.WithTimeout(ctx, 5*time.Minute)
	defer cancel()
	if err := run(ctx, *compose, *candidate, *output); err != nil {
		fmt.Fprintln(os.Stderr, "M1_LIFECYCLE_FAIL", err)
		os.Exit(1)
	}
}
func run(ctx context.Context, compose, requested, output string) (err error) {
	if runtime.GOOS != "linux" || os.Geteuid() == 0 {
		return fmt.Errorf("Linux non-root operator required")
	}
	root := minimal.RepoRoot()
	sha, err := evidence.Candidate(root, requested)
	if err != nil {
		return err
	}
	var nonce [8]byte
	if _, err = rand.Read(nonce[:]); err != nil {
		return err
	}
	project := "trpg-m1-" + hex.EncodeToString(nonce[:])
	abs := func(p string) string {
		if filepath.IsAbs(p) {
			return p
		}
		return filepath.Join(root, p)
	}
	h := &harness{root: root, compose: abs(compose), sha: sha, artifact: abs(output), project: project, image: "trpg-platform/m1-fixture:" + project}
	h.result = result{Candidate: sha, Platform: "Linux", Project: project, Verdict: "FAIL", Qualifications: []string{"Isolated M1 test only; Windows/macOS NOT_RUN; no production deployment claim.", "Static production binaries freshly built and verified against clean candidate; pinned cached PostgreSQL.", "Existing internal fixture CLI uses exactly the versioned source bytes verified before startup."}}
	h.temp, err = os.MkdirTemp("", "trpg-m1-lifecycle-")
	if err != nil {
		return err
	}
	h.fixture = filepath.Join(h.temp, "fixture")
	h.env = append(os.Environ(), "TRPG_M1_PROJECT="+project, "TRPG_M1_IMAGE="+h.image, "TRPG_M1_CANDIDATE="+sha, "TRPG_M1_BUILD_CONTEXT="+h.temp, "TRPG_M1_DOCKERFILE="+filepath.Join(root, "deploy/docker/Dockerfile"), "TRPG_M1_FIXTURE_DIR="+h.fixture, "TRPG_M1_UID="+strconv.Itoa(os.Geteuid()), "TRPG_M1_GID="+strconv.Itoa(os.Getegid()), "TRPG_M1_PG_PASSWORD=m1-synthetic-"+project)
	// Registered before any Docker command, including builds. Cleanup uses a
	// new bounded context, so cancellation cannot skip project teardown.
	defer func() {
		cleanCtx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
		defer cancel()
		cleanErr := h.phase(cleanCtx, "cleanup", func() (map[string]any, error) { return h.cleanup(cleanCtx) })
		err = errors.Join(err, cleanErr)
		if _, e := evidence.Candidate(root, sha); e != nil {
			err = errors.Join(err, e)
		}
		if err == nil && h.result.CleanupVerified {
			h.result.Verdict = "PASS"
		} else {
			h.result.Error = "phase, source, cancellation, or cleanup failed"
		}
		if e := evidence.Write(h.artifact, h.result); e != nil {
			err = errors.Join(err, e)
		}
		if e := os.RemoveAll(h.temp); e != nil {
			err = errors.Join(err, e)
		}
		fmt.Printf("M1_LIFECYCLE %s candidate=%s project=%s evidence=%s\n", h.result.Verdict, sha, project, h.artifact)
	}()
	if err = h.phase(ctx, "source-build", func() (map[string]any, error) {
		for _, d := range []string{filepath.Join(h.temp, "rootfs/app"), h.fixture, filepath.Join(h.fixture, "objects"), filepath.Join(h.fixture, "stage")} {
			if e := os.MkdirAll(d, 0700); e != nil {
				return nil, e
			}
		}
		// /app must be traversable by the non-root runtime UID.
		for _, d := range []string{filepath.Join(h.temp, "rootfs"), filepath.Join(h.temp, "rootfs/app")} {
			if e := os.Chmod(d, 0755); e != nil {
				return nil, e
			}
		}
		hashes := map[string]any{}
		for _, name := range []string{"platformd", "lua-runner"} {
			path := filepath.Join(h.temp, "rootfs/app", name)
			if _, e := h.exec(ctx, []string{"go", "build", "-trimpath", "-o", path, "./cmd/" + name}, append(h.env, "CGO_ENABLED=0")); e != nil {
				return nil, e
			}
			info, e := buildinfo.ReadFile(path)
			if e != nil {
				return nil, e
			}
			rev, modified := "", ""
			for _, s := range info.Settings {
				if s.Key == "vcs.revision" {
					rev = s.Value
				}
				if s.Key == "vcs.modified" {
					modified = s.Value
				}
			}
			if rev != sha || modified != "false" {
				return nil, fmt.Errorf("source binary binding mismatch")
			}
			raw, e := os.ReadFile(path)
			if e != nil {
				return nil, e
			}
			hashes[name] = checkpoint.Hash(raw)
		}
		h.runnerHash = hashes["lua-runner"].(string)
		h.platformHash = hashes["platformd"].(string)
		pair, e := minimal.Load(install.RuntimeConfig{Runner: "/app/lua-runner", SHA256: h.runnerHash, Limits: profile.DefaultLimits()})
		if e != nil {
			return nil, e
		}
		hashes["old_lock"] = pair.Old.Hash()
		hashes["new_lock"] = pair.New.Hash()
		hashes["roles_per_version"] = 5
		return hashes, nil
	}); err != nil {
		return err
	}
	if err = h.phase(ctx, "compose-config-build", func() (map[string]any, error) {
		raw, e := h.composeCmd(ctx, "config")
		if e != nil {
			return nil, e
		}
		if !bytes.Contains(raw, []byte(pgImage)) {
			return nil, fmt.Errorf("pinned PostgreSQL image missing")
		}
		if _, e = h.composeCmd(ctx, "build", "m1-runtime"); e != nil {
			return nil, e
		}
		info, e := h.exec(ctx, []string{"docker", "image", "inspect", h.image, "--format", "{{.Id}} {{index .Config.Labels \"trpg.m1.candidate\"}} {{index .Config.Labels \"trpg.m1.project\"}}"}, h.env)
		if e != nil {
			return nil, e
		}
		parts := strings.Fields(string(info))
		if len(parts) != 3 || parts[1] != sha || parts[2] != project {
			return nil, fmt.Errorf("image ownership binding mismatch")
		}
		return map[string]any{"compose_config_sha256": checkpoint.Hash(raw), "runtime_image_id": parts[0], "postgres_image_id": pgImage}, nil
	}); err != nil {
		return err
	}
	if err = h.phase(ctx, "postgres-health", func() (map[string]any, error) {
		if _, e := h.composeCmd(ctx, "up", "-d", "--no-build", "m1-postgres"); e != nil {
			return nil, e
		}
		id, e := h.serviceID(ctx, "m1-postgres")
		if e != nil {
			return nil, e
		}
		if e = h.until(ctx, func() bool {
			v, e := h.exec(ctx, []string{"docker", "inspect", id, "--format", "{{.State.Health.Status}}"}, h.env)
			return e == nil && strings.TrimSpace(string(v)) == "healthy"
		}); e != nil {
			return nil, e
		}
		port, e := h.composeCmd(ctx, "port", "m1-postgres", "5432")
		if e != nil {
			return nil, e
		}
		address := strings.TrimSpace(string(port))
		host, _, e := net.SplitHostPort(address)
		if e != nil || host != "127.0.0.1" {
			return nil, fmt.Errorf("fixture database not loopback")
		}
		l, e := net.Listen("tcp", "127.0.0.1:0")
		if e != nil {
			return nil, e
		}
		listen := l.Addr().String()
		l.Close()
		h.base = "http://" + listen
		h.gm = "m1-gm-synthetic-" + project
		h.player = "m1-player-synthetic-" + project
		h.cfg = map[string]string{"dsn": "postgres://fixture_admin:m1-synthetic-" + project + "@" + address + "/m1_smoke_fixture?sslmode=disable", "objects": "/fixture/objects", "staging": "/fixture/stage", "runner": "/app/lua-runner", "runner_hash": h.runnerHash, "listen": listen, "workspace": project, "session": "fixture", "gm_token": h.gm, "player_token": h.player, "migration_version": "1.0.0"}
		if e = h.writeConfig("1.0.0"); e != nil {
			return nil, e
		}
		return map[string]any{"container_id": id, "health": "healthy", "loopback": true}, nil
	}); err != nil {
		return err
	}
	golden, e := minimal.ReadGolden()
	if e != nil {
		return e
	}
	var oldView, newView realtime.Frame
	envelope := command.Envelope{CommandID: golden.Commands[0].ID, SessionID: "fixture", ExpectedStateVersion: 1, SeatID: "gm", Type: "increment", Payload: checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(7)}), CorrelationID: "fixture"}
	if err = h.phase(ctx, "install-session-command-broadcast", func() (map[string]any, error) {
		if e := h.start(ctx); e != nil {
			return nil, e
		}
		c, e := h.stream(ctx)
		if e != nil {
			return nil, e
		}
		defer c.Close()
		initial, e := readFrame(c, 1, 0)
		if e != nil || eventstore.Digest(initial.View) != golden.PlayerHashes[0] {
			return nil, fmt.Errorf("initial permitted view mismatch")
		}
		v, cursor, replayed, e := h.command(ctx, envelope)
		if e != nil || v != 2 || cursor != 1 || replayed {
			return nil, fmt.Errorf("fixed commit mismatch")
		}
		oldView, e = readFrame(c, 2, 1)
		if e != nil || eventstore.Digest(oldView.View) != golden.PlayerHashes[1] || len(oldView.Events) != 1 {
			return nil, fmt.Errorf("commit broadcast mismatch")
		}
		broadcastHash := eventstore.Digest(oldView)
		oldView, e = h.view(ctx, 2, 1)
		if e != nil || eventstore.Digest(oldView.View) != golden.PlayerHashes[1] || len(oldView.Events) != 1 {
			return nil, fmt.Errorf("original reconnect facts mismatch")
		}
		live, e := h.migrate(ctx, "upgrade", 2, "live-denied", true)
		if e != nil {
			return nil, e
		}
		_ = live
		return map[string]any{"initial_view_hash": eventstore.Digest(initial.View), "committed_frame_hash": broadcastHash, "reconnect_frame_hash": eventstore.Digest(oldView), "player_view_hash": eventstore.Digest(oldView.View), "event_count": len(oldView.Events), "live_upgrade": "DENIED"}, nil
	}); err != nil {
		return err
	}
	if err = h.phase(ctx, "stop-restart-original-replay", func() (map[string]any, error) {
		pids, e := h.stop(ctx)
		if e != nil {
			return nil, e
		}
		if e = h.start(ctx); e != nil {
			return nil, e
		}
		frame, e := h.view(ctx, 2, 1)
		if e != nil || eventstore.Digest(frame) != eventstore.Digest(oldView) {
			return nil, fmt.Errorf("original restart replay changed events or views")
		}
		return map[string]any{"frame_hash": eventstore.Digest(frame), "reaped_host_pids": pids}, nil
	}); err != nil {
		return err
	}
	var upgrade, restore migration.Result
	if err = h.phase(ctx, "safe-upgrade", func() (map[string]any, error) {
		if _, e := h.stop(ctx); e != nil {
			return nil, e
		}
		var e error
		upgrade, e = h.migrate(ctx, "upgrade", 2, "upgrade", false)
		if e != nil {
			return nil, e
		}
		if upgrade.Receipt.Version != 3 || upgrade.Receipt.Cursor != 1 || upgrade.OldStateHash != golden.StateHashes[1] || upgrade.NewStateHash != eventstore.Digest(checkpoint.Object(map[string]checkpoint.Value{"total": checkpoint.Int(8), "secret": golden.States[1].Table["secret"]})) {
			return nil, fmt.Errorf("migration changed expected facts")
		}
		if e = h.writeConfig("1.1.0"); e != nil {
			return nil, e
		}
		if e = h.start(ctx); e != nil {
			return nil, e
		}
		newView, e = h.view(ctx, 3, 1)
		if e != nil {
			return nil, e
		}
		expected := checkpoint.Object(map[string]checkpoint.Value{"total": checkpoint.Int(8)})
		if eventstore.Digest(newView.View) != eventstore.Digest(expected) {
			return nil, fmt.Errorf("migrated permitted view mismatch")
		}
		return migrationFacts(upgrade), nil
	}); err != nil {
		return err
	}
	if err = h.phase(ctx, "restart-migrated-replay", func() (map[string]any, error) {
		pids, e := h.stop(ctx)
		if e != nil {
			return nil, e
		}
		if e = h.start(ctx); e != nil {
			return nil, e
		}
		f, e := h.view(ctx, 3, 1)
		if e != nil || eventstore.Digest(f) != eventstore.Digest(newView) {
			return nil, fmt.Errorf("migrated restart replay changed facts")
		}
		return map[string]any{"frame_hash": eventstore.Digest(f), "reaped_host_pids": pids}, nil
	}); err != nil {
		return err
	}
	if err = h.phase(ctx, "restore-point-and-idempotent-retry", func() (map[string]any, error) {
		if _, e := h.stop(ctx); e != nil {
			return nil, e
		}
		var e error
		restore, e = h.migrate(ctx, "restore-point", 3, "restore", false)
		if e != nil {
			return nil, e
		}
		if restore.Receipt.Version != 4 || restore.Receipt.Cursor != 1 || restore.PointHash != upgrade.PointHash || restore.NewStateHash != golden.StateHashes[1] {
			return nil, fmt.Errorf("restored authoritative point mismatch")
		}
		if e = h.writeConfig("1.0.0"); e != nil {
			return nil, e
		}
		if e = h.start(ctx); e != nil {
			return nil, e
		}
		f, e := h.view(ctx, 4, 1)
		if e != nil || eventstore.Digest(f.View) != eventstore.Digest(oldView.View) {
			return nil, fmt.Errorf("restored permitted view mismatch")
		}
		v, c, replayed, e := h.command(ctx, envelope)
		if e != nil || v != 2 || c != 1 || !replayed {
			return nil, fmt.Errorf("original idempotent receipt mismatch")
		}
		if _, e = h.stop(ctx); e != nil {
			return nil, e
		}
		facts := migrationFacts(restore)
		facts["idempotent_retry"] = true
		facts["view_hash"] = eventstore.Digest(f.View)
		return facts, nil
	}); err != nil {
		return err
	}
	return nil
}

func migrationFacts(r migration.Result) map[string]any {
	return map[string]any{"version": r.Receipt.Version, "cursor": r.Receipt.Cursor, "point_hash": r.PointHash, "old_state_hash": r.OldStateHash, "new_state_hash": r.NewStateHash, "old_history_hash": r.OldHistoryHash, "new_history_hash": r.NewHistoryHash, "lock_hash": r.LockHash}
}
func (h *harness) phase(ctx context.Context, name string, fn func() (map[string]any, error)) error {
	h.counter++
	p := phase{Name: name, Candidate: h.sha, Verdict: "FAIL"}
	facts, e := fn()
	p.Facts = facts
	if e == nil {
		p.Verdict = "PASS"
	}
	p.Artifact = filepath.Join(filepath.Dir(h.artifact), "phases", h.project, fmt.Sprintf("%02d-%s.json", h.counter, name))
	h.result.Phases = append(h.result.Phases, p)
	e = errors.Join(e, evidence.Write(p.Artifact, p), evidence.Write(h.artifact, h.result))
	fmt.Printf("M1_PHASE %s %s\n", name, p.Verdict)
	if name != "cleanup" && e == nil && os.Getenv("TRPG_M1_SMOKE_FAIL_AFTER") == name {
		return fmt.Errorf("injected phase failure")
	}
	if name != "cleanup" && e == nil && os.Getenv("TRPG_M1_SMOKE_PAUSE_AFTER") == name {
		<-ctx.Done()
		return ctx.Err()
	}
	return e
}
func (h *harness) exec(ctx context.Context, args, env []string) ([]byte, error) {
	cmd := exec.CommandContext(ctx, args[0], args[1:]...)
	cmd.Dir = h.root
	cmd.Env = env
	out, e := cmd.CombinedOutput()
	if e != nil {
		return out, fmt.Errorf("%s command failed; output_sha256=%s", args[0], checkpoint.Hash(out))
	}
	return out, nil
}
func (h *harness) composeCmd(ctx context.Context, args ...string) ([]byte, error) {
	a := []string{"docker", "compose", "-f", h.compose, "--project-name", h.project, "--profile", "m1-fixture"}
	return h.exec(ctx, append(a, args...), h.env)
}
func (h *harness) serviceID(ctx context.Context, name string) (string, error) {
	raw, e := h.composeCmd(ctx, "ps", "-aq", name)
	id := strings.TrimSpace(string(raw))
	if e != nil || len(id) != 64 {
		return "", fmt.Errorf("owned service identity unavailable")
	}
	return id, nil
}
func (h *harness) until(ctx context.Context, fn func() bool) error {
	timer := time.NewTimer(40 * time.Second)
	defer timer.Stop()
	tick := time.NewTicker(200 * time.Millisecond)
	defer tick.Stop()
	for {
		if fn() {
			return nil
		}
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-timer.C:
			return fmt.Errorf("bounded readiness deadline")
		case <-tick.C:
		}
	}
}
func (h *harness) writeConfig(version string) error {
	h.cfg["migration_version"] = version
	return evidence.Write(filepath.Join(h.fixture, "operator.json"), h.cfg)
}
func (h *harness) start(ctx context.Context) error {
	if _, e := h.composeCmd(ctx, "up", "-d", "--no-build", "m1-runtime"); e != nil {
		return e
	}
	if e := h.until(ctx, func() bool {
		req, e := http.NewRequestWithContext(ctx, "GET", h.base+"/ready", nil)
		if e != nil {
			return false
		}
		client := http.Client{Timeout: time.Second}
		r, e := client.Do(req)
		if e != nil {
			return false
		}
		defer r.Body.Close()
		return r.StatusCode == 200
	}); e != nil {
		return e
	}
	id, e := h.serviceID(ctx, "m1-runtime")
	if e != nil {
		return e
	}
	for name, hash := range map[string]string{"platformd": h.platformHash, "lua-runner": h.runnerHash} {
		path := filepath.Join(h.temp, "readback-"+name)
		if _, e := h.exec(ctx, []string{"docker", "cp", id + ":/app/" + name, path}, h.env); e != nil {
			return e
		}
		raw, e := os.ReadFile(path)
		if e != nil || checkpoint.Hash(raw) != hash {
			return fmt.Errorf("running container binary differs from source")
		}
	}
	return nil
}
func (h *harness) stop(ctx context.Context) ([]int, error) {
	id, e := h.serviceID(ctx, "m1-runtime")
	if e != nil {
		return nil, e
	}
	top, e := h.exec(ctx, []string{"docker", "top", id, "-eo", "pid,args"}, h.env)
	if e != nil {
		return nil, e
	}
	var pids []int
	for _, line := range strings.Split(string(top), "\n") {
		f := strings.Fields(line)
		if len(f) > 1 && (strings.Contains(line, "/app/lua-runner") || strings.Contains(line, "/app/platformd")) {
			pid, e := strconv.Atoi(f[0])
			if e == nil && pid > 0 {
				pids = append(pids, pid)
			}
		}
	}
	if len(pids) == 0 {
		return nil, fmt.Errorf("no actual source process observed")
	}
	if _, e = h.composeCmd(ctx, "stop", "-t", "15", "m1-runtime"); e != nil {
		return nil, e
	}
	raw, e := h.exec(ctx, []string{"docker", "inspect", id, "--format", "{{.State.ExitCode}} {{.State.Pid}}"}, h.env)
	if e != nil || strings.TrimSpace(string(raw)) != "0 0" {
		return nil, fmt.Errorf("daemon did not shut down cleanly")
	}
	for _, pid := range pids {
		if syscall.Kill(pid, 0) != syscall.ESRCH {
			return nil, fmt.Errorf("actual daemon/Runner PID not reaped")
		}
	}
	logs, e := h.exec(ctx, []string{"docker", "logs", id}, h.env)
	if e != nil {
		return nil, e
	}
	if e = h.privateOutput(logs); e != nil {
		return nil, e
	}
	return pids, nil
}
func (h *harness) stream(ctx context.Context) (*websocket.Conn, error) {
	header := http.Header{}
	header.Set("Authorization", "Bearer "+h.player)
	header.Set("X-Fixture-Seat", "player")
	c, r, e := websocket.DefaultDialer.DialContext(ctx, strings.Replace(h.base, "http://", "ws://", 1)+"/stream?after=0", header)
	if r != nil && e != nil {
		r.Body.Close()
	}
	return c, e
}
func readFrame(c *websocket.Conn, version, cursor uint64) (realtime.Frame, error) {
	var f realtime.Frame
	c.SetReadDeadline(time.Now().Add(10 * time.Second))
	if e := c.ReadJSON(&f); e != nil {
		return f, e
	}
	if f.Session != "fixture" || f.Version != version || f.Cursor != cursor || len(f.View.Table) != 1 {
		return f, fmt.Errorf("frame/version/cursor/privacy mismatch")
	}
	return f, nil
}
func (h *harness) view(ctx context.Context, v, cursor uint64) (realtime.Frame, error) {
	c, e := h.stream(ctx)
	if e != nil {
		return realtime.Frame{}, e
	}
	defer c.Close()
	return readFrame(c, v, cursor)
}
func (h *harness) command(ctx context.Context, envelope command.Envelope) (uint64, uint64, bool, error) {
	raw, e := json.Marshal(envelope)
	if e != nil {
		return 0, 0, false, e
	}
	req, e := http.NewRequestWithContext(ctx, "POST", h.base+"/command", bytes.NewReader(raw))
	if e != nil {
		return 0, 0, false, e
	}
	req.Header.Set("Authorization", "Bearer "+h.gm)
	req.Header.Set("X-Fixture-Seat", "gm")
	req.Header.Set("Content-Type", "application/json")
	client := http.Client{Timeout: 15 * time.Second}
	r, e := client.Do(req)
	if e != nil {
		return 0, 0, false, e
	}
	defer r.Body.Close()
	out, e := io.ReadAll(io.LimitReader(r.Body, 256<<10))
	if e != nil || r.StatusCode != 200 {
		return 0, 0, false, fmt.Errorf("fixture command did not commit")
	}
	var ack struct {
		Kind     string `json:"kind"`
		Version  uint64 `json:"state_version"`
		Cursor   uint64 `json:"event_cursor"`
		Replayed bool   `json:"replayed"`
	}
	if json.Unmarshal(out, &ack) != nil || ack.Kind != "committed" {
		return 0, 0, false, fmt.Errorf("invalid commit acknowledgement")
	}
	return ack.Version, ack.Cursor, ack.Replayed, nil
}
func (h *harness) migrate(ctx context.Context, op string, version uint64, id string, denied bool) (migration.Result, error) {
	raw, e := h.composeCmd(ctx, "run", "--rm", "--no-deps", "m1-runtime", "m1-migration-fixture", "--config=/fixture/operator.json", "--operation="+op, "--expected-version="+strconv.FormatUint(version, 10), "--point=before-upgrade", "--command="+id)
	var r migration.Result
	if denied {
		if e == nil || !bytes.Contains(raw, []byte("FIXTURE_MIGRATION_REQUIRES_STOPPED_DAEMON")) {
			return r, fmt.Errorf("live daemon migration did not deny")
		}
		return r, nil
	}
	if e != nil {
		return r, e
	}
	// Compose may prefix lifecycle notices. Decode the actual single JSON
	// result line; full private receipts never enter ordinary/machine artifacts.
	found := false
	for _, line := range bytes.Split(raw, []byte{'\n'}) {
		if len(line) > 0 && line[0] == '{' {
			if json.Unmarshal(line, &r) != nil || found {
				return r, fmt.Errorf("ambiguous migration result")
			}
			found = true
		}
	}
	if !found || r.PointHash == "" {
		return r, fmt.Errorf("missing migration result")
	}
	return r, nil
}
func (h *harness) privateOutput(raw []byte) error {
	for _, s := range []string{"fixture-gm-private-value", "sensitive-fixture-value", "must-not-reach-runner", "migration-fixture-gm-private", h.gm, h.player, "m1-synthetic-" + h.project} {
		if s != "" && bytes.Contains(raw, []byte(s)) {
			return fmt.Errorf("private fixture value in ordinary logs")
		}
	}
	return nil
}
func (h *harness) cleanup(ctx context.Context) (map[string]any, error) {
	var errs []error
	_, e := h.composeCmd(ctx, "down", "--volumes", "--remove-orphans", "--timeout", "15")
	errs = append(errs, e)
	facts := map[string]any{"down_volumes_remove_orphans": e == nil}
	for _, kind := range []string{"container", "network", "volume"} {
		raw, e := h.exec(ctx, []string{"docker", kind, "ls", "-q", "--filter", "label=com.docker.compose.project=" + h.project}, h.env)
		if e != nil || len(bytes.TrimSpace(raw)) != 0 {
			errs = append(errs, fmt.Errorf("owned %s resources remain or absence check failed", kind))
		}
		facts[kind+"_remaining"] = strings.Fields(string(raw))
	}
	raw, e := h.exec(ctx, []string{"docker", "image", "ls", "-q", "--filter", "label=trpg.m1.project=" + h.project}, h.env)
	if e != nil {
		errs = append(errs, e)
	} else if len(bytes.TrimSpace(raw)) > 0 {
		_, e = h.exec(ctx, []string{"docker", "image", "rm", h.image}, h.env)
		errs = append(errs, e)
	}
	// A unique owned image tag must also be absent; inspect failure alone is
	// not treated as absence when the Docker daemon is unavailable.
	raw, e = h.exec(ctx, []string{"docker", "image", "ls", "-q", "--filter", "reference=" + h.image}, h.env)
	if e != nil || len(bytes.TrimSpace(raw)) > 0 {
		errs = append(errs, fmt.Errorf("owned runtime image cleanup failed"))
	}
	facts["owned_image_absent"] = e == nil && len(bytes.TrimSpace(raw)) == 0
	e = errors.Join(errs...)
	h.result.CleanupVerified = e == nil
	return facts, e
}
