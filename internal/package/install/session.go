// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package install

import (
	"context"
	"errors"

	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/vm"
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
	// Historical evidence is partitioned by recorded exact graph hash. Each
	// epoch still passes the original exact-artifact evidence validation.
	EpochEvidence map[string]map[string]Evidence
}
type InstalledSession struct {
	VM               *vm.Session
	Commands         *hostapi.Service
	Graph            *store.Graph
	Binding          data.Binding
	observe          func(Execution) error
	runtime          RuntimeConfig
	RecoveryMetadata checkpoint.RecoveryBinding
	Recovery         RecoveryContext
}

func (s *InstalledSession) Close() error {
	pid := s.VM.PID()
	err := s.VM.Destroy()
	if audit := s.observe(Execution{Package: string(s.Graph.Root().ExactLock().Root()), Case: "session-vm-destroy", Profile: profile.ID, Runtime: profile.RuntimeVersion, RunnerHash: s.runtime.SHA256, Outcome: runtimeCode(err), PID: pid, Reaped: s.VM.Reaped()}); audit != nil {
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
	return f.open(ctx, r, false, nil)
}
func (f *SessionFactory) Resume(ctx context.Context, r SessionRequest) (*InstalledSession, error) {
	return f.open(ctx, r, true, nil)
}
func (f *SessionFactory) open(ctx context.Context, r SessionRequest, resume bool, build RecoveryBuilder) (*InstalledSession, error) {
	p, err := f.prepare(ctx, r)
	if err != nil {
		return nil, err
	}
	o := f.options
	r = p.request
	g, contract, h, proofs, binding, recovery := p.graph, p.contract, p.host, p.proofs, p.binding, p.recovery
	if resume && build != nil {
		recovery, err = f.authenticateEpochs(ctx, r, recovery)
		if err != nil {
			return nil, err
		}
	}
	var recovered RecoveryResult
	state := vm.State{Version: 1, Value: contract.State.Seed}
	if resume {
		var snapshot data.Snapshot
		if build != nil {
			recovered, err = build(ctx, recovery)
			snapshot = recovered.Snapshot
		} else {
			snapshot, err = o.Repository.ReadGraphSession(ctx, g, binding)
		}
		if err != nil {
			return nil, err
		}
		if snapshot.Binding != binding || snapshot.Version == 0 || snapshot.SchemaHash != contract.State.Digest {
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
	s, err := vm.New(ctx, vm.Options{SessionID: r.Session, Runner: o.Runtime.Runner, Launcher: o.Runtime.Launcher, Package: g.Root(), Dependencies: g.Dependencies(), State: state, Limits: o.Runtime.Limits, Host: h, Fallbacks: proofs, Audit: func(a profile.Audit) error {
		return o.Audit(data.Audit{Workspace: r.Workspace, Session: r.Session, Level: a.Level, Operation: "vm/" + a.Kind, Outcome: a.Outcome})
	}})
	if err != nil {
		return nil, err
	}
	pid := s.PID()
	fail := func(e error) (*InstalledSession, error) {
		destroy := s.Destroy()
		audit := o.Execution(Execution{Package: string(g.Root().ExactLock().Root()), Case: "session-vm-failed-destroy", Profile: profile.ID, Runtime: profile.RuntimeVersion, RunnerHash: o.Runtime.SHA256, Outcome: runtimeCode(e), PID: pid, Reaped: s.Reaped()})
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
	if build != nil {
		read := func(callback string, input checkpoint.Value) error {
			_, err := service.Read(ctx, s.Token(), hostapi.Command{Callback: callback, ID: "recovery-" + callback, Principal: "platform", ExpectedVersion: state.Version, Input: input, Time: recovered.Time, Random: append([]int64(nil), recovered.Random...)})
			return err
		}
		if recovered.Checkpoint != nil {
			if err = read("restore_checkpoint", recovered.Checkpoint.Value); err != nil {
				return fail(err)
			}
		}
		if err = read("on_session_restore", state.Value); err != nil {
			return fail(err)
		}
	}
	recovery.Metadata.Session.StateVersion = state.Version
	return &InstalledSession{VM: s, Commands: service, Graph: g, Binding: binding, observe: o.Execution, runtime: o.Runtime, RecoveryMetadata: recovery.Metadata, Recovery: recovery}, nil
}
