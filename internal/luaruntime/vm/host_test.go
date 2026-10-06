// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package vm

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"reflect"
	"strings"
	"sync"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
)

const hostCallbacks = `local M={};for _,n in ipairs({"on_session_create","on_session_restore","on_session_start","list_legal_actions","project_view","create_checkpoint","restore_checkpoint","resume_continuation","on_safe_migration_boundary","on_session_end","cleanup"}) do M[n]=function() return {} end end
M.validate_command=function(c) return true end
`

func hostOptions(t *testing.T, pkg *archive.Package) Options {
	t.Helper()
	o := options(t, "host-test", pkg)
	policy, e := capability.NewTrustPolicy(map[capability.TrustLevel][]string{capability.TrustOfficial: {"host.state"}, capability.TrustSigned: {"host.state"}, capability.TrustPrivateUnverified: {"host.state"}, capability.TrustDevelopment: {"host.state"}})
	if e != nil {
		t.Fatal(e)
	}
	grant, e := capability.NewGrantSet([]string{"host.state"})
	if e != nil {
		t.Fatal(e)
	}
	o.Host = &HostOptions{Trust: map[string]capability.TrustLevel{"example.test/runtime": capability.TrustPrivateUnverified}, Policy: policy, Execution: grant}
	return o
}
func hostManifest() string {
	return strings.Replace(fixtureManifest, "required = []", "required = [\"host.state\"]", 1)
}

func TestActualExecutionTokenDiagnosticsPreserveAuthority(t *testing.T) {
	pkg := fixture(t, 1, hostManifest(), hostCallbacks+`M.execute_command=function() return host.state.get({"counter"}) end;return M`)
	s, err := New(context.Background(), hostOptions(t, pkg))
	if err != nil {
		t.Fatal("actual VM prerequisite failed", profile.Code(err))
	}
	defer s.Destroy()
	token := s.Token()
	if token.secret == ([32]byte{}) {
		t.Fatal("actual token prerequisite empty")
	}
	invoke := func() {
		t.Helper()
		result, err := s.Invoke(context.Background(), token, s.SessionID(), "command", nil, func(_ context.Context, call profile.HostCall) (checkpoint.Value, error) {
			if call.Module != "lua/main.lua" || call.Line < 1 || call.Capability != "host.state" {
				t.Fatal("authenticated module boundary not reached")
			}
			return checkpoint.Int(42), nil
		})
		if err != nil || len(result.Values) != 1 || result.Values[0].Number != "42" {
			t.Fatal("execution token did not authorize actual callback", profile.Code(err))
		}
	}
	invoke()
	containers := []any{token, &token, []Token{token}, [1]Token{token}, map[string]Token{"token": token}, struct{ Token Token }{token}}
	for _, format := range []string{"%v", "%+v", "%#v", "%s", "%q", "%x", "%X", "%d", "%b", "%o", "%f", "%e", "%g", "%c", "%U", "%t", "%1000000d", "%.1000000x"} {
		t.Run(format, func(t *testing.T) {
			values := containers
			if strings.Contains(format, "1000000") {
				// A composite's public keys have their own fmt width semantics.
				values = containers[:2]
			}
			for _, container := range values {
				output := fmt.Sprintf(format, container)
				if !strings.Contains(output, "<execution-token>") || len(output) > 256 {
					t.Fatal("token diagnostic was not bounded and opaque; raw output intentionally omitted")
				}
			}
		})
	}
	for _, value := range []any{token, &token} {
		if _, err := json.Marshal(value); err == nil {
			t.Fatal("execution token entered JSON")
		}
	}
	if token.secret != s.Token().secret {
		t.Fatal("diagnostics changed the execution capability")
	}
	invoke()
}

func TestActualSessionHandleDiagnosticsPreserveAuthorityAndState(t *testing.T) {
	const private = "fixture-gm-private-value"
	pkg := fixture(t, 1, hostManifest(), hostCallbacks+`M.execute_command=function() return host.state.get({"counter"}) end;return M`)
	o := hostOptions(t, pkg)
	o.State.Value = checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(7), "gm": checkpoint.Text(private)})
	s, err := New(context.Background(), o)
	if err != nil {
		t.Fatal("actual VM prerequisite failed", profile.Code(err))
	}
	defer s.Destroy()
	token := s.Token()
	before, err := json.Marshal(s.state.Value)
	if err != nil || !strings.Contains(string(before), private) {
		t.Fatal("actual state prerequisite failed", profile.Code(err))
	}
	invoke := func() {
		t.Helper()
		result, err := s.Invoke(context.Background(), token, s.SessionID(), "command", nil, func(_ context.Context, call profile.HostCall) (checkpoint.Value, error) {
			if call.Module != "lua/main.lua" || call.Line < 1 || call.Capability != "host.state" {
				t.Fatal("authenticated module boundary not reached")
			}
			return checkpoint.Int(42), nil
		})
		if err != nil || len(result.Values) != 1 || result.Values[0].Number != "42" {
			t.Fatal("session diagnostics changed actual callback authority", profile.Code(err))
		}
	}
	invoke()
	// A diagnostic framework can box the public handle as a value. It must never
	// use that copy to execute a VM; here it is only formatted, like a logger.
	value := reflect.ValueOf(s).Elem().Interface()
	containers := []any{s, value, []*Session{s}, [1]*Session{s}, map[string]*Session{"session": s}, struct{ Session *Session }{s}, []any{value}, map[string]any{"session": value}, struct{ Session any }{value}}
	for _, format := range []string{"%v", "%+v", "%#v", "%s", "%q", "%x", "%X", "%d", "%b", "%o", "%f", "%e", "%g", "%c", "%U", "%t", "%1000000d", "%.1000000x"} {
		t.Run(format, func(t *testing.T) {
			values := containers
			if strings.Contains(format, "1000000") {
				values = containers[:2]
			}
			for _, container := range values {
				output := fmt.Sprintf(format, container)
				if !strings.Contains(output, "<session-vm:redacted>") || len(output) > 256 || strings.Contains(output, private) {
					t.Fatal("session handle diagnostic exposed private data; raw output intentionally omitted")
				}
			}
		})
	}
	after, err := json.Marshal(s.state.Value)
	if err != nil || string(before) != string(after) || token.secret != s.Token().secret {
		t.Fatal("session diagnostics changed authoritative state or execution capability")
	}
	invoke()
}

