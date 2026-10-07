// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package task

import (
	"context"
	"crypto/sha256"
	"fmt"
	"io"
	"sort"
	"sync"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
)

type Credential struct{ data **credentialData }
type credentialData struct{ hash [32]byte }

func (Credential) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private task worker credential>")
}
func (Credential) MarshalJSON() ([]byte, error) { return nil, ErrDenied }
func (c Credential) state() *credentialData {
	if c.data == nil {
		return nil
	}
	return *c.data
}
func NewCredential(raw []byte) (Credential, error) {
	if len(raw) < 32 || len(raw) > 256 {
		return Credential{}, ErrInvalid
	}
	d := &credentialData{hash: sha256.Sum256(raw)}
	return Credential{data: &d}, nil
}

type WorkerGrant struct {
	ID         string
	Credential Credential
	Workspaces []string
	Expires    time.Time
}
type workerRecord struct {
	id         string
	workspaces map[string]bool
	expires    time.Time
	epoch      uint64
	disabled   bool
}
type Authority struct{ data **authorityData }
type authorityData struct {
	mu      sync.RWMutex
	records map[[32]byte]workerRecord
}
type Worker struct{ data **workerData }
type workerData struct {
	issuer *Authority
	key    [32]byte
	id     string
	epoch  uint64
}

func (Authority) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<task worker authority>") }
func (Authority) MarshalJSON() ([]byte, error) { return nil, ErrDenied }
func (Worker) Format(f fmt.State, _ rune)      { _, _ = io.WriteString(f, "<authenticated task worker>") }
func (Worker) MarshalJSON() ([]byte, error)    { return nil, ErrDenied }
func (a *Authority) state() *authorityData {
	if a == nil || a.data == nil {
		return nil
	}
	return *a.data
}
func (w Worker) state() *workerData {
	if w.data == nil {
		return nil
	}
	return *w.data
}
func NewAuthority(grants []WorkerGrant) (*Authority, error) {
	if len(grants) < 1 || len(grants) > 16 {
		return nil, ErrInvalid
	}
	d := &authorityData{records: map[[32]byte]workerRecord{}}
	a := &Authority{data: &d}
	ids := map[string]bool{}
	now := time.Now()
	for _, g := range grants {
		if !store.ValidID(g.ID) || ids[g.ID] || g.Credential.state() == nil || len(g.Workspaces) < 1 || len(g.Workspaces) > 32 || !g.Expires.After(now) || g.Expires.After(now.Add(24*time.Hour)) {
			return nil, ErrInvalid
		}
		key := g.Credential.state().hash
		if _, ok := d.records[key]; ok {
			return nil, ErrInvalid
		}
		spaces := map[string]bool{}
		for _, s := range g.Workspaces {
			if !store.ValidID(s) || spaces[s] {
				return nil, ErrInvalid
			}
			spaces[s] = true
		}
		ids[g.ID] = true
		d.records[key] = workerRecord{id: g.ID, workspaces: spaces, expires: g.Expires, epoch: 1}
	}
	return a, nil
}
func (a *Authority) Authenticate(ctx context.Context, c Credential) (Worker, error) {
	if a.state() == nil || ctx == nil || ctx.Err() != nil || c.state() == nil {
		return Worker{}, ErrDenied
	}
	a.state().mu.RLock()
	r, ok := a.state().records[c.state().hash]
	a.state().mu.RUnlock()
	if !ok || r.disabled || !r.expires.After(time.Now()) {
		return Worker{}, ErrDenied
	}
	d := &workerData{issuer: a, key: c.state().hash, id: r.id, epoch: r.epoch}
	return Worker{data: &d}, nil
}
func (w Worker) record(ctx context.Context) (workerRecord, error) {
	v := w.state()
	if v == nil || v.issuer.state() == nil || ctx == nil || ctx.Err() != nil {
		return workerRecord{}, ErrDenied
	}
	d := v.issuer.state()
	d.mu.RLock()
	r, ok := d.records[v.key]
	d.mu.RUnlock()
	if !ok || r.disabled || r.id != v.id || r.epoch != v.epoch || !r.expires.After(time.Now()) {
		return workerRecord{}, ErrDenied
	}
	return r, nil
}
func (w Worker) ID() string {
	if w.state() == nil {
		return ""
	}
	return w.state().id
}
func (w Worker) Check(ctx context.Context, workspace string) error {
	r, e := w.record(ctx)
	if e != nil || !r.workspaces[workspace] {
		return ErrDenied
	}
	return nil
}
func (w Worker) Workspaces(ctx context.Context) ([]string, error) {
	r, e := w.record(ctx)
	if e != nil {
		return nil, e
	}
	out := make([]string, 0, len(r.workspaces))
	for s := range r.workspaces {
		out = append(out, s)
	}
	sort.Strings(out)
	return out, nil
}
func (a *Authority) Revoke(id string) error {
	if a.state() == nil || !store.ValidID(id) {
		return ErrDenied
	}
	d := a.state()
	d.mu.Lock()
	defer d.mu.Unlock()
	for k, r := range d.records {
		if r.id == id {
			r.disabled = true
			r.epoch++
			d.records[k] = r
			return nil
		}
	}
	return ErrDenied
}
