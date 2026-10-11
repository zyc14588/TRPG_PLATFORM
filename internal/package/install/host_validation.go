// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package install

import (
	"context"
	"encoding/json"
	"errors"
	"sync"

	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/vm"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

func validateHostRuntime(ctx context.Context, c RuntimeConfig, items []staged, proofs map[string]vm.FallbackProof, p *Policy, emit func(Execution) error) (resultErr error) {
	root := items[0]
	d, _ := root.pkg.Manifest()
	id := string(d.Package.PackageID)
	contract := root.approval.Host
	h, err := p.hostOptions(items)
	if err != nil {
		return err
	}
	_, resolution, err := p.validate(root.pkg, root.evidence)
	if err != nil {
		return err
	}
	needed := map[string]bool{}
	for _, name := range profile.StandardCallbackNames() {
		if name == "validate_command" || name == "execute_command" {
			name = "command"
		}
		needed[name] = true
	}
	if !resolution.Effective.Contains(capability.HostTask) && !resolution.Effective.Contains(capability.HostAI) {
		delete(needed, "resume_continuation")
	}
	for _, test := range root.approval.Tests {
		if test.Host != nil {
			if !validCaseName(test.Name) {
				return ErrPolicy
			}
			delete(needed, test.Host.Callback)
		}
	}
	if len(needed) != 0 {
		return ErrPolicy
	}
	deps := make([]*archive.Package, 0, len(items)-1)
	for _, item := range items[1:] {
		deps = append(deps, item.pkg)
	}
	s, err := vm.New(ctx, vm.Options{SessionID: "host-quarantine", Runner: c.Runner, Launcher: c.Launcher, Package: root.pkg, Dependencies: deps, State: vm.State{Version: 1, Value: contract.State.Seed}, Limits: c.Limits, Fallbacks: proofs, Host: h, Audit: func(profile.Audit) error { return nil }})
	if err != nil {
		return err
	}
	pid := s.PID()
	defer func() {
		destroyErr := s.Destroy()
		resultErr = errors.Join(resultErr, destroyErr)
		if e := emit(Execution{Package: id, Case: "host-quarantine-destroy", Profile: profile.ID, Runtime: profile.RuntimeVersion, RunnerHash: c.SHA256, Outcome: runtimeCode(resultErr), PID: pid, Reaped: s.Reaped()}); e != nil {
			resultErr = errors.Join(resultErr, e)
		}
	}()
	binding := data.Binding{Workspace: "install-quarantine", Session: s.SessionID(), GraphHash: s.GraphHash()}
	repo := &quarantineRepository{snapshot: data.Snapshot{Binding: binding, Version: 1, State: contract.State.Seed, SchemaHash: contract.State.Digest}, budget: contract.Budget}
	o, err := contract.bind(s, graphPackages(root.pkg, deps), repo, binding.Workspace)
	if err != nil {
		return err
	}
	service, err := hostapi.New(o)
	if err != nil {
		return err
	}
	for _, test := range root.approval.Tests {
		if test.Host == nil {
			continue
		}
		t := test.Host
		receipt, e := service.Execute(ctx, s.Token(), hostapi.Command{Callback: t.Callback, ID: test.Name, Principal: "certification", ExpectedVersion: s.StateVersion(), Input: t.Input, Time: t.Time, Random: t.Random})
		if e == nil && !equalHostValue(receipt.Result, t.Expected) {
			e = profile.Fail(profile.ErrScript)
		}
		if auditErr := emit(Execution{Package: id, Case: "host:" + test.Name, Profile: profile.ID, Runtime: profile.RuntimeVersion, RunnerHash: c.SHA256, Outcome: runtimeCode(e), PID: pid}); auditErr != nil {
			e = auditErr
		}
		if e != nil {
			return e
		}
	}
	return nil
}
func equalHostValue(a, b checkpoint.Value) bool {
	x, _ := json.Marshal(a)
	y, _ := json.Marshal(b)
	return string(x) == string(y)
}
func quarantineClone[T any](v T) T {
	raw, _ := json.Marshal(v)
	var copy T
	_ = json.Unmarshal(raw, &copy)
	return copy
}

// A certificate gets a private, bounded data-only workspace. There is no DB,
// runtime publication, worker dispatch or external AI side effect in this type.
type quarantineRepository struct {
	mu       sync.Mutex
	snapshot data.Snapshot
	budget   hostapi.Budget
}
type quarantineTransaction struct {
	repo   *quarantineRepository
	header data.Header
	closed bool
}

func (r *quarantineRepository) Begin(ctx context.Context, h data.Header) (data.Transaction, error) {
	if ctx.Err() != nil {
		return nil, ctx.Err()
	}
	r.mu.Lock()
	if h.Binding != r.snapshot.Binding || h.ExpectedVersion != r.snapshot.Version {
		r.mu.Unlock()
		return nil, data.ErrConflict
	}
	return &quarantineTransaction{repo: r, header: h}, nil
}
func (t *quarantineTransaction) Snapshot() data.Snapshot { return quarantineClone(t.repo.snapshot) }
func (t *quarantineTransaction) Rollback() error {
	if !t.closed {
		t.closed = true
		t.repo.mu.Unlock()
	}
	return nil
}
func (t *quarantineTransaction) Commit(ctx context.Context, c data.Commit) (data.Receipt, error) {
	if t.closed || ctx.Err() != nil || c.Header != t.header {
		return data.Receipt{}, data.ErrConflict
	}
	r := t.repo
	s := quarantineClone(r.snapshot)
	rows := map[string]data.Row{}
	quantities := map[string]data.Quantity{}
	for _, row := range s.Rows {
		rows[row.PackageID+"/"+row.Namespace+"/"+row.Key] = row
	}
	for _, q := range s.Quantities {
		quantities[q.PackageID+"/"+q.Table+"/"+q.Key] = q
	}
	for _, row := range c.Rows {
		key := row.PackageID + "/" + row.Namespace + "/" + row.Key
		if row.Deleted {
			delete(rows, key)
		} else {
			rows[key] = row
		}
	}
	for _, q := range c.Quantities {
		quantities[q.PackageID+"/"+q.Table+"/"+q.Key] = q
	}
	if len(rows)+len(quantities) > r.budget.Rows {
		return data.Receipt{}, profile.Fail(profile.ErrBudget)
	}
	s.Rows = nil
	s.Quantities = nil
	size := 0
	for _, row := range rows {
		raw, _ := json.Marshal(row)
		size += len(raw)
		s.Rows = append(s.Rows, row)
	}
	for _, q := range quantities {
		raw, _ := json.Marshal(q)
		size += len(raw)
		s.Quantities = append(s.Quantities, q)
	}
	if size > r.budget.DataBytes {
		return data.Receipt{}, profile.Fail(profile.ErrBudget)
	}
	s.State = quarantineClone(c.State)
	s.Version++
	s.SchemaHash = c.SchemaHash
	r.snapshot = s
	receipt := quarantineClone(data.Receipt{Header: c.Header, Version: s.Version, Result: c.Result, Events: c.Events, Inputs: c.Inputs})
	t.closed = true
	r.mu.Unlock()
	return receipt, nil
}
