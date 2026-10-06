//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package replay_test

import (
	"bytes"
	"context"
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
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/session/testdata"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

type outputBuffer struct {
	mu    sync.Mutex
	data  []byte
	ready chan string
}

func (b *outputBuffer) Write(p []byte) (int, error) {
	b.mu.Lock()
	defer b.mu.Unlock()
	if len(b.data)+len(p) > 2<<20 {
		return 0, fmt.Errorf("bounded fixture output exceeded")
	}
	b.data = append(b.data, p...)
	text := string(b.data)
	if start := strings.Index(text, "FIXTURE_READY "); start >= 0 {
		if end := strings.IndexByte(text[start:], '\n'); end >= 0 {
			select {
			case b.ready <- text[start+len("FIXTURE_READY ") : start+end]:
			default:
			}
		}
	}
	return len(p), nil
}
func (b *outputBuffer) snapshot() string { b.mu.Lock(); defer b.mu.Unlock(); return string(b.data) }

type process struct {
	cmd     *exec.Cmd
	output  *outputBuffer
	done    chan error
	url     string
	stopped bool
}

func startDaemon(t *testing.T, path string) *process {
	t.Helper()
	p := &process{cmd: exec.Command(daemonBinary, "m1-fixture", "--config", path), output: &outputBuffer{ready: make(chan string, 1)}, done: make(chan error, 1)}
	p.cmd.Stdout = p.output
	p.cmd.Stderr = p.output
	if err := p.cmd.Start(); err != nil {
		t.Fatal(err)
	}
	go func() { p.done <- p.cmd.Wait() }()
	t.Cleanup(func() { p.stop(t) })
	select {
	case p.url = <-p.output.ready:
	case err := <-p.done:
		p.stopped = true
		t.Fatal("actual daemon failed", err, p.output.snapshot())
	case <-time.After(30 * time.Second):
		p.stop(t)
		t.Fatal("actual daemon readiness timeout", p.output.snapshot())
	}
	t.Logf("B006_ACTUAL_PLATFORMD_PID %d %s", p.cmd.Process.Pid, p.url)
	return p
}
func (p *process) stop(t *testing.T) {
	t.Helper()
	if p.stopped {
		return
	}
	p.stopped = true
	_ = p.cmd.Process.Signal(syscall.SIGTERM)
	select {
	case err := <-p.done:
		if err != nil {
			t.Error("actual daemon signal exit", err, p.output.snapshot())
		}
	case <-time.After(8 * time.Second):
		_ = p.cmd.Process.Kill()
		<-p.done
		t.Error("actual daemon did not exit on SIGTERM")
	}
	if syscall.Kill(p.cmd.Process.Pid, 0) != syscall.ESRCH {
		t.Error("daemon not joined")
	}
	alive := map[int]bool{}
	for _, line := range strings.Split(p.output.snapshot(), "\n") {
		if !strings.HasPrefix(line, "FIXTURE_EXECUTION ") {
			continue
		}
		var v install.Execution
		if json.Unmarshal([]byte(strings.TrimPrefix(line, "FIXTURE_EXECUTION ")), &v) != nil {
			t.Error("invalid daemon runner evidence")
			continue
		}
		if v.PID > 0 {
			alive[v.PID] = !v.Reaped
		}
		t.Logf("B006_DAEMON_EXECUTION %+v", v)
	}
	if len(alive) == 0 {
		t.Error("actual daemon never ran production Lua child")
	}
	for pid, live := range alive {
		if live || syscall.Kill(pid, 0) != syscall.ESRCH {
			t.Errorf("daemon runner %d not reaped", pid)
		}
	}
}
func wire(t *testing.T, p *process, path, token, seat string, body []byte) (int, []byte) {
	t.Helper()
	request, err := http.NewRequest(http.MethodPost, p.url+path, bytes.NewReader(body))
	if err != nil {
		t.Fatal(err)
	}
	request.Header.Set("Authorization", "Bearer "+token)
	request.Header.Set("X-Fixture-Seat", seat)
	client := &http.Client{Timeout: 5 * time.Second}
	response, err := client.Do(request)
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	raw, err := io.ReadAll(io.LimitReader(response.Body, 256<<10))
	if err != nil {
		t.Fatal(err)
	}
	return response.StatusCode, raw
}
func stream(t *testing.T, p *process, token, seat string, after uint64) *websocket.Conn {
	t.Helper()
	header := http.Header{"Authorization": {"Bearer " + token}, "X-Fixture-Seat": {seat}}
	url := strings.Replace(p.url, "http://", "ws://", 1) + fmt.Sprintf("/stream?after=%d", after)
	c, res, err := websocket.DefaultDialer.Dial(url, header)
	if err != nil {
		if res != nil {
			t.Fatal(err, res.StatusCode)
		}
		t.Fatal(err)
	}
	t.Cleanup(func() { c.Close() })
	return c
}
func readStream(t *testing.T, c *websocket.Conn) []byte {
	t.Helper()
	_ = c.SetReadDeadline(time.Now().Add(3 * time.Second))
	_, raw, err := c.ReadMessage()
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func TestActualPlatformdRestartBetweenSQLCommitAndBroadcast(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "operator.json")
	workspace := fmt.Sprintf("%s-daemon-%d", runID, sequence.Add(1))
	gmToken, playerToken := "b006-daemon-gm-0123456789abcdef", "b006-daemon-player-0123456789abcdef"
	config := map[string]any{"dsn": dsn, "objects": filepath.Join(root, "objects"), "staging": filepath.Join(root, "stage"), "runner": runner, "runner_hash": runnerHash, "listen": "127.0.0.1:0", "workspace": workspace, "session": "daemon", "gm_token": gmToken, "player_token": playerToken, "commit_barrier": true}
	write := func() {
		raw, _ := json.Marshal(config)
		if err := os.WriteFile(path, raw, 0600); err != nil {
			t.Fatal(err)
		}
	}
	write()
	pkg, _, err := fixture.Build(runtime(), "")
	if err != nil {
		t.Fatal(err)
	}
	lock, err := pkg.ExactLock().Digest()
	if err != nil {
		t.Fatal(err)
	}
	b := data.Binding{Workspace: workspace, Session: "daemon", GraphHash: string(lock)}
	host, err := postgres.OpenHostRepository(context.Background(), dsn, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer host.Close()
	p := startDaemon(t, path)
	player := stream(t, p, playerToken, "player", 0)
	if raw := readStream(t, player); bytes.Contains(raw, []byte(fixture.PrivateValue)) {
		t.Fatal("private genesis leak")
	}
	env := envelope(b, "first", "increment", 1)
	raw, _ := json.Marshal(env)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, p.url+"/command", bytes.NewReader(raw))
	if err != nil {
		t.Fatal(err)
	}
	req.Header.Set("Authorization", "Bearer "+gmToken)
	req.Header.Set("X-Fixture-Seat", "gm")
	done := make(chan error, 1)
	go func() {
		res, err := (&http.Client{Timeout: 5 * time.Second}).Do(req)
		if res != nil {
			res.Body.Close()
		}
		done <- err
	}()
	deadline := time.Now().Add(4 * time.Second)
	for !strings.Contains(p.output.snapshot(), "FIXTURE_COMMIT_BARRIER") {
		if time.Now().After(deadline) {
			t.Fatal("actual post-commit barrier not reached", p.output.snapshot())
		}
		time.Sleep(5 * time.Millisecond)
	}
	committed, err := host.InspectRecovery(context.Background(), b)
	if err != nil || committed.Records != 1 {
		t.Fatal("barrier did not follow real SQL commit", err)
	}
	_ = player.SetReadDeadline(time.Now().Add(50 * time.Millisecond))
	if _, _, err := player.ReadMessage(); err == nil {
		t.Fatal("published before post-commit barrier release")
	}
	player.Close()
	// Cancel the original request while the actual daemon is still blocked after
	// SQL commit. SIGTERM then joins its VM; canceled publication cannot send.
	cancel()
	if err := <-done; err == nil {
		t.Fatal("original commit acknowledgement was delivered")
	}
	p.stop(t)
	if err = host.DropDerived(context.Background(), b); err != nil {
		t.Fatal(err)
	}
	config["commit_barrier"] = false
	write()
	p = startDaemon(t, path)
	player = stream(t, p, playerToken, "player", 0)
	restored := readStream(t, player)
	if bytes.Contains(restored, []byte(fixture.PrivateValue)) || !bytes.Contains(restored, []byte(`"state_version":2`)) || !bytes.Contains(restored, []byte(`"event_cursor":1`)) {
		t.Fatal("actual restart did not replay immutable committed history", string(restored))
	}
	status, ack := wire(t, p, "/command", gmToken, "gm", raw)
	if status != http.StatusOK || !bytes.Contains(ack, []byte(`"replayed":true`)) {
		t.Fatal("lost acknowledgement replay", status, string(ack))
	}
	for k := 0; k < 3; k++ {
		if status, ack := wire(t, p, "/sleep", gmToken, "gm", nil); status != http.StatusOK {
			t.Fatal("repeat activation/sleep", status, string(ack))
		}
		conn := stream(t, p, gmToken, "gm", 1)
		_ = readStream(t, conn)
		conn.Close()
	}
	player.Close()
	p.stop(t)
	after, err := host.InspectRecovery(context.Background(), b)
	if err != nil || after.ImmutableHash != committed.ImmutableHash {
		t.Fatal("restart/activation repeated a task or external intent", err)
	}
	facts, err := host.Inspect(context.Background(), b)
	if err != nil || facts.Version != 2 || facts.Tasks != 2 || facts.Continuations != 1 || facts.Outbox != 3 {
		t.Fatal("recorded effects repeated", err)
	}
	t.Logf("TEST-DATA-001 actual platformd post-commit cut; state_hash=%s immutable_hash=%s records=%d; SIGTERM daemon and runner joins verified", eventstore.Digest(facts.State), after.ImmutableHash, after.Records)
}
