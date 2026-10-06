//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package replay_test

import (
	"context"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/projection"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/recovery"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/session/testdata"
	replayfixture "github.com/zyc14588/TRPG_PLATFORM/internal/session/testdata/replay"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
	"reflect"
	"strings"
	"testing"
)

func inspection(t *testing.T, e *environment, b data.Binding) postgres.RecoveryInspection {
	t.Helper()
	v, err := e.host.InspectRecovery(context.Background(), b)
	if err != nil {
		t.Fatal(err)
	}
	return v
}
func populated(t *testing.T, source string, n int) (*environment, data.Binding) {
	t.Helper()
	e := setup(t, source)
	t.Cleanup(func() { e.assertReaped(t) })
	b := e.create(t, "session")
	r := newRig(t, e, b, e.host)
	for k := 1; k <= n; k++ {
		got, err := r.registry.Submit(context.Background(), r.gm, envelope(b, fmt.Sprintf("command-%d", k), "increment", uint64(k)))
		if err != nil || got.Version != uint64(k+1) {
			t.Fatalf("fixture command %d: %v", k, err)
		}
	}
	if err := r.registry.Sleep(context.Background(), r.gm); err != nil {
		t.Fatal("fixture checkpoint", err)
	}
	if err := r.registry.Close(); err != nil {
		t.Fatal(err)
	}
	return e, b
}
func recoverSession(t *testing.T, e *environment, b data.Binding, repo *postgres.HostRepository) (*install.InstalledSession, recovery.Report, error) {
	t.Helper()
	s, err := recovery.New(repo)
	if err != nil {
		return nil, recovery.Report{}, err
	}
	var report recovery.Report
	installed, err := e.factory(t, repo).Recover(context.Background(), e.request(b.Session), func(ctx context.Context, o install.RecoveryContext) (install.RecoveryResult, error) {
		var err error
		report, err = s.Reconstruct(ctx, o)
		if err != nil {
			return install.RecoveryResult{}, err
		}
		return s.Build(ctx, o)
	})
	return installed, report, err
}
func assertVMFacts(t *testing.T, s *install.InstalledSession, counter int64) {
	t.Helper()
	got, err := s.Commands.Read(context.Background(), s.VM.Token(), hostapi.Command{ID: "read-facts", Principal: "gm", Callback: "project_view", ExpectedVersion: s.VM.StateVersion(), Input: checkpoint.Object(map[string]checkpoint.Value{})})
	if err != nil || !reflect.DeepEqual(got.Result, fixture.State(counter)) {
		t.Fatal("installed VM state/package-data facts differ", err)
	}
}
func seatViews(t *testing.T, e *environment, b data.Binding) (string, string) {
	t.Helper()
	r := newRig(t, e, b, e.host)
	gm, err := r.registry.Reconnect(context.Background(), r.gm, 0, nil)
	if err != nil {
		t.Fatal(err)
	}
	player, err := r.registry.Reconnect(context.Background(), r.player, 0, nil)
	if err != nil {
		t.Fatal(err)
	}
	if _, ok := player.View.Table["secret"]; ok {
		t.Fatal("seat view leaked private fact")
	}
	if err = r.registry.Close(); err != nil {
		t.Fatal(err)
	}
	return eventstore.Digest(gm), eventstore.Digest(player)
}

