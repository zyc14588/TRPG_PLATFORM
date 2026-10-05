// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package install

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/ipc"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/vm"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
)

type RuntimeConfig struct {
	Runner, SHA256 string
	Limits         profile.Limits
}
type Execution struct {
	Package, Case, Profile, Runtime, RunnerHash, Outcome string
	PID                                                  int
	Reaped                                               bool
}

func runtimeCode(err error) string {
	if errors.Is(err, ipc.ErrRunner) {
		return "IPC_RUNNER_FAILED"
	}
	if errors.Is(err, ipc.ErrProtocol) {
		return "IPC_PROTOCOL_FAILED"
	}
	return profile.Code(err)
}

func validateRuntime(ctx context.Context, c RuntimeConfig, items []staged, report func(Execution) error) (string, error) {
	var executions []Execution
	emit := func(e Execution) error { executions = append(executions, e); return report(e) }
	proofs := map[string]vm.FallbackProof{}
	needsRunner := false
	for _, item := range items {
		for _, e := range item.pkg.Entries() {
			if strings.HasSuffix(e.Path(), ".lua") {
				needsRunner = true
			}
		}
		if len(item.approval.Tests) > 0 {
			needsRunner = true
		}
	}
	if needsRunner {
		if !filepath.IsAbs(c.Runner) || c.Limits.Validate() != nil {
			return "", profile.Fail(profile.ErrConfiguration)
		}
		info, err := os.Lstat(c.Runner)
		if err != nil {
			return "", err
		}
		if !info.Mode().IsRegular() || info.Size() > 128<<20 {
			return "", profile.Fail(profile.ErrConfiguration)
		}
		data, err := os.ReadFile(c.Runner)
		if err != nil {
			return "", err
		}
		if object.Hash(data) != c.SHA256 {
			return "", profile.Fail(profile.ErrConfiguration)
		}
	}
	for _, item := range items {
		d, _ := item.pkg.Manifest()
		id := string(d.Package.PackageID)
		config := profile.Config{Limits: c.Limits, Modules: map[string][]byte{}}
		own := []string{}
		total := 0
		for _, other := range items {
			od, _ := other.pkg.Manifest()
			// A dependency's production validation sees its own locked reachable
			// graph, never its parent or an undeclared sibling from the root graph.
			if _, reachable := item.pkg.ExactLock().Package(od.Package.PackageID); !reachable {
				continue
			}
			for _, entry := range other.pkg.Entries() {
				if strings.HasSuffix(entry.Path(), ".lua") {
					name := entry.Path()
					if other.pkg != item.pkg {
						name = string(od.Package.PackageID) + "/" + name
					} else {
						own = append(own, name)
					}
					if _, ok := config.Modules[name]; ok {
						return "", profile.Fail(profile.ErrConfiguration)
					}
					data := entry.Bytes()
					total += len(data)
					config.Modules[name] = data
				}
			}
		}
		if len(own) == 0 && len(item.approval.Tests) == 0 {
			if d.CanStartSession() {
				return "", profile.Fail(profile.ErrConfiguration)
			}
			if err := emit(Execution{Package: id, Case: "no-script-structural-validation", Profile: profile.ID, Runtime: profile.RuntimeVersion, Outcome: "NOT_APPLICABLE_NO_SCRIPTS"}); err != nil {
				return "", err
			}
			continue
		}
		if total > profile.MaxModuleBytes || len(config.Modules) > 256 || len(item.approval.Tests) == 0 || d.Package.LuaProfile != profile.ID {
			return "", profile.Fail(profile.ErrConfiguration)
		}
		// Non-session packages execute in the same B002 isolated IPC profile.
		// Module results are intentionally not serialized (libraries may return
		// functions); the trusted tests must explicitly return boolean true.
		client, err := ipc.Start(ctx, c.Runner, config)
		if err != nil {
			return "", err
		}
		pid := client.PID()
		run := func(name string, source []byte) error {
			response, e := client.Call(ctx, ipc.Request{Operation: "execute", Source: source})
			if e == nil && (len(response.Result.Values) != 1 || response.Result.Values[0].Kind != "boolean" || !response.Result.Values[0].Boolean) {
				e = profile.Fail(profile.ErrScript)
			}
			event := Execution{Package: id, Case: name, Profile: profile.ID, Runtime: profile.RuntimeVersion, RunnerHash: c.SHA256, Outcome: runtimeCode(e), PID: pid}
			if auditErr := emit(event); auditErr != nil {
				return auditErr
			}
			return e
		}
		for _, name := range own {
			module := strings.ReplaceAll(strings.TrimSuffix(name, ".lua"), "/", ".")
			if err = run("module:"+name, []byte(fmt.Sprintf("local value = require(%q); return true", module))); err != nil {
				break
			}
		}
		if err == nil {
			for _, test := range item.approval.Tests {
				if err = run(test.Name, test.Source); err != nil {
					break
				}
				if test.Capability != "" {
					binding, _ := json.Marshal(struct{ Artifact, Case, Source, Behavior, Runner string }{string(item.pkg.ArtifactIdentity().Digest()), test.Name, object.Hash(test.Source), test.Behavior, c.SHA256})
					proofs[id+":"+test.Capability] = vm.FallbackProof{PackageHash: string(item.pkg.ContentHash()), Behavior: test.Behavior, EvidenceDigest: object.Hash(binding)}
				}
			}
		}
		client.Kill() // waits for cmd.Wait; no runner survives successful or failed validation
		if auditErr := emit(Execution{Package: id, Case: "ipc-destroy", Profile: profile.ID, Runtime: profile.RuntimeVersion, RunnerHash: c.SHA256, Outcome: runtimeCode(err), PID: pid, Reaped: true}); auditErr != nil {
			return "", auditErr
		}
		if err != nil {
			return "", err
		}
	}
	root := items[0]
	document, _ := root.pkg.Manifest()
	if document.CanStartSession() {
		deps := make([]*archive.Package, 0, len(items)-1)
		for _, item := range items[1:] {
			deps = append(deps, item.pkg)
		}
		s, err := vm.New(ctx, vm.Options{SessionID: "install-validation", Runner: c.Runner, Package: root.pkg, Dependencies: deps, Limits: c.Limits, State: vm.State{Value: checkpoint.Value{Kind: "nil"}}, Fallbacks: proofs, Audit: func(profile.Audit) error { return nil }})
		if err != nil {
			return "", err
		}
		pid := s.PID()
		for _, test := range root.approval.Tests {
			result, e := s.Execute(ctx, s.Token(), test.Source)
			if e == nil && (len(result.Values) != 1 || result.Values[0].Kind != "boolean" || !result.Values[0].Boolean) {
				e = profile.Fail(profile.ErrScript)
			}
			if auditErr := emit(Execution{Package: string(document.Package.PackageID), Case: "vm:" + test.Name, Profile: profile.ID, Runtime: profile.RuntimeVersion, RunnerHash: c.SHA256, Outcome: runtimeCode(e), PID: pid}); auditErr != nil {
				e = auditErr
			}
			if e != nil {
				err = e
				break
			}
		}
		destroyErr := s.Destroy()
		if auditErr := emit(Execution{Package: string(document.Package.PackageID), Case: "vm-destroy", Profile: profile.ID, Runtime: profile.RuntimeVersion, RunnerHash: c.SHA256, Outcome: runtimeCode(destroyErr), PID: pid, Reaped: true}); auditErr != nil {
			return "", auditErr
		}
		if err != nil {
			return "", err
		}
		if destroyErr != nil {
			return "", destroyErr
		}
	}
	raw, _ := json.Marshal(executions)
	return object.Hash(raw), nil
}
