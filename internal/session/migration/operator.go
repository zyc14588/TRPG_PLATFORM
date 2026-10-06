// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package migration

import (
	"context"
	"errors"
	"reflect"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/projection"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/recovery"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

type Lease interface {
	install.SessionRepository
	recovery.Repository
	ReadSessionLocks(context.Context, *store.Graph, data.Binding) ([]data.SessionLock, error)
	CapturePoint(context.Context, string, data.Snapshot, string, data.CheckpointCache) (data.RecoveryPoint, error)
	ReserveUpgrade(context.Context, data.RecoveryPoint, data.EffectRecord, data.EffectRecord) error
	SavePoint(context.Context, data.RecoveryPoint) error
	LoadPoint(context.Context, string) (data.RecoveryPoint, error)
	Stage(context.Context, data.EffectRecord, projection.Cache, data.CheckpointCache) error
	CommitMigration(context.Context, *store.Graph, data.EffectRecord, projection.Cache, data.CheckpointCache) (data.Receipt, error)
	Rollback() error
}
type Acquire func(context.Context, *store.Graph, *store.Graph, data.Binding, uint64) (Lease, error)
type Operator struct {
	factory *install.SessionFactory
	acquire Acquire
}
type Request struct {
	From, To           install.SessionRequest
	ExpectedVersion    uint64
	PointID, CommandID string
	Plan               Plan
}
type Result struct {
	Receipt                                                                         data.Receipt
	PointHash, OldStateHash, NewStateHash, OldHistoryHash, NewHistoryHash, LockHash string
}

func New(factory *install.SessionFactory, acquire Acquire) (*Operator, error) {
	if factory == nil || acquire == nil {
		return nil, data.ErrDenied
	}
	return &Operator{factory, acquire}, nil
}
func snapshot(i projection.Image) data.Snapshot {
	return data.Snapshot{Binding: i.Binding, Version: i.Version, SchemaHash: i.StateSchema, State: i.State, Rows: i.Rows, Quantities: i.Quantities}
}
func cache(o install.RecoveryContext, i projection.Image) (projection.Cache, data.CheckpointCache, error) {
	c, err := projection.Seal(o.Metadata, i)
	if err != nil {
		return c, data.CheckpointCache{}, err
	}
	m := c.Metadata
	cp := data.CheckpointCache{Binding: i.Binding, Version: i.Version, Cursor: i.Cursor, StateSchema: i.StateSchema, CheckpointSchema: m.CheckpointSchema, Value: eventstore.Copy(i.State), Hash: eventstore.Digest(i.State), Recovery: &m, HistoryHash: i.HistoryHash}
	if o.CheckpointSchema.Validate(cp.Value) != nil {
		return c, cp, checkpoint.ErrRejected
	}
	return c, cp, nil
}
func transition(before, after data.Snapshot, cursor uint64, from, to install.RecoveryContext, p data.RecoveryPoint, id, direction string, ended bool) (data.EffectRecord, error) {
	m := data.MigrationTransition{PointID: p.ID, PointHash: p.Hash, Direction: direction, From: from.Lock, To: to.Lock, Before: eventstore.Copy(from.Metadata), After: eventstore.Copy(to.Metadata), WasEnded: ended}
	m.Before.Session.StateVersion = before.Version
	m.After.Session.StateVersion = after.Version
	h := data.Header{Binding: before.Binding, Principal: from.Graph.Membership().Principal, CommandID: id, ExpectedVersion: before.Version, Fingerprint: eventstore.Digest(struct {
		Transition data.MigrationTransition
		State      data.Snapshot
	}{m, after})}
	return eventstore.RecordMigration(before, after, h, cursor, m)
}
func read(ctx context.Context, s *install.InstalledSession, callback string, input checkpoint.Value) (checkpoint.Value, error) {
	r, err := s.Commands.Read(ctx, s.VM.Token(), hostapi.Command{ID: "migration-" + callback, Principal: "platform", ExpectedVersion: s.VM.StateVersion(), Callback: callback, Input: input})
	return r.Result, err
}
func (o *Operator) Run(ctx context.Context, r Request) (result Result, err error) {
	return o.run(ctx, r, false)
}
func (o *Operator) RestorePoint(ctx context.Context, r Request) (result Result, err error) {
	return o.run(ctx, r, true)
}
func (o *Operator) run(ctx context.Context, r Request, restore bool) (result Result, err error) {
	ctx, cancel := context.WithTimeout(ctx, 20*time.Second)
	defer cancel()
	if !store.ValidID(r.PointID) || !store.ValidID(r.CommandID) || r.From.Workspace != r.To.Workspace || r.From.Session != r.To.Session {
		return result, data.ErrDenied
	}
	source, err := o.factory.Authenticate(ctx, r.From)
	if err != nil {
		return result, err
	}
	target, err := o.factory.Authenticate(ctx, r.To)
	if err != nil {
		return result, err
	}
	lease, err := o.acquire(ctx, source.Graph, target.Graph, source.Binding, r.ExpectedVersion)
	if err != nil {
		return result, err
	}
	defer lease.Rollback()
	f, err := o.factory.WithRepository(lease)
	if err != nil {
		return result, err
	}
	rebuild, err := recovery.New(lease)
	if err != nil {
		return result, err
	}
	old, err := f.Recover(ctx, r.From, rebuild.Build)
	if err != nil {
		return result, err
	}
	source = old.Recovery
	report, err := rebuild.Reconstruct(ctx, source)
	if err != nil {
		return result, errors.Join(err, old.Close())
	}
	h, err := lease.ReadReplayHistory(ctx, source.Graph, source.Binding)
	if err != nil {
		return result, errors.Join(err, old.Close())
	}
	if !restore && len(h.Records) > 0 && !h.Ended {
		if !source.SafeBoundaryDeclared {
			return result, errors.Join(data.ErrDenied, old.Close())
		}
		safe, e := read(ctx, old, "on_safe_migration_boundary", checkpoint.Object(map[string]checkpoint.Value{}))
		if e != nil || safe.Kind != "boolean" || !safe.Boolean {
			return result, errors.Join(data.ErrDenied, e, old.Close())
		}
	}
	value, e := read(ctx, old, "create_checkpoint", checkpoint.Object(map[string]checkpoint.Value{}))
	closeErr := old.Close()
	if e != nil || closeErr != nil {
		return result, errors.Join(e, closeErr)
	}
	before := snapshot(report.Image)
	if source.CheckpointSchema.Validate(value) != nil || eventstore.Digest(value) != eventstore.Digest(before.State) {
		return result, checkpoint.ErrRejected
	}
	_, sourceCP, err := cache(source, report.Image)
	if err != nil {
		return result, err
	}
	var point data.RecoveryPoint
	var after data.Snapshot
	direction := "upgrade"
	if restore {
		direction = "restore-point"
		point, err = lease.LoadPoint(ctx, r.PointID)
		if err != nil {
			return result, err
		}
		after = eventstore.Copy(point.Verified)
		after.Version = before.Version + 1
		after.Binding = target.Binding
		if target.ValidateSnapshot(after) != nil {
			return result, eventstore.ErrHistory
		}
	} else {
		point, err = lease.CapturePoint(ctx, r.PointID, before, report.Image.HistoryHash, sourceCP)
		if err != nil {
			return result, err
		}
		after, err = r.Plan.Apply(before, target)
		if err != nil {
			return result, err
		}
	}
	record, err := transition(before, after, report.Image.Cursor, source, target, point, r.CommandID, direction, h.Ended)
	if err != nil {
		return result, err
	}
	contexts := eventstoreContexts(source, target)
	if contexts.ValidateRecord(record) != nil {
		return result, eventstore.ErrHistory
	}
	image, err := projection.Apply(report.Image, record)
	if err != nil {
		return result, err
	}
	if !reflect.DeepEqual(snapshot(image), after) {
		return result, eventstore.ErrHistory
	}
	if !restore {
		reverted := eventstore.Copy(before)
		reverted.Version = after.Version + 1
		inverse, e := transition(after, reverted, image.Cursor, target, source, point, r.CommandID+"-restore", "restore-point", h.Ended)
		if e != nil {
			return result, e
		}
		if contexts.ValidateRecord(inverse) != nil {
			return result, eventstore.ErrHistory
		}
		if err = lease.ReserveUpgrade(ctx, point, record, inverse); err != nil {
			return result, err
		}
		if err = lease.SavePoint(ctx, point); err != nil {
			return result, err
		}
	}
	c, cp, err := cache(target, image)
	if err != nil {
		return result, err
	}
	if err = lease.Stage(ctx, record, c, cp); err != nil {
		return result, err
	}
	current, err := f.Recover(ctx, r.To, rebuild.Build)
	if err != nil {
		return result, err
	}
	rehearsed, err := rebuild.Reconstruct(ctx, current.Recovery)
	if err == nil && (!reflect.DeepEqual(rehearsed.Image, image) || !rehearsed.CheckpointAccepted) {
		err = eventstore.ErrHistory
	}
	if err == nil {
		actual, e := read(ctx, current, "create_checkpoint", checkpoint.Object(map[string]checkpoint.Value{}))
		if e != nil {
			err = e
		} else if eventstore.Digest(actual) != cp.Hash {
			err = checkpoint.ErrRejected
		}
	}
	closeErr = current.Close()
	if err != nil || closeErr != nil {
		return result, errors.Join(err, closeErr)
	}
	receipt, err := lease.CommitMigration(ctx, target.Graph, record, c, cp)
	if err != nil {
		return result, err
	}
	return Result{Receipt: receipt, PointHash: point.Hash, OldStateHash: eventstore.Digest(before.State), NewStateHash: eventstore.Digest(after.State), OldHistoryHash: report.Image.HistoryHash, NewHistoryHash: image.HistoryHash, LockHash: target.Lock.Hash}, nil
}
func eventstoreContexts(source, target install.RecoveryContext) install.RecoveryContext {
	contexts := source
	contexts.Epochs = map[string]install.RecoveryContext{}
	for hash, c := range source.Epochs {
		contexts.Epochs[hash] = c
	}
	contexts.Epochs[source.Binding.GraphHash] = source
	contexts.Epochs[target.Binding.GraphHash] = target
	return contexts
}
