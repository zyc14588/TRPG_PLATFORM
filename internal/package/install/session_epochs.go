// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package install

import (
	"context"
	"errors"
	"reflect"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

type epochRepository interface {
	ReadSessionLocks(context.Context, *store.Graph, data.Binding) ([]data.SessionLock, error)
}

func (f *SessionFactory) authenticateEpochs(ctx context.Context, r SessionRequest, current RecoveryContext) (RecoveryContext, error) {
	repo, ok := f.options.Repository.(epochRepository)
	if !ok {
		return current, nil
	}
	locks, err := repo.ReadSessionLocks(ctx, current.Graph, current.Binding)
	if errors.Is(err, data.ErrNotFound) {
		return current, nil
	}
	if err != nil || len(locks) == 0 || len(locks) > 257 {
		return current, ErrPolicy
	}
	current.Epochs = map[string]RecoveryContext{current.Binding.GraphHash: current}
	current.OriginGraphHash = locks[0].GraphHash
	for _, lock := range locks {
		if data.ValidateSessionLock(lock) != nil {
			return current, ErrPolicy
		}
		if already, ok := current.Epochs[lock.GraphHash]; ok {
			if !reflect.DeepEqual(already.Lock, lock) {
				return current, ErrPolicy
			}
			continue
		}
		epoch := r
		epoch.Root = lock.RootIdentity
		epoch.Dependencies = nil
		for _, p := range lock.Packages {
			if p.Identity != lock.RootIdentity {
				epoch.Dependencies = append(epoch.Dependencies, p.Identity)
			}
		}
		epoch.Evidence = map[string]Evidence{}
		available := r.EpochEvidence[lock.GraphHash]
		if available == nil {
			available = r.Evidence
		}
		for _, p := range lock.Packages {
			if e, ok := available[p.Identity]; ok {
				epoch.Evidence[p.Identity] = e
			}
		}
		approved, e := f.prepare(ctx, epoch)
		if e != nil || !reflect.DeepEqual(approved.recovery.Lock, lock) {
			return current, ErrPolicy
		}
		current.Epochs[lock.GraphHash] = approved.recovery
	}
	if _, ok = current.Epochs[current.OriginGraphHash]; !ok {
		return current, ErrPolicy
	}
	return current, nil
}

func (o RecoveryContext) Origin() (RecoveryContext, error) {
	if len(o.Epochs) == 0 {
		return o, nil
	}
	v, ok := o.Epochs[o.OriginGraphHash]
	if !ok {
		return RecoveryContext{}, ErrPolicy
	}
	return v, nil
}