func TestFullPackageHashCopyCannotChangeVMOrModuleAuthority(t *testing.T) {
	pkg := fixture(t, 1, hostManifest(), hostCallbacks+`M.execute_command=function() return host.state.get({"counter"}) end;return M`)
	s, err := New(context.Background(), hostOptions(t, pkg))
	if err != nil {
		t.Fatal(err)
	}
	defer s.Destroy()
	original := s.PackageHashes()
	if len(original) != 1 || original["example.test/runtime"] != string(pkg.ContentHash()) {
		t.Fatal("authenticated graph binding absent", original)
	}
	copy := s.PackageHashes()
	delete(copy, "example.test/runtime")
	copy["example.test/forged"] = "sha256:" + strings.Repeat("a", 64)
	if current := s.PackageHashes(); len(current) != 1 || current["example.test/runtime"] != original["example.test/runtime"] {
		t.Fatal("returned map mutated VM authority", current)
	}
	if _, ok := s.ModuleIdentity("example.test/forged:lua/main.lua"); ok {
		t.Fatal("graph copy created a module identity")
	}
	result, err := s.Invoke(context.Background(), s.Token(), s.SessionID(), "command", nil, func(_ context.Context, c profile.HostCall) (checkpoint.Value, error) {
		if c.Module != "lua/main.lua" {
			t.Fatal("module authority changed", c.Module)
		}
		return checkpoint.Int(42), nil
	})
	if err != nil || len(result.Values) != 1 || result.Values[0].Number != "42" {
		t.Fatal("VM was changed by hash copy", result, err)
	}
	if err := s.AcceptCommittedState(2, checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(42)})); err != nil {
		t.Fatal(err)
	}
	if current := s.PackageHashes(); len(current) != 1 || current["example.test/runtime"] != original["example.test/runtime"] {
		t.Fatal("commit changed package authority", current)
	}
	var readers sync.WaitGroup
	for i := 0; i < 8; i++ {
		readers.Add(1)
		go func() {
			defer readers.Done()
			for n := 0; n < 32; n++ {
				copy := s.PackageHashes()
				copy["example.test/runtime"] = "changed"
				delete(copy, "example.test/runtime")
			}
		}()
	}
	for version := uint64(3); version <= 10; version++ {
		if err := s.AcceptCommittedState(version, checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(42)})); err != nil {
			t.Fatal(err)
		}
	}
	readers.Wait()
	if current := s.PackageHashes(); len(current) != 1 || current["example.test/runtime"] != original["example.test/runtime"] {
		t.Fatal("concurrent readers changed package authority", current)
	}
}
func TestHostVersionAndRequiredEntrypointsFailClosed(t *testing.T) {
	for _, c := range []struct{ name, text string }{{"major", strings.Replace(hostManifest(), "major = 1", "major = 2", 1)}, {"minor", strings.Replace(strings.Replace(hostManifest(), "min_minor = 0", "min_minor = 1", 1), "max_minor = 0", "max_minor = 1", 1)}} {
		t.Run(c.name, func(t *testing.T) {
			o := hostOptions(t, fixture(t, 1, c.text, hostCallbacks+`M.execute_command=function() return 1 end;return M`))
			s, e := New(context.Background(), o)
			if e == nil {
				s.Destroy()
				t.Fatal("incompatible Host API loaded")
			}
		})
	}
	for _, name := range profile.StandardCallbackNames() {
		if name == "resume_continuation" {
			continue
		}
		t.Run("missing-"+name, func(t *testing.T) {
			source := hostCallbacks + `M.execute_command=function() return 1 end;M["` + name + `"]=nil;return M`
			s, e := New(context.Background(), hostOptions(t, fixture(t, 1, hostManifest(), source)))
			if e == nil {
				s.Destroy()
				t.Fatal("missing required standard entrypoint loaded")
			}
		})
	}
}
func TestHostIntersectionsAndConditionalEntrypoint(t *testing.T) {
	source := hostCallbacks + `M.resume_continuation=nil;M.execute_command=function() return host.state.get({"counter"}) end;return M`
	o := hostOptions(t, fixture(t, 1, hostManifest(), source))
	s, e := New(context.Background(), o)
	if e != nil {
		t.Fatal("conditional resume required without async authority", e)
	}
	s.Destroy()
	for _, missing := range []string{"context", "trust", "mapping", "default"} {
		t.Run(missing, func(t *testing.T) {
			bad := o
			h := *o.Host
			bad.Host = &h
			switch missing {
			case "context":
				h.Execution = capability.GrantSet{}
			case "trust":
				h.Policy = capability.TrustPolicy{}
			case "mapping":
				h.Trust = map[string]capability.TrustLevel{}
			case "default":
				bad.Host = nil
			}
			s, e := New(context.Background(), bad)
			if e == nil {
				s.Destroy()
				t.Fatal("missing grant layer admitted required authority")
			}
		})
	}
}
func TestHostModuleOriginCannotBorrowRootAuthority(t *testing.T) {
	depText := strings.Replace(fixtureManifest, "example.test/runtime", "example.test/library", 1)
	for _, c := range []struct{ name, dep, root string }{
		{"direct", `return function() return host.state.get({"counter"}) end`, `return dep()`},
		{"cached-native", `local borrowed=host.state.get;return function() return borrowed({"counter"}) end`, `return dep()`},
		{"caught", `return function() pcall(function() return host.state.get({"counter"}) end);return 1 end`, `return dep()`},
		{"coroutine", `return function() local co=coroutine.create(function() return host.state.get({"counter"}) end);local ok,value=coroutine.resume(co);return 1 end`, `return dep()`},
	} {
		t.Run(c.name, func(t *testing.T) {
			dep := fixture(t, 1, depText, c.dep)
			text := hostManifest() + "\n[[dependencies]]\npackage_id = \"example.test/library\"\nversion = \"1.0.0\"\noptional = false\n"
			rootNode, e := dependency.NewLockedPackage("example.test/runtime", "1.0.0", "sha256:"+strings.Repeat("0", 64), nil, []string{"example.test/library"})
			if e != nil {
				t.Fatal(e)
			}
			depNode, e := dependency.NewLockedPackage("example.test/library", "1.0.0", string(dep.ContentHash()), nil, nil)
			if e != nil {
				t.Fatal(e)
			}
			lock, e := dependency.BuildExactLock("example.test/runtime", []dependency.LockedPackage{rootNode, depNode})
			if e != nil {
				t.Fatal(e)
			}
			source := `local dep=require("example.test/library:lua.main");` + hostCallbacks + `M.execute_command=function() ` + c.root + ` end;return M`
			pkg, e := archive.FromFiles(map[string][]byte{archive.ManifestPath: []byte(text), "lua/main.lua": []byte(source)}, lock, extension.DefaultSupport)
			if e != nil {
				t.Fatal(e)
			}
			o := hostOptions(t, pkg)
			o.Dependencies = []*archive.Package{dep}
			o.Host.Trust["example.test/library"] = capability.TrustOfficial
			s, e := New(context.Background(), o)
			if e != nil {
				t.Fatal(e)
			}
			defer s.Destroy()
			pid := s.PID()
			calls := 0
			_, e = s.Invoke(context.Background(), s.Token(), o.SessionID, "command", nil, func(context.Context, profile.HostCall) (checkpoint.Value, error) {
				calls++
				return checkpoint.Int(42), nil
			})
			if profile.Code(e) != profile.ErrCapability || calls != 0 {
				t.Fatal("dependency borrowed root capability", calls, e)
			}
			if _, e = os.Stat(fmt.Sprintf("/proc/%d", pid)); !os.IsNotExist(e) {
				t.Fatal("contaminated runner not reaped", e)
			}
		})
	}
}
func TestAuthorizedOriginAndOpaqueTokenStayInParent(t *testing.T) {
	source := hostCallbacks + `M.execute_command=function() assert(debug==nil and load==nil);local get=host.state.get;return get({"counter"}) end;return M`
	s, e := New(context.Background(), hostOptions(t, fixture(t, 1, hostManifest(), source)))
	if e != nil {
		t.Fatal(e)
	}
	defer s.Destroy()
	result, e := s.Invoke(context.Background(), s.Token(), s.SessionID(), "command", nil, func(_ context.Context, c profile.HostCall) (checkpoint.Value, error) {
		if c.Module != "lua/main.lua" || c.Line < 1 || c.Phase != "execute" || c.Capability != "host.state" {
			t.Fatal("origin not bound", c)
		}
		return checkpoint.Int(42), nil
	})
	if e != nil || len(result.Values) != 1 || result.Values[0].Number != "42" {
		t.Fatal(result, e)
	}
	if _, e = s.Execute(context.Background(), s.Token(), []byte(`return host.state.get({"counter"})`)); profile.Code(e) != profile.ErrCapability {
		t.Fatal("unbound source could invoke Host", e)
	}
}
