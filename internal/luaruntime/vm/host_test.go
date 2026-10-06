// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package vm

import (
	"context"
	"fmt"
	"os"
	"strings"
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
