// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package recovery composes immutable history reduction with authenticated
// installed graphs. Reconstruction has no external dispatcher or model client.
package recovery

import (
	"context"
	"errors"
	"fmt"
	"reflect"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/projection"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

type Repository interface {
	ReadReplayHistory(context.Context, *store.Graph, data.Binding) (data.ReplayHistory, error)
	ReadProjectionCache(context.Context, data.Binding) (projection.Cache, error)
	ReadCheckpoint(context.Context, data.Binding) (data.CheckpointCache, error)
	RepairDerived(context.Context, *store.Graph, projection.Cache) error
	SaveCheckpoint(context.Context, data.CheckpointCache) error
}
type Service struct{ repository Repository }
type Report struct {
	Image              projection.Image
	SnapshotAccepted   bool
	CheckpointAccepted bool
	Checkpoint         *data.CheckpointCache
	RecordedTime       int64
	RecordedRandom     []int64
}

func New(r Repository) (*Service, error) {
	if r == nil {
		return nil, eventstore.ErrHistory
	}
	return &Service{r}, nil
}
func (s *Service) Reconstruct(ctx context.Context, o install.RecoveryContext) (Report, error) {
	ctx, cancel := context.WithTimeout(ctx, 3*time.Second)
	defer cancel()
	h, err := s.repository.ReadReplayHistory(ctx, o.Graph, o.Binding)
	if err != nil {
		return Report{}, fmt.Errorf("%w: immutable-history", err)
	}
	if h.Creation.SeedHash != eventstore.Digest(o.Seed) {
		return Report{}, fmt.Errorf("%w: bound-seed", eventstore.ErrHistory)
	}
	var candidate *projection.Cache
	cache, err := s.repository.ReadProjectionCache(ctx, o.Binding)
	if err == nil {
		candidate = &cache
	} else if !errors.Is(err, data.ErrNotFound) && !errors.Is(err, checkpoint.ErrRejected) {
		return Report{}, err
	}
	i, accepted, err := projection.Rebuild(h, o.Metadata, candidate, o.ValidateRecord)
	if err != nil {
		return Report{}, fmt.Errorf("%w: reduction", err)
	}
	if o.StateSchema.Validate(i.State) != nil {
		return Report{}, fmt.Errorf("%w: state-schema", eventstore.ErrHistory)
	}
	result := Report{Image: i, SnapshotAccepted: accepted}
	if len(h.Records) > 0 {
		last := h.Records[len(h.Records)-1]
		result.RecordedTime = last.Inputs.Time
		result.RecordedRandom = append([]int64(nil), last.Inputs.Random...)
	}
	c, err := s.repository.ReadCheckpoint(ctx, o.Binding)
	if err == nil {
		m := eventstore.Copy(o.Metadata)
		m.Session.StateVersion = i.Version
		// M1 supports reconstructible data checkpoints. Arbitrary opaque cache
		// facts have no independent authority, even with a valid self-hash.
		if c.Recovery != nil && c.Recovery.Validate() == nil && reflect.DeepEqual(*c.Recovery, m) && c.Binding == i.Binding && c.Version == i.Version && c.Cursor == i.Cursor && c.StateSchema == m.StateSchema && c.CheckpointSchema == m.CheckpointSchema && c.HistoryHash == i.HistoryHash && c.Hash == eventstore.Digest(c.Value) && c.Hash == eventstore.Digest(i.State) && o.CheckpointSchema.Validate(c.Value) == nil {
			result.CheckpointAccepted = true
			result.Checkpoint = &c
		}
	} else if !errors.Is(err, data.ErrNotFound) && !errors.Is(err, data.ErrDenied) && !errors.Is(err, checkpoint.ErrRejected) {
		return Report{}, err
	}
	return result, nil
}
func (s *Service) Build(ctx context.Context, o install.RecoveryContext) (install.RecoveryResult, error) {
	ctx, cancel := context.WithTimeout(ctx, 3*time.Second)
	defer cancel()
	r, err := s.Reconstruct(ctx, o)
	if err != nil {
		return install.RecoveryResult{}, err
	}
	c, err := projection.Seal(o.Metadata, r.Image)
	if err != nil {
		return install.RecoveryResult{}, err
	}
	if err = s.repository.RepairDerived(ctx, o.Graph, c); err != nil {
		return install.RecoveryResult{}, err
	}
	i := r.Image
	return install.RecoveryResult{Snapshot: data.Snapshot{Binding: i.Binding, Version: i.Version, SchemaHash: i.StateSchema, State: i.State, Rows: i.Rows, Quantities: i.Quantities}, Checkpoint: r.Checkpoint, Time: r.RecordedTime, Random: r.RecordedRandom}, nil
}
func (s *Service) Save(ctx context.Context, o install.RecoveryContext, value checkpoint.Value, version, cursor uint64) error {
	ctx, cancel := context.WithTimeout(ctx, 3*time.Second)
	defer cancel()
	r, err := s.Reconstruct(ctx, o)
	if err != nil {
		return err
	}
	if o.CheckpointSchema.Validate(value) != nil {
		return checkpoint.ErrRejected
	}
	if r.Image.Version != version || r.Image.Cursor != cursor {
		return data.ErrConflict
	}
	m := eventstore.Copy(o.Metadata)
	m.Session.StateVersion = r.Image.Version
	return s.repository.SaveCheckpoint(ctx, data.CheckpointCache{Binding: o.Binding, Version: r.Image.Version, Cursor: r.Image.Cursor, StateSchema: m.StateSchema, CheckpointSchema: m.CheckpointSchema, Value: eventstore.Copy(value), Hash: eventstore.Digest(value), Recovery: &m, HistoryHash: r.Image.HistoryHash})
}
