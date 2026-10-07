// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package postgres

import (
	"context"
	"errors"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/projection"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"io"
)

type platformLaunchRuntime struct{ data **platformLaunchRuntimeData }
type platformLaunchRuntimeData struct{ host *HostRepository }

func (r *platformLaunchRuntime) state() *platformLaunchRuntimeData {
	if r == nil || r.data == nil {
		return nil
	}
	return *r.data
}
func (*platformLaunchRuntime) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<launch runtime repository>")
}
func (*platformLaunchRuntime) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func launchRuntimeError(e error) error {
	if e == nil {
		return nil
	}
	for _, known := range []error{data.ErrNotFound, data.ErrDenied, data.ErrConflict, data.ErrUnknownCommit, checkpoint.ErrRejected} {
		if errors.Is(e, known) {
			return known
		}
	}
	return auth.ErrUnavailable
}
func (r *platformLaunchRuntime) ReadReplayHistory(c context.Context, g *store.Graph, b data.Binding) (data.ReplayHistory, error) {
	v, e := r.state().host.ReadReplayHistory(c, g, b)
	return v, launchRuntimeError(e)
}
func (r *platformLaunchRuntime) ReadProjectionCache(c context.Context, b data.Binding) (projection.Cache, error) {
	v, e := r.state().host.ReadProjectionCache(c, b)
	return v, launchRuntimeError(e)
}
func (r *platformLaunchRuntime) ReadCheckpoint(c context.Context, b data.Binding) (data.CheckpointCache, error) {
	v, e := r.state().host.ReadCheckpoint(c, b)
	return v, launchRuntimeError(e)
}
func (r *platformLaunchRuntime) RepairDerived(c context.Context, g *store.Graph, v projection.Cache) error {
	return launchRuntimeError(r.state().host.RepairDerived(c, g, v))
}
func (r *platformLaunchRuntime) SaveCheckpoint(c context.Context, v data.CheckpointCache) error {
	return launchRuntimeError(r.state().host.SaveCheckpoint(c, v))
}
func (r *platformLaunchRuntime) ReadJournal(c context.Context, b data.Binding, n uint64, l int) (data.JournalPage, error) {
	v, e := r.state().host.ReadJournal(c, b, n, l)
	return v, launchRuntimeError(e)
}
func (r *platformLaunchRuntime) ReadCreation(c context.Context, b data.Binding) (data.Creation, error) {
	v, e := r.state().host.ReadCreation(c, b)
	return v, launchRuntimeError(e)
}
func (r *platformLaunchRuntime) LookupCommand(c context.Context, b data.Binding, p, id string) (data.Receipt, error) {
	v, e := r.state().host.LookupCommand(c, b, p, id)
	return v, launchRuntimeError(e)
}
