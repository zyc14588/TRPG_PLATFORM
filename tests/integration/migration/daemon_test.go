//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package migration_test

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"syscall"
	"testing"
	"time"

	"github.com/gorilla/websocket"
	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/migration"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/migration"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/realtime"
)

type daemonOutput struct {
	mu    sync.Mutex
	raw   []byte
	ready chan string
}

func (b *daemonOutput) Write(raw []byte) (int, error) {
	b.mu.Lock()
	defer b.mu.Unlock()
	if len(b.raw)+len(raw) > 2<<20 {
		return 0, fmt.Errorf("fixture output bound")
	}
	b.raw = append(b.raw, raw...)
	text := string(b.raw)
	if start := strings.Index(text, "FIXTURE_READY "); start >= 0 {
		if end := strings.IndexByte(text[start:], '\n'); end >= 0 {
			select {
			case b.ready <- text[start+len("FIXTURE_READY ") : start+end]:
			default:
			}
		}
	}
	return len(raw), nil
}
func (b *daemonOutput) snapshot() []byte {
	b.mu.Lock()
	defer b.mu.Unlock()
	return append([]byte(nil), b.raw...)
}

type fixtureDaemon struct {
	cmd     *exec.Cmd
	output  *daemonOutput
	done    chan error
	url     string
	stopped bool
}

