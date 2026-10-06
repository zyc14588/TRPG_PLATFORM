// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//go:build linux && m1_acceptance

package fixtureminimal

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"strings"
	"sync"
	"syscall"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/migration"
	"github.com/zyc14588/TRPG_PLATFORM/internal/projection"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/migration"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/realtime"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/recovery"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

type ReplayProof struct {
	StateHash, SnapshotHash, CheckpointHash, HistoryHash, EventsHash, GMViewHash, PlayerViewHash string
	SnapshotBoundary                                                                             int
	SnapshotAccepted, CheckpointAccepted                                                         bool
}
type FixedResult struct {
	Candidate            string            `json:"candidate_sha"`
	Verdict              string            `json:"verdict"`
	SourceCatalogHash    string            `json:"source_catalog_sha256"`
	RunnerHash           string            `json:"runner_sha256"`
	OriginalEventsHash   string            `json:"original_events_hash"`
	OriginalSnapshotHash string            `json:"original_snapshot_hash"`
	Replays              []ReplayProof     `json:"replays"`
	Upgrade, Restore     migration.Result  `json:"-"`
	MigrationHashes      map[string]string `json:"migration_hashes"`
	ObservedRunnerPIDs   []int             `json:"observed_runner_pids"`
	ActualReapedPIDs     int               `json:"actual_esrch_runner_pids"`
}

type fixedEnvironment struct {
	pair               fixture.Pair
	host               *postgres.HostRepository
	factory            *install.SessionFactory
	workspace, session string
	close              func() error
	mu                 sync.Mutex
	executions         []install.Execution
}

func RequireFixtureDSN(dsn string) error {
	u, err := url.Parse(dsn)
	if err != nil || (u.Scheme != "postgres" && u.Scheme != "postgresql") || u.Hostname() != "127.0.0.1" || !strings.HasSuffix(strings.TrimPrefix(u.Path, "/"), "_fixture") {
		return fmt.Errorf("explicit disposable loopback PostgreSQL fixture DSN required")
	}
	return nil
}

func openFixed(ctx context.Context, dsn, dir, workspace, session string, r install.RuntimeConfig) (e *fixedEnvironment, err error) {
	if err = RequireFixtureDSN(dsn); err != nil {
		return nil, err
	}
	e = &fixedEnvironment{workspace: workspace, session: session}
	var closers []func() error
	e.close = func() error {
		var errs []error
		for i := len(closers) - 1; i >= 0; i-- {
			errs = append(errs, closers[i]())
		}
		return errors.Join(errs...)
	}
	defer func() {
		if err != nil {
			err = errors.Join(err, e.close())
			e = nil
		}
	}()
	e.pair, err = Load(r)
	if err != nil {
		return e, err
	}
	policy, err := install.NewPolicy(e.pair.Config)
	if err != nil {
		return e, err
	}
	root, stage := filepath.Join(dir, "objects"), filepath.Join(dir, "stage")
	for _, path := range []string{root, stage} {
		if err = os.MkdirAll(path, 0700); err != nil {
			return e, err
		}
	}
	objects, err := object.Open(root)
	if err != nil {
		return e, err
	}
	closers = append(closers, objects.Close)
	repo, err := postgres.OpenInstallationRepository(ctx, dsn, objects, extension.DefaultSupport, nil)
	if err != nil {
		return e, err
	}
	closers = append(closers, func() error { repo.Close(); return nil })
	if err = repo.Bootstrap(ctx); err != nil {
		return e, err
	}
	if err = repo.ProvisionWorkspace(ctx, workspace); err != nil {
		return e, err
	}
	e.host, err = postgres.OpenHostRepository(ctx, dsn, nil)
	if err != nil {
		return e, err
	}
	closers = append(closers, func() error { e.host.Close(); return nil })
	if err = e.host.Bootstrap(ctx); err != nil {
		return e, err
	}
	credential := store.Credential("m1-fixed-synthetic-install-credential")
	access, err := store.NewAccess(map[store.Credential][]store.Membership{credential: {{Principal: "operator", Workspace: workspace, Read: true, Install: true}}})
	if err != nil {
		return e, err
	}
	reader, err := store.NewReader(repo, objects, access, extension.DefaultSupport)
	if err != nil {
		return e, err
	}
	observe := func(v install.Execution) error {
		e.mu.Lock()
		defer e.mu.Unlock()
		e.executions = append(e.executions, v)
		return nil
	}
	installer, err := install.New(install.Options{StagingRoot: stage, Policy: policy, Access: access, Objects: objects, Repository: repo, Support: extension.DefaultSupport, Runtime: r, Observe: func(string) error { return nil }, Execution: observe})
	if err != nil {
		return e, err
	}
	input := func(p *archive.Package) (install.Input, error) {
		s, x := p.Export()
		return install.Input{Archive: bytes.NewReader(s.Bytes()), Evidence: e.pair.Evidence[string(p.ArtifactIdentity().Digest())]}, x
	}
	for i, g := range []fixture.Graph{e.pair.Old, e.pair.New} {
		req := install.Request{Credential: credential, Workspace: workspace, ID: fmt.Sprintf("fixed-install-%d", i)}
		if req.Root, err = input(g.Root); err != nil {
			return e, err
		}
		for _, p := range g.Dependencies {
			v, x := input(p)
			if x != nil {
				return e, x
			}
			req.Dependencies = append(req.Dependencies, v)
		}
		if _, err = installer.Install(ctx, req); err != nil {
			return e, err
		}
	}
	e.factory, err = install.NewSessionFactory(install.SessionOptions{Reader: reader, Policy: policy, Repository: e.host, Runtime: r, Execution: observe, Audit: func(data.Audit) error { return nil }, Validate: func(context.Context, data.Commit) error { return nil }})
	return e, err
}
func (e *fixedEnvironment) request(next bool) install.SessionRequest {
	return e.pair.Request("m1-fixed-synthetic-install-credential", e.workspace, e.session, next)
}