func TestTEST_DATA_001RealHistoryAndEveryRecoveryBoundary(t *testing.T) {
	e, b := populated(t, replayfixture.FactsSource, 3)
	baseline := inspection(t, e, b)
	if baseline.Records != 3 {
		t.Fatal("missing immutable effects")
	}
	s, report, err := recoverSession(t, e, b, e.host)
	if err != nil {
		t.Fatal(err)
	}
	o := s.Recovery
	assertVMFacts(t, s, 4)
	if !report.CheckpointAccepted {
		t.Fatal("saved head checkpoint not used")
	}
	if err = s.Close(); err != nil {
		t.Fatal(err)
	}
	h, err := e.host.ReadReplayHistory(context.Background(), o.Graph, b)
	if err != nil {
		t.Fatal(err)
	}
	page, err := e.host.ReadJournal(context.Background(), b, 0, 128)
	if err != nil {
		t.Fatal(err)
	}
	for k, r := range h.Records {
		if !r.Complete || eventstore.Validate(r) != nil || r.Inputs.Time != 1000 || !reflect.DeepEqual(r.Inputs.Random, []int64{7}) || !reflect.DeepEqual(r.Inputs.ToolResults, []checkpoint.Value{checkpoint.Int(7)}) || r.Inputs.Command.Table["payload"].Table["delta"].Number != "1" || r.Events[0].SchemaVersion != 1 || r.Events[0].SchemaHash != o.EventSchemas[fixture.PackageID+"/change"].Digest() || !reflect.DeepEqual(r.Events[0], page.Events[k].Event) {
			t.Fatal("lost complete effects/original inputs/schema bytes")
		}
	}
	want, _, err := projection.Rebuild(h, o.Metadata, nil, o.ValidateRecord)
	if err != nil {
		t.Fatal(err)
	}
	gmView, playerView := seatViews(t, e, b)
	prefix, err := projection.Genesis(h.Creation)
	if err != nil {
		t.Fatal(err)
	}
	for n := 0; n <= 3; n++ {
		t.Run(fmt.Sprintf("snapshot-and-checkpoint-boundary-%d", n), func(t *testing.T) {
			if err := e.host.DropDerived(context.Background(), b); err != nil {
				t.Fatal(err)
			}
			c, err := projection.Seal(o.Metadata, prefix)
			if err != nil {
				t.Fatal(err)
			}
			m := eventstore.Copy(c.Metadata)
			cp := data.CheckpointCache{Binding: b, Version: prefix.Version, Cursor: prefix.Cursor, StateSchema: m.StateSchema, CheckpointSchema: m.CheckpointSchema, Value: prefix.State, Hash: eventstore.Digest(prefix.State), Recovery: &m, HistoryHash: prefix.HistoryHash}
			if err = e.host.StageRecoveryCache(context.Background(), b, &c, &cp); err != nil {
				t.Fatal(err)
			}
			installed, got, err := recoverSession(t, e, b, e.host)
			if err != nil {
				t.Fatal(err)
			}
			assertVMFacts(t, installed, 4)
			if !got.SnapshotAccepted || got.CheckpointAccepted != (n == 3) || !reflect.DeepEqual(got.Image, want) {
				t.Fatal("boundary changed facts/cache qualification")
			}
			if err = installed.Close(); err != nil {
				t.Fatal(err)
			}
			gm, player := seatViews(t, e, b)
			if gm != gmView || player != playerView || inspection(t, e, b).ImmutableHash != baseline.ImmutableHash {
				t.Fatal("recovery changed immutable history or permitted seat views")
			}
			t.Logf("TEST-DATA-001 boundary=%d state_hash=%s history_hash=%s event_cursor=%d gm_view=%s player_view=%s", n, eventstore.Digest(got.Image.State), got.Image.HistoryHash, got.Image.Cursor, gm, player)
		})
		if n < 3 {
			prefix, err = projection.Apply(prefix, h.Records[n])
			if err != nil {
				t.Fatal(err)
			}
		}
	}
	if err = e.host.DropDerived(context.Background(), b); err != nil {
		t.Fatal(err)
	}
	installed, empty, err := recoverSession(t, e, b, e.host)
	if err != nil {
		t.Fatal(err)
	}
	assertVMFacts(t, installed, 4)
	if empty.SnapshotAccepted || empty.CheckpointAccepted || !reflect.DeepEqual(empty.Image, want) {
		t.Fatal("mutable cache used as genesis")
	}
	if err = installed.Close(); err != nil {
		t.Fatal(err)
	}
	if inspection(t, e, b).ImmutableHash != baseline.ImmutableHash {
		t.Fatal("empty recovery rewrote history/tasks/outbox")
	}
}