func startMigrationDaemon(t *testing.T, path string) *fixtureDaemon {
	t.Helper()
	p := &fixtureDaemon{cmd: exec.Command(daemonBinary, "m1-fixture", "--config", path), output: &daemonOutput{ready: make(chan string, 1)}, done: make(chan error, 1)}
	p.cmd.Stdout, p.cmd.Stderr = p.output, p.output
	if err := p.cmd.Start(); err != nil {
		t.Fatal(err)
	}
	go func() { p.done <- p.cmd.Wait() }()
	t.Cleanup(func() { p.stop(t) })
	select {
	case p.url = <-p.output.ready:
	case err := <-p.done:
		p.stopped = true
		t.Fatal("daemon startup failed", err, checkpoint.Hash(p.output.snapshot()))
	case <-time.After(30 * time.Second):
		p.stop(t)
		t.Fatal("daemon readiness timeout")
	}
	t.Logf("B007_ACTUAL_PLATFORMD_PID %d", p.cmd.Process.Pid)
	return p
}
func assertDaemonExecutionsReaped(t *testing.T, raw []byte) {
	t.Helper()
	alive := map[int]bool{}
	for _, line := range strings.Split(string(raw), "\n") {
		if !strings.HasPrefix(line, "FIXTURE_EXECUTION ") {
			continue
		}
		var v install.Execution
		if json.Unmarshal([]byte(strings.TrimPrefix(line, "FIXTURE_EXECUTION ")), &v) != nil {
			t.Fatal("invalid runner machine evidence")
		}
		if v.PID > 0 {
			alive[v.PID] = !v.Reaped
		}
		t.Logf("B007_DAEMON_EXECUTION %+v", v)
	}
	if len(alive) == 0 {
		t.Fatal("production Runner never executed")
	}
	for pid, live := range alive {
		if live || syscall.Kill(pid, 0) != syscall.ESRCH {
			t.Errorf("owned Runner %d not reaped", pid)
		}
	}
}
func (p *fixtureDaemon) stop(t *testing.T) {
	t.Helper()
	if p.stopped {
		return
	}
	p.stopped = true
	_ = p.cmd.Process.Signal(syscall.SIGTERM)
	select {
	case err := <-p.done:
		if err != nil {
			t.Error("daemon shutdown failed", err, checkpoint.Hash(p.output.snapshot()))
		}
	case <-time.After(8 * time.Second):
		p.cmd.Process.Kill()
		<-p.done
		t.Error("daemon exceeded shutdown bound")
	}
	if syscall.Kill(p.cmd.Process.Pid, 0) != syscall.ESRCH {
		t.Error("owned daemon not joined")
	}
	assertDaemonExecutionsReaped(t, p.output.snapshot())
}
func daemonView(t *testing.T, p *fixtureDaemon, token, field string, version, cursor uint64) realtime.Frame {
	t.Helper()
	headers := http.Header{"Authorization": {"Bearer " + token}, "X-Fixture-Seat": {"player"}}
	c, res, err := websocket.DefaultDialer.Dial(strings.Replace(p.url, "http://", "ws://", 1)+"/stream?after=0", headers)
	if err != nil {
		if res != nil {
			t.Fatal("stream denied", res.StatusCode)
		}
		t.Fatal(err)
	}
	defer c.Close()
	c.SetReadDeadline(time.Now().Add(4 * time.Second))
	_, raw, err := c.ReadMessage()
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Contains(raw, []byte(fixture.PrivateValue)) {
		t.Fatal("private value crossed player boundary")
	}
	var f realtime.Frame
	if checkpoint.StrictDecode(raw, &f, 256<<10) != nil || f.Version != version || f.Cursor != cursor || f.View.Kind != "table" || len(f.View.Table) != 1 || f.View.Table[field].Kind != "integer" {
		t.Fatal("invalid filtered recovery view", checkpoint.Hash(raw))
	}
	return f
}
func daemonCommand(t *testing.T, p *fixtureDaemon, token string, e command.Envelope) (uint64, uint64, bool) {
	t.Helper()
	raw, err := json.Marshal(e)
	if err != nil {
		t.Fatal(err)
	}
	r, err := http.NewRequest(http.MethodPost, p.url+"/command", bytes.NewReader(raw))
	if err != nil {
		t.Fatal(err)
	}
	r.Header.Set("Authorization", "Bearer "+token)
	r.Header.Set("X-Fixture-Seat", "gm")
	response, err := (&http.Client{Timeout: 5 * time.Second}).Do(r)
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	raw, err = io.ReadAll(io.LimitReader(response.Body, 256<<10+1))
	if err != nil {
		t.Fatal(err)
	}
	if response.StatusCode != 200 || bytes.Contains(raw, []byte(fixture.PrivateValue)) {
		t.Fatal("command rejected or private receipt", response.StatusCode, checkpoint.Hash(raw))
	}
	var ack struct {
		Kind        string           `json:"kind"`
		ID          string           `json:"command_id"`
		Correlation string           `json:"correlation_id"`
		Version     uint64           `json:"state_version"`
		Cursor      uint64           `json:"event_cursor"`
		Replayed    bool             `json:"replayed"`
		Result      checkpoint.Value `json:"result"`
	}
	if checkpoint.StrictDecode(raw, &ack, 256<<10) != nil || ack.Kind != "committed" || ack.ID != e.CommandID {
		t.Fatal("invalid committed receipt")
	}
	return ack.Version, ack.Cursor, ack.Replayed
}
func daemonMigration(t *testing.T, path, operation, commandID string, version uint64, denied bool) migration.Result {
	t.Helper()
	c := exec.Command(daemonBinary, "m1-migration-fixture", "--config", path, "--operation", operation, "--expected-version", fmt.Sprint(version), "--command", commandID)
	var out, errOut bytes.Buffer
	c.Stdout, c.Stderr = &out, &errOut
	err := c.Run()
	if denied {
		if err == nil || strings.TrimSpace(errOut.String()) != "FIXTURE_MIGRATION_REQUIRES_STOPPED_DAEMON" {
			t.Fatal("active daemon guard bypassed", err, checkpoint.Hash(errOut.Bytes()))
		}
		return migration.Result{}
	}
	if err != nil {
		t.Fatal("offline migration failed", err, checkpoint.Hash(errOut.Bytes()))
	}
	assertDaemonExecutionsReaped(t, errOut.Bytes())
	var result migration.Result
	if checkpoint.StrictDecode(out.Bytes(), &result, 256<<10) != nil || !checkpoint.IsDigest(result.PointHash) || !checkpoint.IsDigest(result.LockHash) {
		t.Fatal("invalid migration machine receipt")
	}
	return result
}

