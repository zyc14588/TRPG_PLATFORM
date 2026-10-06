// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package install

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/vm"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

type preparedSession struct {
	request  SessionRequest
	graph    *store.Graph
	contract *HostContract
	host     *vm.HostOptions
	proofs   map[string]vm.FallbackProof
	recovery RecoveryContext
	binding  data.Binding
}

func (f *SessionFactory) prepare(ctx context.Context, r SessionRequest) (preparedSession, error) {
	r.Dependencies = append([]string(nil), r.Dependencies...)
	owned := map[string]Evidence{}
	for id, e := range r.Evidence {
		copyAttestation := func(a *Attestation) *Attestation {
			if a == nil {
				return nil
			}
			copy := *a
			copy.Signature = append([]byte(nil), a.Signature...)
			return &copy
		}
		owned[id] = Evidence{Publisher: copyAttestation(e.Publisher), Certification: copyAttestation(e.Certification)}
	}
	r.Evidence = owned
	o := f.options
	if !store.ValidID(r.Session) {
		return preparedSession{}, ErrPolicy
	}
	g, err := o.Reader.LoadGraph(ctx, r.Credential, r.Workspace, r.Root, r.Dependencies)
	if err != nil {
		return preparedSession{}, err
	}
	items := []staged{}
	for _, pkg := range append([]*archive.Package{g.Root()}, g.Dependencies()...) {
		id := string(pkg.ArtifactIdentity().Digest())
		a, _, err := o.Policy.validate(pkg, r.Evidence[id])
		if err != nil {
			return preparedSession{}, err
		}
		items = append(items, staged{pkg: pkg, evidence: r.Evidence[id], approval: a})
	}
	if len(r.Evidence) > len(items) {
		return preparedSession{}, ErrPolicy
	}
	for id := range r.Evidence {
		found := false
		for _, item := range items {
			if id == string(item.pkg.ArtifactIdentity().Digest()) {
				found = true
			}
		}
		if !found {
			return preparedSession{}, ErrPolicy
		}
	}
	for _, a := range g.Artifacts() {
		if a.PolicyDigest != o.Policy.Digest() {
			return preparedSession{}, ErrPolicy
		}
	}
	contract := items[0].approval.Host
	if contract == nil {
		return preparedSession{}, ErrPolicy
	}
	proofs := map[string]vm.FallbackProof{}
	// Startup re-authenticates current operator evidence and reruns the bounded
	// quarantine suite; fallback proofs therefore describe actual passed work.
	if _, err = validateRuntime(ctx, o.Runtime, items, o.Execution, runtimePolicy{Policy: o.Policy, Fallbacks: proofs}); err != nil {
		return preparedSession{}, err
	}
	h, err := o.Policy.hostOptions(items)
	if err != nil {
		return preparedSession{}, err
	}
	lock, _ := g.Root().ExactLock().Digest()
	binding := data.Binding{Workspace: r.Workspace, Session: r.Session, GraphHash: string(lock)}
	recovery, err := recoveryContext(g, binding, contract, o.Runtime)
	if err != nil {
		return preparedSession{}, err
	}
	return preparedSession{request: r, graph: g, contract: contract, host: h, proofs: proofs, recovery: recovery, binding: binding}, nil
}

// Authenticate applies the same current ACL, object, policy, quarantine and
// bound Schema checks as VM startup. It does not provision or mutate a Session.
func (f *SessionFactory) Authenticate(ctx context.Context, r SessionRequest) (RecoveryContext, error) {
	p, err := f.prepare(ctx, r)
	return p.recovery, err
}

// WithRepository retains the complete approved factory configuration while a
// trusted migration lease supplies its private SQL-backed read-only rehearsal.
func (f *SessionFactory) WithRepository(r SessionRepository) (*SessionFactory, error) {
	o := f.options
	o.Repository = r
	return NewSessionFactory(o)
}
