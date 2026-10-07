// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package actor

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
)

// RecoveryPoint is metadata only. Authoritative history remains untouched;
// the checkpoint is derived and is never returned as a game-state export.
type RecoveryPoint struct{ Version, Cursor uint64 }

func (r *Registry) CreateRecoveryPoint(ctx context.Context, i command.Identity) (RecoveryPoint, error) {
	if e := r.options.Authority.CheckRecoveryPoint(ctx, i); e != nil {
		return RecoveryPoint{}, e
	}
	v, e := r.dispatch(request{ctx: ctx, identity: i, kind: "recovery-point"})
	return v.point, e
}