func TestLinuxDaemonStopUpgradeRestartReplayAndRecordedPointRestore(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "operator.json")
	gm, player := "b007-fixture-gm-0123456789abcdef", "b007-fixture-player-0123456789abcdef"
	cfg := map[string]string{"dsn": dsn, "objects": filepath.Join(dir, "objects"), "staging": filepath.Join(dir, "stage"), "runner": runner, "runner_hash": runnerHash, "listen": "127.0.0.1:0", "workspace": fmt.Sprintf("%s-daemon-%d", runID, sequence.Add(1)), "session": "daemon", "gm_token": gm, "player_token": player, "migration_version": "1.0.0"}
	write := func(version string) {
		cfg["migration_version"] = version
		raw, _ := json.Marshal(cfg)
		if err := os.WriteFile(path, raw, 0600); err != nil {
			t.Fatal(err)
		}
	}
	write("1.0.0")
	p := startMigrationDaemon(t, path)
	initial := daemonView(t, p, player, "counter", 1, 0)
	if !reflectValue(initial.View.Table["counter"], checkpoint.Int(1)) {
		t.Fatal("wrong initial fixture")
	}
	envelope := command.Envelope{CommandID: "first", SessionID: "daemon", ExpectedStateVersion: 1, SeatID: "gm", Type: "increment", Payload: checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(7)}), CorrelationID: "fixture"}
	v, c, replayed := daemonCommand(t, p, gm, envelope)
	if v != 2 || c != 1 || replayed {
		t.Fatal("wrong first commit")
	}
	old := daemonView(t, p, player, "counter", 2, 1)
	daemonMigration(t, path, "upgrade", "live-denied", 2, true)
	p.stop(t)
	upgraded := daemonMigration(t, path, "upgrade", "upgrade", 2, false)
	if upgraded.Receipt.Version != 3 || upgraded.Receipt.Cursor != 1 {
		t.Fatal("upgrade changed event cursor")
	}
	write("1.1.0")
	p = startMigrationDaemon(t, path)
	next := daemonView(t, p, player, "total", 3, 1)
	if !reflectValue(next.View.Table["total"], old.View.Table["counter"]) {
		t.Fatal("restarted target lost migrated value")
	}
	p.stop(t)
	p = startMigrationDaemon(t, path)
	replay := daemonView(t, p, player, "total", 3, 1)
	if eventstore.Digest(replay) != eventstore.Digest(next) {
		t.Fatal("target restart replay differs")
	}
	p.stop(t)
	restored := daemonMigration(t, path, "restore-point", "restore", 3, false)
	if restored.Receipt.Version != 4 || restored.Receipt.Cursor != 1 || restored.PointHash != upgraded.PointHash {
		t.Fatal("point restore changed original cursor or point")
	}
	write("1.0.0")
	p = startMigrationDaemon(t, path)
	after := daemonView(t, p, player, "counter", 4, 1)
	if eventstore.Digest(after.View) != eventstore.Digest(old.View) {
		t.Fatal("restarted old view not restored")
	}
	v, c, replayed = daemonCommand(t, p, gm, envelope)
	if v != 2 || c != 1 || !replayed {
		t.Fatal("original command receipt changed after restoration")
	}
	p.stop(t)
	evidence(t, "actual-linux-platformd-migration", struct {
		Upgrade, Restore         migration.Result
		OldViewHash, NewViewHash string
	}{upgraded, restored, eventstore.Digest(old.View), eventstore.Digest(next.View)})
}
func reflectValue(a, b checkpoint.Value) bool { return eventstore.Digest(a) == eventstore.Digest(b) }
