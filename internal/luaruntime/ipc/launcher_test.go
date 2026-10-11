//go:build linux

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package ipc

import (
	"context"
	"errors"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func originalRunner(t *testing.T) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "lua-runner")
	cmd := exec.Command("go", "build", "-trimpath", "-o", path, "../../../cmd/lua-runner")
	if out, e := cmd.CombinedOutput(); e != nil {
		t.Fatalf("original runner build: %v: %s", e, out)
	}
	return path
}
func assertOriginalParentAndGone(t *testing.T, c *Client, unknown bool) {
	t.Helper()
	pid := c.PID()
	c.Kill() // this joins the original Cmd.Wait even after a missing caller ACK
	if _, e := os.Stat(fmt.Sprintf("/proc/%d", pid)); !os.IsNotExist(e) {
		t.Fatal("actual child remains after original Wait", pid, e)
	}
	exit, e := c.StopAndWait(context.Background())
	if unknown {
		if !errors.Is(e, ErrUnknownExit) || exit.Reaped || exit.PID != pid {
			t.Fatal("UNKNOWN was promoted to success")
		}
	} else if e != nil || !exit.Reaped || exit.PID != pid {
		t.Fatal("actual parent Wait not acknowledged", e)
	}
}
func TestNilLauncherRetainsOriginalProcessAndWait(t *testing.T) {
	r, e := Launch(context.Background(), nil, originalRunner(t), profile.Config{Limits: profile.DefaultLimits()})
	if e != nil {
		t.Fatal(e)
	}
	c, ok := r.(*Client)
	if !ok {
		t.Fatal("nil launcher did not use original local client")
	}
	t.Cleanup(c.Kill)
	status, e := os.ReadFile(fmt.Sprintf("/proc/%d/status", c.PID()))
	if e != nil {
		t.Fatal(e)
	}
	if !strings.Contains(string(status), fmt.Sprintf("PPid:\t%d\n", os.Getpid())) {
		t.Fatal("local client is not the actual process parent")
	}
	out, e := c.Call(context.Background(), Request{Operation: "execute", Source: []byte("return 42")})
	if e != nil || out.PID != c.PID() || len(out.Result.Values) != 1 || out.Result.Values[0].Number != "42" {
		t.Fatal("original PID-bound call failed", e)
	}
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	exit, e := c.StopAndWait(ctx)
	if e != nil || !exit.Reaped {
		t.Fatal("Wait acknowledgement missing", e)
	}
	assertOriginalParentAndGone(t, c, false)
}
func TestCanceledWaitAcknowledgementStaysUnknown(t *testing.T) {
	c, e := Start(context.Background(), originalRunner(t), profile.Config{Limits: profile.DefaultLimits()})
	if e != nil {
		t.Fatal(e)
	}
	t.Cleanup(c.Kill)
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	exit, e := c.StopAndWait(ctx)
	if !errors.Is(e, ErrUnknownExit) || exit.Reaped || exit.PID != c.PID() {
		t.Fatal("canceled Wait fabricated proof")
	}
	assertOriginalParentAndGone(t, c, true)
	if _, e = c.Call(context.Background(), Request{Operation: "execute", Source: []byte("return 43")}); e == nil {
		t.Fatal("uncertain runner reused")
	}
}
