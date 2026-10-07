// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package launch

import (
	"context"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

// SubmitContinuation shares the existing coordinator and installed runtime.
// The supplied current resolver revalidates the durable worker lease at every
// authority check; the worker receives no game writer, browser seat or hub.
func (s *Service) SubmitContinuation(ctx context.Context, j task.Job, current command.NativeResolver) (data.Receipt, error) {
	v, e := j.StorageValue()
	if e != nil || current == nil || s.state() == nil {
		return data.Receipt{}, task.ErrDenied
	}
	if e = s.liveService(ctx, v.Scope.WorkspaceID, v.Scope.RoomID); e != nil {
		return data.Receipt{}, task.ErrUnavailable
	}
	cfg, e := s.config(v.Binding.Workspace, v.ConfigurationID)
	if e != nil || configurationHash(cfg) != v.ConfigurationHash {
		return data.Receipt{}, task.ErrDenied
	}
	hash, e := sessionRequest(cfg, v.Scope).authenticate(ctx)
	if e != nil || hash != v.Binding.GraphHash {
		return data.Receipt{}, task.ErrDenied
	}
	envelope, e := j.Envelope()
	if e != nil {
		return data.Receipt{}, task.ErrDenied
	}
	if e = s.state().actors.Activate(ctx, nil, v.Binding, auth.RoomSecret(cfg), "task-system"); e != nil {
		return data.Receipt{}, task.ErrUnavailable
	}
	d := s.state().actors.state()
	d.mu.Lock()
	entry := d.entries[bindingKey(v.Binding)]
	if d.closed || entry == nil || !entry.active || entry.binding != v.Binding || entry.authority == nil || entry.registry == nil {
		d.mu.Unlock()
		return data.Receipt{}, task.ErrUnavailable
	}
	a, registry := entry.authority, entry.registry
	d.mu.Unlock()
	i, e := a.IssueContinuation(ctx, current, envelope)
	if e != nil {
		return data.Receipt{}, task.ErrDenied
	}
	defer a.ReleaseNative(i)
	if i.Binding() != v.Binding || i.Principal() != "task-system" || i.Seat() != "task-system" {
		return data.Receipt{}, task.ErrDenied
	}
	r, e := registry.Submit(ctx, i, envelope)
	return r, task.SafeError(e)
}
