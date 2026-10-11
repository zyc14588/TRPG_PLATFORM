// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package m2smoke

import (
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"testing"
)

func TestCarrierDependencyHasExactStandaloneSubgraphAndPolicy(t *testing.T) {
	root, dep, policy, e := BuildCarrierWithDependency(install.RuntimeConfig{SHA256: checkpoint.Hash([]byte("owned real runtime supplied by lifecycle")), Limits: profile.DefaultLimits()})
	if e != nil {
		t.Fatal(e)
	}
	if len(root.ExactLock().Packages()) != 2 || len(dep.ExactLock().Packages()) != 1 || len(policy.Artifacts) != 2 {
		t.Fatal("standalone graph or exact approvals missing")
	}
	actual, _ := dep.Manifest()
	node, ok := root.ExactLock().Package(actual.Package.PackageID)
	if !ok || node.ContentHash != dep.ContentHash() {
		t.Fatal("dependency absent from root exact lock")
	}
	if _, e = install.NewPolicy(policy); e != nil {
		t.Fatal(e)
	}
	for _, pkg := range []*archive.Package{root, dep} {
		snapshot, e := pkg.Export()
		if e != nil {
			t.Fatal(e)
		}
		loaded, e := archive.Import(snapshot, extension.DefaultSupport)
		_ = loaded
		if e != nil {
			t.Fatal(e)
		}
	}
}