func TestRealCorruptAndSelfHashedCacheMismatchesFallBack(t *testing.T) {
	e, b := populated(t, replayfixture.FactsSource, 2)
	baseline := inspection(t, e, b)
	s, report, err := recoverSession(t, e, b, e.host)
	if err != nil {
		t.Fatal(err)
	}
	good, err := projection.Seal(s.Recovery.Metadata, report.Image)
	if err != nil {
		t.Fatal(err)
	}
	cp, err := e.host.ReadCheckpoint(context.Background(), b)
	if err != nil {
		t.Fatal(err)
	}
	s.Close()
	changes := map[string]func(*projection.Cache, *data.CheckpointCache){
		"lock": func(c *projection.Cache, p *data.CheckpointCache) {
			c.Metadata.Session.DependencyLock = eventstore.Digest("wrong")
		},
		"package-artifacts": func(c *projection.Cache, p *data.CheckpointCache) {
			c.Metadata.ArtifactsHash = eventstore.Digest("wrong")
		},
		"package-content": func(c *projection.Cache, p *data.CheckpointCache) {
			c.Metadata.Session.PackageHashes[fixture.PackageID] = eventstore.Digest("wrong")
		},
		"profile": func(c *projection.Cache, p *data.CheckpointCache) { c.Metadata.Session.LuaProfile = "wrong" },
		"runtime": func(c *projection.Cache, p *data.CheckpointCache) { c.Metadata.Session.RuntimeVersion = "wrong" },
		"runner":  func(c *projection.Cache, p *data.CheckpointCache) { c.Metadata.RunnerHash = eventstore.Digest("wrong") },
		"state-schema": func(c *projection.Cache, p *data.CheckpointCache) {
			c.Metadata.StateSchema = eventstore.Digest("wrong")
		},
		"event-schemas": func(c *projection.Cache, p *data.CheckpointCache) {
			c.Metadata.EventSchemas = eventstore.Digest("wrong")
		},
		"checkpoint-schema": func(c *projection.Cache, p *data.CheckpointCache) {
			c.Metadata.CheckpointSchema = eventstore.Digest("wrong")
		},
		"state-version": func(c *projection.Cache, p *data.CheckpointCache) { c.Metadata.Session.StateVersion++ },
		"self-hashed-false-facts": func(c *projection.Cache, p *data.CheckpointCache) {
			c.Image.State.Table["counter"] = checkpoint.Int(999)
			p.Value = eventstore.Copy(c.Image.State)
			p.Hash = eventstore.Digest(p.Value)
		},
		"corrupt-hash": func(c *projection.Cache, p *data.CheckpointCache) { p.Hash = eventstore.Digest("corrupt") },
	}
	for name, change := range changes {
		t.Run(name, func(t *testing.T) {
			c, p := eventstore.Copy(good), eventstore.Copy(cp)
			change(&c, &p)
			m := eventstore.Copy(c.Metadata)
			p.Recovery = &m
			c.Hash = ""
			c.Hash = eventstore.Digest(c)
			if name == "corrupt-hash" {
				c.Hash = eventstore.Digest("corrupt")
			}
			if err := e.host.DropDerived(context.Background(), b); err != nil {
				t.Fatal(err)
			}
			if err := e.host.StageRecoveryCache(context.Background(), b, &c, &p); err != nil {
				t.Fatal(err)
			}
			installed, got, err := recoverSession(t, e, b, e.host)
			if err != nil {
				t.Fatal(err)
			}
			assertVMFacts(t, installed, 3)
			installed.Close()
			if got.SnapshotAccepted || got.CheckpointAccepted || !reflect.DeepEqual(got.Image, report.Image) || inspection(t, e, b).ImmutableHash != baseline.ImmutableHash {
				t.Fatal("untrusted cache rewrote authority")
			}
		})
	}
}

