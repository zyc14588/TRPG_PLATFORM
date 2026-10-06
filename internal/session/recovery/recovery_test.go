// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package recovery_test

import (
	"context"
	"errors"
	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/projection"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/recovery"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/session/testdata/replay"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"reflect"
	"testing"
)

// A data-only unit double; PostgreSQL/process proofs live in integration/replay.
type repository struct {
	h                  data.ReplayHistory
	image              *projection.Cache
	cp                 *data.CheckpointCache
	readErr, repairErr error
	repairs, saves     int
	saved              data.CheckpointCache
}

func (r *repository) ReadReplayHistory(ctx context.Context, _ *store.Graph, _ data.Binding) (data.ReplayHistory, error) {
	if e := ctx.Err(); e != nil {
		return data.ReplayHistory{}, e
	}
	return eventstore.Copy(r.h), r.readErr
}
func (r *repository) ReadProjectionCache(context.Context, data.Binding) (projection.Cache, error) {
	if r.image == nil {
		return projection.Cache{}, data.ErrNotFound
	}
	return eventstore.Copy(*r.image), nil
}
func (r *repository) ReadCheckpoint(context.Context, data.Binding) (data.CheckpointCache, error) {
	if r.cp == nil {
		return data.CheckpointCache{}, data.ErrNotFound
	}
	return eventstore.Copy(*r.cp), nil
}
func (r *repository) RepairDerived(_ context.Context, _ *store.Graph, _ projection.Cache) error {
	r.repairs++
	return r.repairErr
}
func (r *repository) SaveCheckpoint(_ context.Context, c data.CheckpointCache) error {
	r.saves++
	r.saved = eventstore.Copy(c)
	return nil
}

func TestReplayConsumesRecordedInputsAndNeverDispatchesRecordedIntents(t *testing.T) {
	h, o, e := fixture.History(3)
	if e != nil {
		t.Fatal(e)
	}
	r := &repository{h: h}
	s, e := recovery.New(r)
	if e != nil {
		t.Fatal(e)
	}
	original := eventstore.Digest(h)
	for n := 0; n < 3; n++ {
		got, e := s.Build(context.Background(), o)
		if e != nil || got.Snapshot.Version != 4 || got.Time != 1000 || !reflect.DeepEqual(got.Random, []int64{7}) {
			t.Fatal(e)
		}
		if eventstore.Digest(r.h) != original || r.saves != 0 {
			t.Fatal("replay rewrote history or emitted checkpoint")
		}
	}
	if r.repairs != 3 {
		t.Fatal("missing derived repair")
	}
	// Tool results and accepted command payload remain original data. There is
	// no external dispatcher in this service interface; Outbox is not consumed.
	if !reflect.DeepEqual(h.Records[2].Inputs.ToolResults, []checkpoint.Value{checkpoint.Int(7)}) || len(r.h.Records[2].Outbox) != 1 {
		t.Fatal("recorded inputs/intents lost")
	}
}
func TestCheckpointCompatibilityIncludesFactsNotOnlySelfHash(t *testing.T) {
	h, o, e := fixture.History(2)
	if e != nil {
		t.Fatal(e)
	}
	i, _, e := projection.Rebuild(h, o.Metadata, nil, o.ValidateRecord)
	if e != nil {
		t.Fatal(e)
	}
	m := eventstore.Copy(o.Metadata)
	m.Session.StateVersion = i.Version
	good := data.CheckpointCache{Binding: o.Binding, Version: i.Version, Cursor: i.Cursor, StateSchema: m.StateSchema, CheckpointSchema: m.CheckpointSchema, Value: i.State, Hash: eventstore.Digest(i.State), Recovery: &m, HistoryHash: i.HistoryHash}
	changes := map[string]func(*data.CheckpointCache){"compatible": func(*data.CheckpointCache) {}, "legacy": func(c *data.CheckpointCache) { c.Recovery = nil }, "lock": func(c *data.CheckpointCache) { c.Recovery.Session.DependencyLock = eventstore.Digest("wrong") }, "package": func(c *data.CheckpointCache) { c.Recovery.Session.PackageHashes["wrong"] = eventstore.Digest("wrong") }, "profile": func(c *data.CheckpointCache) { c.Recovery.Session.LuaProfile = "wrong" }, "runtime": func(c *data.CheckpointCache) { c.Recovery.Session.RuntimeVersion = "wrong" }, "runner": func(c *data.CheckpointCache) { c.Recovery.RunnerHash = eventstore.Digest("wrong") }, "schema": func(c *data.CheckpointCache) { c.CheckpointSchema = eventstore.Digest("wrong") }, "version": func(c *data.CheckpointCache) { c.Version-- }, "cursor": func(c *data.CheckpointCache) { c.Cursor-- }, "history": func(c *data.CheckpointCache) { c.HistoryHash = eventstore.Digest("wrong") }, "corrupt": func(c *data.CheckpointCache) { c.Hash = eventstore.Digest("wrong") }, "self-hashed-false-facts": func(c *data.CheckpointCache) {
		c.Value.Table["counter"] = checkpoint.Int(99)
		c.Hash = eventstore.Digest(c.Value)
	}}
	for name, change := range changes {
		t.Run(name, func(t *testing.T) {
			c := eventstore.Copy(good)
			change(&c)
			r := &repository{h: h, cp: &c}
			s, e := recovery.New(r)
			if e != nil {
				t.Fatal(e)
			}
			got, e := s.Reconstruct(context.Background(), o)
			if e != nil || got.CheckpointAccepted != (name == "compatible") || !reflect.DeepEqual(got.Image, i) {
				t.Fatal("checkpoint became authority", e, got.CheckpointAccepted)
			}
		})
	}
}
func TestFailureRetryCancellationAndCheckpointHeadRace(t *testing.T) {
	h, o, e := fixture.History(2)
	if e != nil {
		t.Fatal(e)
	}
	r := &repository{h: h, repairErr: errors.New("synthetic SQL abort")}
	s, e := recovery.New(r)
	if e != nil {
		t.Fatal(e)
	}
	before := eventstore.Digest(h)
	if _, e = s.Build(context.Background(), o); e == nil {
		t.Fatal("repair failure hidden")
	}
	r.repairErr = nil
	got, e := s.Build(context.Background(), o)
	if e != nil || eventstore.Digest(r.h) != before {
		t.Fatal("retry altered history", e)
	}
	if e = s.Save(context.Background(), o, got.Snapshot.State, 2, 1); !errors.Is(e, data.ErrConflict) || r.saves != 0 {
		t.Fatal("stale head saved", e)
	}
	if e = s.Save(context.Background(), o, checkpoint.Text("invalid-schema"), 3, 2); e == nil || r.saves != 0 {
		t.Fatal("unapproved schema saved")
	}
	if e = s.Save(context.Background(), o, got.Snapshot.State, 3, 2); e != nil || r.saves != 1 || r.saved.Recovery == nil || r.saved.HistoryHash == "" {
		t.Fatal(e)
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, e = s.Build(ctx, o); !errors.Is(e, context.Canceled) {
		t.Fatal("ignored cancellation", e)
	}
	r.readErr = data.ErrDenied
	if _, e = s.Build(context.Background(), o); !errors.Is(e, data.ErrDenied) {
		t.Fatal("history read failure hidden", e)
	}
}
