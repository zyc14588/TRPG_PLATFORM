// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"bytes"
	"context"
	"encoding/binary"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/ipc"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
)

var testRunner string

func TestMain(m *testing.M) {
	dir, err := os.MkdirTemp("", "b002-cli-runner-")
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	testRunner = filepath.Join(dir, "lua-runner")
	build := exec.Command("go", "build", "-trimpath", "-o", testRunner, "./cmd/lua-runner")
	build.Dir = "../.."
	if out, err := build.CombinedOutput(); err != nil {
		fmt.Fprintln(os.Stderr, string(out), err)
		os.RemoveAll(dir)
		os.Exit(1)
	}
	code := m.Run()
	os.RemoveAll(dir)
	os.Exit(code)
}

func TestRunnerProductionBoundaryAndBudgets(t *testing.T) {
	t.Setenv("B002_PRIVATE_CREDENTIAL", "must-not-reach-runner")
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	c, err := ipc.Start(ctx, testRunner, profile.Config{Limits: profile.DefaultLimits()})
	if err != nil {
		t.Fatal(err)
	}
	defer c.Kill()
	if c.PID() == os.Getpid() {
		t.Fatal("engine is not in a separate process")
	}
	script := `assert(_VERSION=="Lua 5.5");assert(io==nil and os==nil and debug==nil and package==nil and load==nil);assert(exec==nil and socket==nil and ffi==nil and http==nil);return 7`
	r, err := c.Call(ctx, ipc.Request{Operation: "execute", Source: []byte(script)})
	if err != nil || r.Result.Audit.Level != "AUDIT-0" || r.Result.Values[0].Number != "7" {
		t.Fatalf("%#v %v", r, err)
	}
	env, err := os.ReadFile(fmt.Sprintf("/proc/%d/environ", c.PID()))
	if err != nil || bytes.Contains(env, []byte("B002_PRIVATE_CREDENTIAL")) || bytes.Contains(env, []byte("must-not-reach-runner")) {
		t.Fatalf("credential isolation: %v", err)
	}
	status, err := os.ReadFile(fmt.Sprintf("/proc/%d/status", c.PID()))
	if err != nil || !bytes.Contains(status, []byte("NoNewPrivs:\t1")) {
		t.Fatalf("NoNewPrivs missing: %v", err)
	}
	limits, err := os.ReadFile(fmt.Sprintf("/proc/%d/limits", c.PID()))
	if err != nil {
		t.Fatal(err)
	}
	foundData, foundCPU, foundCore := false, false, false
	for _, line := range strings.Split(string(limits), "\n") {
		fields := strings.Fields(line)
		if strings.HasPrefix(line, "Max data size") {
			foundData = len(fields) >= 5 && fields[3] == "134217728" && fields[4] == "134217728"
		}
		if strings.HasPrefix(line, "Max cpu time") {
			foundCPU = len(fields) >= 5 && fields[3] != "unlimited"
		}
		if strings.HasPrefix(line, "Max core file size") {
			foundCore = len(fields) >= 6 && fields[4] == "0" && fields[5] == "0"
		}
	}
	if !foundData || !foundCPU || !foundCore {
		t.Fatal("kernel budgets missing", string(limits))
	}
	if _, err := c.Call(ctx, ipc.Request{Operation: "execute", Source: []byte(`while true do end`)}); err == nil {
		t.Fatal("instruction budget bypass")
	}
	if _, err := c.Call(ctx, ipc.Request{Operation: "execute", Source: []byte(`return 1`)}); profile.Code(err) != profile.ErrPoisoned {
		t.Fatal("runner continued after budget failure", err)
	}
}

func TestRunnerRejectsMalformedIPC(t *testing.T) {
	oversize := make([]byte, 4)
	binary.BigEndian.PutUint32(oversize, uint32(ipc.MaxFrameBytes+1))
	frame := func(body string) []byte {
		var b bytes.Buffer
		if err := ipc.WriteFrame(&b, []byte(body)); err != nil {
			t.Fatal(err)
		}
		return b.Bytes()
	}
	for _, raw := range [][]byte{oversize, {0, 0, 0, 8, '{'}, frame(`{"version":1,"version":1,"id":1,"operation":"initialize"}`), frame(`{"version":1,"id":1,"operation":"execute","audit_level":"OFF"}`), frame(`{"version":9,"id":1,"operation":"execute"}`)} {
		ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
		cmd := exec.CommandContext(ctx, testRunner, "serve")
		cmd.Stdin = bytes.NewReader(raw)
		out, err := cmd.CombinedOutput()
		cancel()
		if err == nil || strings.Contains(string(out), "goroutine") {
			t.Fatalf("malformed IPC accepted or internals leaked: %s %v", out, err)
		}
	}
}