func TestRealCommandEvidenceSQLRollbackAndRecoveryFailureRetry(t *testing.T) {
	e := setup(t, replayfixture.FactsSource)
	t.Cleanup(func() { e.assertReaped(t) })
	b := e.create(t, "session")
	before := inspection(t, e, b)
	bad, err := e.host.WithTransactionAbort("after-replay-evidence")
	if err != nil {
		t.Fatal(err)
	}
	r := newRig(t, e, b, bad)
	if _, err = r.registry.Reconnect(context.Background(), r.gm, 0, nil); err != nil {
		t.Fatal(err)
	}
	before = inspection(t, e, b) // activation repairs a cache before the command transaction
	if _, err = r.registry.Submit(context.Background(), r.gm, envelope(b, "command-1", "increment", 1)); err == nil {
		t.Fatal("actual PostgreSQL abort hidden")
	}
	r.registry.Close()
	if got := inspection(t, e, b); got != before {
		t.Fatal("partial command/evidence survived SQL rollback")
	}
	r = newRig(t, e, b, e.host)
	if _, err = r.registry.Submit(context.Background(), r.gm, envelope(b, "command-1", "increment", 1)); err != nil {
		t.Fatal(err)
	}
	r.registry.Close()
	baseline := inspection(t, e, b)
	for _, point := range []string{"recovery-after-state", "recovery-after-data", "recovery-after-cache", "recovery-before-commit"} {
		t.Run(point, func(t *testing.T) {
			if err := e.host.DropDerived(context.Background(), b); err != nil {
				t.Fatal(err)
			}
			before := inspection(t, e, b)
			bad, err := e.host.WithRecoveryTransactionAbort(point)
			if err != nil {
				t.Fatal(err)
			}
			installed, _, err := recoverSession(t, e, b, bad)
			if err == nil || installed != nil {
				t.Fatal("actual repair SQL abort hidden")
			}
			if got := inspection(t, e, b); got != before {
				t.Fatal("partial derived repair or history mutation survived")
			}
			installed, _, err = recoverSession(t, e, b, e.host)
			if err != nil {
				t.Fatal(err)
			}
			assertVMFacts(t, installed, 2)
			installed.Close()
			if inspection(t, e, b).ImmutableHash != baseline.ImmutableHash {
				t.Fatal("retry changed history")
			}
		})
	}
}

func TestRealCheckpointRestoreVMFailureReapedAndFullReplayRetry(t *testing.T) {
	source := strings.TrimSuffix(replayfixture.FactsSource, "return M\n") + `M.restore_checkpoint=function(input) if input.counter~=nil and input.counter>=4 then error("synthetic restore failure") end return {} end
return M
`
	e, b := populated(t, source, 3)
	before := inspection(t, e, b)
	installed, _, err := recoverSession(t, e, b, e.host)
	if err == nil || installed != nil {
		t.Fatal("restore failure not reached")
	}
	if inspection(t, e, b).ImmutableHash != before.ImmutableHash {
		t.Fatal("VM failure changed history")
	}
	if err = e.host.DropDerived(context.Background(), b); err != nil {
		t.Fatal(err)
	}
	installed, got, err := recoverSession(t, e, b, e.host)
	if err != nil {
		t.Fatal(err)
	}
	if got.CheckpointAccepted {
		t.Fatal("removed checkpoint used")
	}
	assertVMFacts(t, installed, 4)
	installed.Close()
	if inspection(t, e, b).ImmutableHash != before.ImmutableHash {
		t.Fatal("retry history changed")
	}
}

