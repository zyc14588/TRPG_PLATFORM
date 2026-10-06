// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//go:build linux && m1_acceptance

package fixtureminimal

import (
	"context"
	"crypto/rand"
	"debug/buildinfo"
	"encoding/hex"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/tests/m1/evidence"
)

func TestActualFixedReplayEveryBoundaryAndSafePair(t *testing.T) {
	root := RepoRoot()
	sha, err := evidence.Candidate(root, "HEAD")
	if err != nil {
		t.Fatal(err)
	}
	dsn := os.Getenv("TRPG_M1_FIXTURE_PG_DSN")
	if RequireFixtureDSN(dsn) != nil {
		t.Fatal("TRPG_M1_FIXTURE_PG_DSN must name the explicit disposable fixture")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Minute)
	defer cancel()
	dir := t.TempDir()
	source := filepath.Join(dir, "source")
	if err = evidence.BuildCheckout(ctx, root, sha, source); err != nil {
		t.Fatal(err)
	}
	runner := filepath.Join(dir, "lua-runner")
	cmd := exec.CommandContext(ctx, "go", "build", "-buildvcs=true", "-trimpath", "-o", runner, "./cmd/lua-runner")
	cmd.Dir = source
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatal("source Runner build failed", err, checkpoint.Hash(out))
	}
	info, err := buildinfo.ReadFile(runner)
	if err != nil {
		t.Fatal(err)
	}
	revision, modified := "", ""
	for _, s := range info.Settings {
		if s.Key == "vcs.revision" {
			revision = s.Value
		}
		if s.Key == "vcs.modified" {
			modified = s.Value
		}
	}
	if revision != sha || modified != "false" {
		t.Fatal("Runner binary is not bound to clean exact source")
	}
	raw, err := os.ReadFile(runner)
	if err != nil {
		t.Fatal(err)
	}
	var nonce [8]byte
	if _, err = rand.Read(nonce[:]); err != nil {
		t.Fatal(err)
	}
	result, runErr := RunFixed(ctx, sha, dsn, dir, "m1-fixed-"+hex.EncodeToString(nonce[:]), "fixture", install.RuntimeConfig{Runner: runner, SHA256: checkpoint.Hash(raw), Limits: profile.DefaultLimits()})
	path := os.Getenv("TRPG_M1_FIXED_EVIDENCE")
	if path == "" {
		path = filepath.Join(root, "tests/fixture-minimal/.artifacts/fixed-replay.json")
	}
	if err = evidence.Write(path, result); err != nil {
		t.Fatal("fixed machine evidence write failed", err)
	}
	if runErr != nil || result.Verdict != "PASS" {
		t.Fatal("fixed replay failed", runErr)
	}
	if _, err = evidence.Candidate(root, sha); err != nil {
		t.Fatal(err)
	}
	t.Logf("TEST-QUALITY-003 candidate=%s replay_boundaries=%d reaped=%d artifact=%s", sha, len(result.Replays), result.ActualReapedPIDs, path)
}
