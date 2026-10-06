// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package install

import (
	"context"
	"errors"

	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/vm"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

type SessionRepository interface {
	data.Repository
	ProvisionGraph(context.Context, *store.Graph, data.Binding, uint64, string, checkpoint.Value) error
	ReadGraphSession(context.Context, *store.Graph, data.Binding) (data.Snapshot, error)
}
type SessionOptions struct {
	Reader     *store.Reader
	Policy     *Policy
	Repository SessionRepository
	Runtime    RuntimeConfig
	Execution  func(Execution) error
	Audit      func(data.Audit) error
	Validate   func(context.Context, data.Commit) error
}
type SessionFactory struct{ options SessionOptions }
type SessionRequest struct {
	Credential               store.Credential
	Workspace, Session, Root string
	Dependencies             []string
	Evidence                 map[string]Evidence // exact artifact identity -> actual attestations
}
type InstalledSession struct {
	VM       *vm.Session
	Commands *hostapi.Service
	Graph    *store.Graph
	Binding  data.Binding
	observe  func(Execution) error
	runtime  RuntimeConfig
}

func (s *InstalledSession) Close() error {
	pid := s.VM.PID()
	err := s.VM.Destroy()
	if audit := s.observe(Execution{Package: string(s.Graph.Root().ExactLock().Root()), Case: "session-vm-destroy", Profile: profile.ID, Runtime: profile.RuntimeVersion, RunnerHash: s.runtime.SHA256, Outcome: runtimeCode(err), PID: pid, Reaped: true}); audit != nil {
		return audit
	}
	return err
}

func NewSessionFactory(o SessionOptions) (*SessionFactory, error) {
	if o.Reader == nil || o.Policy == nil || o.Repository == nil || o.Execution == nil || o.Audit == nil || o.Validate == nil {
		return nil, ErrPolicy
	}
	return &SessionFactory{options: o}, nil
}
func (f *SessionFactory) Create(ctx context.Context, r SessionRequest) (*InstalledSession, error) {
	return f.open(ctx, r, false)
}
func (f *SessionFactory) Resume(ctx context.Context, r SessionRequest) (*InstalledSession, error) {
	return f.open(ctx, r, true)
}
func (f *SessionFactory) open(ctx context.Context, r SessionRequest, resume bool) (*InstalledSession, error) {
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
		return nil, ErrPolicy
	}
	g, err := o.Reader.LoadGraph(ctx, r.Credential, r.Workspace, r.Root, r.Dependencies)
	if err != nil {
		return nil, err
	}
	items := []staged{}
	for _, pkg := range append([]*archive.Package{g.Root()}, g.Dependencies()...) {
		id := string(pkg.ArtifactIdentity().Digest())
		a, _, err := o.Policy.validate(pkg, r.Evidence[id])
		if err != nil {
			return nil, err
		}
		items = append(items, staged{pkg: pkg, evidence: r.Evidence[id], approval: a})
	}
	if len(r.Evidence) > len(items) {
		return nil, ErrPolicy
	}
	for id := range r.Evidence {
		found := false
		for _, item := range items {
			if id == string(item.pkg.ArtifactIdentity().Digest()) {
				found = true
			}
		}
		if !found {
			return nil, ErrPolicy
		}
	}
	for _, a := range g.Artifacts() {
		if a.PolicyDigest != o.Policy.Digest() {
			return nil, ErrPolicy
		}
	}
	contract := items[0].approval.Host
	if contract == nil {
		return nil, ErrPolicy
	}
	proofs := map[string]vm.FallbackProof{}
	// Startup re-authenticates current operator evidence and reruns the bounded
	// quarantine suite; fallback proofs therefore describe actual passed work.
	if _, err = validateRuntime(ctx, o.Runtime, items, o.Execution, runtimePolicy{Policy: o.Policy, Fallbacks: proofs}); err != nil {
		return nil, err
	}
	h, err := o.Policy.hostOptions(items)
	if err != nil {
		return nil, err
	}
	lock, _ := g.Root().ExactLock().Digest()
	binding := data.Binding{Workspace: r.Workspace, Session: r.Session, GraphHash: string(lock)}
	state := vm.State{Version: 1, Value: contract.State.Seed}
	if resume {
		snapshot, err := o.Repository.ReadGraphSession(ctx, g, binding)
		if err != nil {
			return nil, err
		}
		if snapshot.SchemaHash != contract.State.Digest {
			return nil, ErrPolicy
		}
		state = vm.State{Version: snapshot.Version, Value: snapshot.State}
	}
	packages := graphPackages(g.Root(), g.Dependencies())
	schema, err := hostapi.BindSchema(packages[contract.State.PackageID], contract.State.Path, contract.State.Digest, contract.State.Seed)
	if err != nil || schema.Validate(state.Value) != nil {
		return nil, ErrPolicy
	}
	if err = verifyRuntime(o.Runtime); err != nil {
		return nil, err
	}
	s, err := vm.New(ctx, vm.Options{SessionID: r.Session, Runner: o.Runtime.Runner, Package: g.Root(), Dependencies: g.Dependencies(), State: state, Limits: o.Runtime.Limits, Host: h, Fallbacks: proofs, Audit: func(a profile.Audit) error {
		return o.Audit(data.Audit{Workspace: r.Workspace, Session: r.Session, Level: a.Level, Operation: "vm/" + a.Kind, Outcome: a.Outcome})
	}})
	if err != nil {
		return nil, err
	}
	pid := s.PID()
	fail := func(e error) (*InstalledSession, error) {
		destroy := s.Destroy()
		audit := o.Execution(Execution{Package: string(g.Root().ExactLock().Root()), Case: "session-vm-failed-destroy", Profile: profile.ID, Runtime: profile.RuntimeVersion, RunnerHash: o.Runtime.SHA256, Outcome: runtimeCode(e), PID: pid, Reaped: true})
		return nil, errors.Join(e, destroy, audit)
	}
	if err = o.Execution(Execution{Package: string(g.Root().ExactLock().Root()), Case: "session-vm-ready", Profile: profile.ID, Runtime: profile.RuntimeVersion, RunnerHash: o.Runtime.SHA256, Outcome: runtimeCode(nil), PID: pid}); err != nil {
		return fail(err)
	}
	config, err := contract.bind(s, packages, o.Repository, r.Workspace)
	if err != nil {
		return fail(err)
	}
	config.Audit = o.Audit
	config.Validate = o.Validate
	service, err := hostapi.New(config)
	if err != nil {
		return fail(err)
	}
	if !resume {
		if err = o.Repository.ProvisionGraph(ctx, g, binding, state.Version, schema.Digest(), state.Value); err != nil {
			return fail(err)
		}
	}
	return &InstalledSession{VM: s, Commands: service, Graph: g, Binding: binding, observe: o.Execution, runtime: o.Runtime}, nil
}