func snapshotOf(i projection.Image) data.Snapshot {
	return data.Snapshot{Binding: i.Binding, Version: i.Version, SchemaHash: i.StateSchema, State: i.State, Rows: i.Rows, Quantities: i.Quantities}
}
func views(ctx context.Context, s *install.InstalledSession) (string, string, error) {
	hashes := []string{}
	for _, seat := range []string{"gm", "player"} {
		r, err := s.Commands.Read(ctx, s.VM.Token(), hostapi.Command{ID: "fixed-view-" + seat, Principal: "operator", ExpectedVersion: s.VM.StateVersion(), Callback: "project_view", Input: checkpoint.Object(map[string]checkpoint.Value{"seat_id": checkpoint.Text(seat)})})
		if err != nil {
			return "", "", err
		}
		fields := []string{"counter"}
		if seat == "gm" {
			fields = append(fields, "secret")
		}
		view := realtime.Filter(r.Result, fields)
		if seat == "player" {
			if _, ok := view.Table["secret"]; ok {
				return "", "", fmt.Errorf("private player view")
			}
		}
		hashes = append(hashes, eventstore.Digest(view))
	}
	return hashes[0], hashes[1], nil
}

// RunFixed executes real installed-source commands, then rebuilds every fixed
// history boundary with a fresh production Runner. Its evidence stores hashes.
func RunFixed(ctx context.Context, candidate, dsn, dir, workspace, session string, r install.RuntimeConfig) (result FixedResult, err error) {
	result = FixedResult{Candidate: candidate, Verdict: "FAIL", RunnerHash: r.SHA256, MigrationHashes: map[string]string{}}
	e, err := openFixed(ctx, dsn, dir, workspace, session, r)
	if err != nil {
		return result, err
	}
	var active *install.InstalledSession
	defer func() {
		if active != nil {
			err = errors.Join(err, active.Close())
		}
		err = errors.Join(err, e.close())
		e.mu.Lock()
		defer e.mu.Unlock()
		pids := map[int]bool{}
		for _, v := range e.executions {
			if v.PID > 0 {
				pids[v.PID] = v.Reaped
			}
		}
		for pid, reaped := range pids {
			result.ObservedRunnerPIDs = append(result.ObservedRunnerPIDs, pid)
			if !reaped || syscall.Kill(pid, 0) != syscall.ESRCH {
				err = errors.Join(err, fmt.Errorf("owned production Runner was not reaped"))
			} else {
				result.ActualReapedPIDs++
			}
		}
		if len(pids) == 0 {
			err = errors.Join(err, fmt.Errorf("no production execution observed"))
		}
		sort.Ints(result.ObservedRunnerPIDs)
		if err == nil {
			result.Verdict = "PASS"
		}
	}()
	raw, err := os.ReadFile(filepath.Join(SourceRoot(), "catalog.json"))
	if err != nil {
		return result, err
	}
	result.SourceCatalogHash = checkpoint.Hash(raw)
	g, err := ReadGolden()
	if err != nil {
		return result, err
	}
	active, err = e.factory.Create(ctx, e.request(false))
	if err != nil {
		return result, err
	}
	binding := active.Binding
	gm, player, err := views(ctx, active)
	if err != nil {
		return result, err
	}
	if gm != g.GMHashes[0] || player != g.PlayerHashes[0] {
		return result, fmt.Errorf("initial fixed views differ")
	}
	for i, c := range g.Commands {
		receipt, x := active.Commands.Execute(ctx, active.VM.Token(), hostapi.Command{ID: c.ID, Principal: "operator", ExpectedVersion: uint64(i + 1), Input: fixture.Command(c.Delta), Time: c.Time, Random: c.Random})
		if x != nil {
			return result, x
		}
		if receipt.Version != c.Version || receipt.Cursor != c.Cursor || receipt.Result.Number != fmt.Sprint(c.Counter) || receipt.Inputs.Time != c.Time || !reflect.DeepEqual(receipt.Inputs.Random, c.Random) || len(receipt.Events) != 1 || eventstore.Digest(receipt.Events[0].Payload) != g.EventHashes[i] {
			return result, fmt.Errorf("fixed command facts differ")
		}
		gm, player, x = views(ctx, active)
		if x != nil {
			return result, x
		}
		if gm != g.GMHashes[i+1] || player != g.PlayerHashes[i+1] {
			return result, fmt.Errorf("fixed command seat views differ")
		}
	}
	service, err := recovery.New(e.host)
	if err != nil {
		return result, err
	}
	baseline, err := service.Reconstruct(ctx, active.Recovery)
	if err != nil {
		return result, err
	}
	var expected data.Snapshot
	if checkpoint.StrictDecode(g.Snapshot["after-fixed-commands"], &expected, 64<<10) != nil {
		return result, fmt.Errorf("invalid fixed snapshot")
	}
	expected.Binding = binding
	if !reflect.DeepEqual(snapshotOf(baseline.Image), expected) {
		return result, fmt.Errorf("fixed complete snapshot differs")
	}
	result.OriginalSnapshotHash = eventstore.Digest(expected)
	checkpointReceipt, err := active.Commands.Read(ctx, active.VM.Token(), hostapi.Command{ID: "fixed-create-checkpoint", Principal: "operator", ExpectedVersion: 3, Callback: "create_checkpoint", Input: checkpoint.Object(map[string]checkpoint.Value{})})
	if err != nil {
		return result, err
	}
	if !reflect.DeepEqual(checkpointReceipt.Result, g.Checkpoint) {
		return result, fmt.Errorf("fixed checkpoint value differs")
	}
	if err = service.Save(ctx, active.Recovery, checkpointReceipt.Result, 3, 2); err != nil {
		return result, err
	}
	options := active.Recovery
	if err = active.Close(); err != nil {
		return result, err
	}
	active = nil
	history, err := e.host.ReadReplayHistory(ctx, options.Graph, binding)
	if err != nil {
		return result, err
	}
	page, err := e.host.ReadJournal(ctx, binding, 0, 128)
	if err != nil {
		return result, err
	}
	if len(page.Events) != 2 || len(history.Records) != 2 {
		return result, fmt.Errorf("missing complete fixed history")
	}
	result.OriginalEventsHash = eventstore.Digest(page.Events)
	for i, record := range history.Records {
		if eventstore.Validate(record) != nil || record.StateHash != g.StateHashes[i+1] || record.Inputs.Time != g.Commands[i].Time || !reflect.DeepEqual(record.Inputs.Random, g.Commands[i].Random) || eventstore.Digest(record.Events[0].Payload) != g.EventHashes[i] || !reflect.DeepEqual(record.Events[0], page.Events[i].Event) {
			return result, fmt.Errorf("original fixed event or input differs")
		}
	}
	immutable, err := e.host.InspectRecovery(ctx, binding)
	if err != nil {
		return result, err
	}
	prefix, err := projection.Genesis(history.Creation)
	if err != nil {
		return result, err
	}
	for boundary := 0; boundary <= len(history.Records); boundary++ {
		if err = e.host.DropDerived(ctx, binding); err != nil {
			return result, err
		}
		cache, x := projection.Seal(options.Metadata, prefix)
		if x != nil {
			return result, x
		}
		metadata := cache.Metadata
		cp := data.CheckpointCache{Binding: binding, Version: prefix.Version, Cursor: prefix.Cursor, StateSchema: metadata.StateSchema, CheckpointSchema: metadata.CheckpointSchema, Value: prefix.State, Hash: eventstore.Digest(prefix.State), Recovery: &metadata, HistoryHash: prefix.HistoryHash}
		if err = e.host.StageRecoveryCache(ctx, binding, &cache, &cp); err != nil {
			return result, err
		}
		active, err = e.factory.Recover(ctx, e.request(false), service.Build)
		if err != nil {
			return result, err
		}
		report, x := service.Reconstruct(ctx, active.Recovery)
		if x != nil {
			return result, x
		}
		gm, player, x = views(ctx, active)
		if x != nil {
			return result, x
		}
		current, x := e.host.InspectRecovery(ctx, binding)
		if x != nil {
			return result, x
		}
		replayedJournal, x := e.host.ReadJournal(ctx, binding, 0, 128)
		if x != nil {
			return result, x
		}
		replayedCheckpoint, x := active.Commands.Read(ctx, active.VM.Token(), hostapi.Command{ID: "replay-checkpoint", Principal: "operator", ExpectedVersion: 3, Callback: "create_checkpoint", Input: checkpoint.Object(map[string]checkpoint.Value{})})
		if x != nil {
			return result, x
		}
		if !reflect.DeepEqual(replayedJournal.Events, page.Events) || !reflect.DeepEqual(replayedCheckpoint.Result, g.Checkpoint) {
			return result, fmt.Errorf("replayed events/checkpoint changed original facts")
		}
		if !reflect.DeepEqual(report.Image, baseline.Image) || eventstore.Digest(snapshotOf(report.Image)) != result.OriginalSnapshotHash || !report.SnapshotAccepted || report.CheckpointAccepted != (boundary == 2) || gm != g.GMHashes[2] || player != g.PlayerHashes[2] || current.ImmutableHash != immutable.ImmutableHash || report.RecordedTime != g.Commands[1].Time || !reflect.DeepEqual(report.RecordedRandom, g.Commands[1].Random) {
			return result, fmt.Errorf("replay boundary changed authoritative facts")
		}
		proof := ReplayProof{StateHash: eventstore.Digest(report.Image.State), SnapshotHash: eventstore.Digest(snapshotOf(report.Image)), CheckpointHash: eventstore.Digest(replayedCheckpoint.Result), HistoryHash: report.Image.HistoryHash, EventsHash: eventstore.Digest(replayedJournal.Events), GMViewHash: gm, PlayerViewHash: player, SnapshotBoundary: boundary, SnapshotAccepted: report.SnapshotAccepted, CheckpointAccepted: report.CheckpointAccepted}
		result.Replays = append(result.Replays, proof)
		if err = active.Close(); err != nil {
			return result, err
		}
		active = nil
		if boundary < len(history.Records) {
			prefix, err = projection.Apply(prefix, history.Records[boundary])
			if err != nil {
				return result, err
			}
		}
	}
	operator, err := migration.New(e.factory, func(ctx context.Context, from, to *store.Graph, b data.Binding, v uint64) (migration.Lease, error) {
		return e.host.AcquireMigration(ctx, from, to, b, v)
	})
	if err != nil {
		return result, err
	}
	request := migration.Request{From: e.request(false), To: e.request(true), ExpectedVersion: 3, PointID: "fixed-before-upgrade", CommandID: "fixed-upgrade", Plan: e.pair.Plan()}
	result.Upgrade, err = operator.Run(ctx, request)
	if err != nil {
		return result, err
	}
	if result.Upgrade.Receipt.Version != 4 || result.Upgrade.Receipt.Cursor != 2 || result.Upgrade.OldStateHash != g.StateHashes[2] || result.Upgrade.NewStateHash != eventstore.Digest(g.Migrated) {
		return result, fmt.Errorf("safe migration changed fixed facts")
	}
	request.From, request.To = request.To, request.From
	request.ExpectedVersion = 4
	request.CommandID = "fixed-restore"
	result.Restore, err = operator.RestorePoint(ctx, request)
	if err != nil {
		return result, err
	}
	if result.Restore.Receipt.Version != 5 || result.Restore.Receipt.Cursor != 2 || result.Restore.PointHash != result.Upgrade.PointHash || result.Restore.NewStateHash != g.StateHashes[2] {
		return result, fmt.Errorf("recorded point restoration differs")
	}
	result.MigrationHashes = map[string]string{"point": result.Upgrade.PointHash, "target_state": result.Upgrade.NewStateHash, "restored_state": result.Restore.NewStateHash, "target_lock": result.Upgrade.LockHash, "restored_lock": result.Restore.LockHash}
	return result, nil
}
