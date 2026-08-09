// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"bytes"
	"context"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/ipc"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
)

func TestProfileCommandReportsSafeProductionDefaults(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if code := run(context.Background(), []string{"profile"}, strings.NewReader(""), &stdout, &stderr); code != 0 {
		t.Fatalf("profile exit=%d stderr=%s", code, stderr.String())
	}
	for _, expected := range []string{
		`"profile_id":"platform-lua-5.5-p1"`,
		`"runtime_version":"github.com/arnodel/golua@v0.2.0"`,
		`"runtime_license":"Apache-2.0"`,
		`"source_only":true`,
		`"production_debug":false`,
		`"debug"`,
		`"native-modules"`,
	} {
		if !strings.Contains(stdout.String(), expected) {
			t.Fatalf("profile output missing %s: %s", expected, stdout.String())
		}
	}
}

func TestServeUsesTypedProductionLifecycleProtocol(t *testing.T) {
	create, err := ipc.EncodeRequest("create", ipc.Create{SessionID: "runner-session", LuaProfile: profile.ProductionID, RuntimeVersion: profile.RuntimeIdentity})
	if err != nil {
		t.Fatal(err)
	}
	eval, err := ipc.EncodeRequest("eval", ipc.Eval{SessionID: "runner-session", ChunkName: "probe", Source: "return io == nil and debug == nil and package == nil"})
	if err != nil {
		t.Fatal(err)
	}
	destroy, err := ipc.EncodeRequest("destroy", ipc.Destroy{SessionID: "runner-session"})
	if err != nil {
		t.Fatal(err)
	}
	input := bytes.NewBuffer(nil)
	for _, message := range [][]byte{create, eval, destroy} {
		input.Write(message)
		input.WriteByte('\n')
	}
	var stdout, stderr bytes.Buffer
	if code := run(context.Background(), []string{"serve"}, input, &stdout, &stderr); code != 0 {
		t.Fatalf("serve exit=%d stderr=%s", code, stderr.String())
	}
	if strings.Count(stdout.String(), `"result":"PASS"`) != 3 || !strings.Contains(stdout.String(), `"boolean":true`) {
		t.Fatalf("serve output = %s", stdout.String())
	}
}

func TestEvidenceRejectsUnboundCandidate(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if code := run(context.Background(), []string{"evidence", "--candidate-sha", "latest"}, strings.NewReader(""), &stdout, &stderr); code == 0 {
		t.Fatal("evidence accepted an unbound candidate")
	}
	if stdout.Len() != 0 {
		t.Fatalf("invalid evidence emitted machine report: %s", stdout.String())
	}
}

func TestEvidenceSpecsAreUniqueAndExact(t *testing.T) {
	want := map[string]string{
		"TEST-LUA-001": "go test -count=1 ./internal/luaruntime/profile/... ./cmd/lua-runner/...",
		"TEST-LUA-002": "go test -count=1 ./internal/luaruntime/vm/... ./internal/luaruntime/checkpoint/...",
	}
	if len(evidenceSpecs) != len(want) {
		t.Fatalf("evidence spec count = %d, want %d", len(evidenceSpecs), len(want))
	}
	seen := make(map[string]bool, len(evidenceSpecs))
	for _, spec := range evidenceSpecs {
		if seen[spec.id] {
			t.Fatalf("duplicate Test ID %s", spec.id)
		}
		seen[spec.id] = true
		if spec.command != want[spec.id] {
			t.Fatalf("%s command = %q, want %q", spec.id, spec.command, want[spec.id])
		}
	}
}

func TestLicensesCommandIncludesPinnedRuntimeLicense(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if code := run(context.Background(), []string{"licenses"}, strings.NewReader(""), &stdout, &stderr); code != 0 {
		t.Fatalf("licenses exit=%d stderr=%s", code, stderr.String())
	}
	if !strings.HasPrefix(stdout.String(), "github.com/arnodel/golua@v0.2.0\n") || !strings.Contains(stdout.String(), "Apache License") {
		t.Fatalf("licenses output is incomplete: %s", stdout.String())
	}
}