func TestRealLegacyIncompleteHistoryFailsClosed(t *testing.T) {
	e := setup(t, "")
	t.Cleanup(func() { e.assertReaped(t) })
	b := e.create(t, "legacy")
	creation, err := e.host.ReadCreation(context.Background(), b)
	if err != nil {
		t.Fatal(err)
	}
	h := data.Header{Binding: b, Principal: "operator", CommandID: "legacy-direct", ExpectedVersion: 1, Fingerprint: eventstore.Digest("legacy-input")}
	tx, err := e.host.Begin(context.Background(), h)
	if err != nil {
		t.Fatal(err)
	}
	defer tx.Rollback()
	c := data.Commit{Header: h, State: fixture.State(2), SchemaHash: creation.SchemaHash, Result: checkpoint.Int(2), Inputs: data.Inputs{Callback: "command", Command: fixture.CommandInput("increment", 1)}, Events: []data.Event{{ID: "legacy-event", Type: fixture.PackageID + "/change", Payload: fixture.State(2)}}, Audit: []data.Audit{{Level: "AUDIT-0", ArgumentsHash: eventstore.Digest("arguments"), ResultHash: eventstore.Digest("result")}}}
	if _, err = tx.Commit(context.Background(), c); err != nil {
		t.Fatal(err)
	}
	before := inspection(t, e, b)
	if before.Records != 1 {
		t.Fatal("legacy qualification not recorded")
	}
	if installed, _, err := recoverSession(t, e, b, e.host); err == nil || installed != nil {
		t.Fatal("incomplete legacy evidence silently trusted")
	}
	if inspection(t, e, b) != before {
		t.Fatal("failed legacy recovery changed bytes")
	}
	// This is a trusted SQL legacy arrangement, not a new VM-approved command.
	t.Log("legacy direct client: complete=false; authenticated replay rejected; all original bytes preserved")
}

func TestRealTenantGraphACLAndPolicyReauthentication(t *testing.T) {
	e, b := populated(t, replayfixture.FactsSource, 1)
	s, _, err := recoverSession(t, e, b, e.host)
	if err != nil {
		t.Fatal(err)
	}
	g := s.Graph
	s.Close()
	before := inspection(t, e, b)
	for _, bad := range []data.Binding{{Workspace: "other-workspace", Session: b.Session, GraphHash: b.GraphHash}, {Workspace: b.Workspace, Session: b.Session, GraphHash: eventstore.Digest("other-lock")}, {Workspace: b.Workspace, Session: "missing-session", GraphHash: b.GraphHash}} {
		if _, err = e.host.ReadReplayHistory(context.Background(), g, bad); err == nil {
			t.Fatal("tenant/graph substitution accepted")
		}
	}
	access, err := store.NewAccess(map[store.Credential][]store.Membership{credential: {{Principal: "revoked-principal", Workspace: e.workspace, Read: true, Install: false}}})
	if err != nil {
		t.Fatal(err)
	}
	reader, err := store.NewReader(e.repository, e.objects, access, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	calls := e.executionCount()
	rebuild, err := recovery.New(e.host)
	if err != nil {
		t.Fatal(err)
	}
	factory := func(reader *store.Reader, policy *install.Policy) *install.SessionFactory {
		f, err := install.NewSessionFactory(install.SessionOptions{Reader: reader, Policy: policy, Repository: e.host, Runtime: runtime(), Execution: e.observe, Audit: func(data.Audit) error { return nil }, Validate: func(context.Context, data.Commit) error { return nil }})
		if err != nil {
			t.Fatal(err)
		}
		return f
	}
	if installed, err := factory(reader, e.policy).Recover(context.Background(), e.request(b.Session), rebuild.Build); err == nil || installed != nil {
		t.Fatal("revoked principal/grant resumed")
	}
	config := e.config
	config.Context = "development"
	changed, err := install.NewPolicy(config)
	if err != nil {
		t.Fatal(err)
	}
	if installed, err := factory(e.reader, changed).Recover(context.Background(), e.request(b.Session), rebuild.Build); err == nil || installed != nil {
		t.Fatal("changed current policy resumed")
	}
	if e.executionCount() != calls {
		t.Fatal("ACL/policy denial launched fresh VM")
	}

	if inspection(t, e, b) != before {
		t.Fatal("denials changed authority")
	}
}
