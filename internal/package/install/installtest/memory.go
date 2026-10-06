// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package installtest

import (
	"context"
	"sync"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
)

// MemoryInstallation is only a unit-test persistence double. Live PostgreSQL
// tests exercise the production transaction, ACL and serialization boundaries.
type MemoryInstallation struct {
	mu           sync.Mutex
	Publications []store.Publication
	artifacts    map[string]store.Artifact
	results      map[string]store.Result
	owners       map[string]string
}

func NewMemoryInstallation() *MemoryInstallation {
	return &MemoryInstallation{artifacts: map[string]store.Artifact{}, results: map[string]store.Result{}, owners: map[string]string{}}
}
func (m *MemoryInstallation) Preflight(context.Context, string, []string) error { return nil }
func (m *MemoryInstallation) Publish(ctx context.Context, p store.Publication) (store.Result, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	if ctx.Err() != nil {
		return store.Result{}, ctx.Err()
	}
	r := store.Result{Workspace: p.Workspace, RequestID: p.RequestID, Fingerprint: p.Fingerprint, Root: p.Root}
	for _, a := range p.Artifacts {
		m.artifacts[p.Workspace+"/"+a.Identity] = a
		m.owners[p.Workspace+"/"+a.Identity] = p.Principal
		r.Artifacts = append(r.Artifacts, a.Identity)
	}
	m.results[p.Workspace+"/"+p.RequestID] = r
	m.Publications = append(m.Publications, p)
	return r, nil
}
func (m *MemoryInstallation) Resolve(_ context.Context, workspace, principal, id, fingerprint string) (store.Result, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	r, ok := m.results[workspace+"/"+id]
	if !ok {
		return store.Result{}, store.ErrNotFound
	}
	if r.Fingerprint != fingerprint {
		return store.Result{}, store.ErrConflict
	}
	return r, nil
}
func (m *MemoryInstallation) Lookup(_ context.Context, workspace, principal, id string) (store.Artifact, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	key := workspace + "/" + id
	a, ok := m.artifacts[key]
	if !ok || m.owners[key] != principal {
		return store.Artifact{}, store.ErrNotFound
	}
	a.Objects = append([]store.ObjectRef(nil), a.Objects...)
	return a, nil
}
