// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package store

import (
	"bytes"
	"context"
	"encoding/json"
	"sort"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
)

// Graph can only be produced by an authorized installed Reader. Its package
// bytes, exact lock, workspace membership and installation evidence are owned.
type Graph struct {
	root         *archive.Package
	dependencies []*archive.Package
	membership   Membership
	artifacts    []GraphArtifact
}
type GraphArtifact struct {
	Identity, PackageID, ContentHash, PolicyDigest, ValidationDigest string
}

func (g *Graph) Root() *archive.Package { return g.root }
func (g *Graph) Dependencies() []*archive.Package {
	return append([]*archive.Package(nil), g.dependencies...)
}
func (g *Graph) Membership() Membership     { return g.membership }
func (g *Graph) Artifacts() []GraphArtifact { return append([]GraphArtifact(nil), g.artifacts...) }

func (r *Reader) LoadGraph(ctx context.Context, c Credential, workspace, root string, dependencies []string) (*Graph, error) {
	if len(dependencies) > 127 {
		return nil, ErrDenied
	}
	m, err := r.access.Authorize(c, workspace, false)
	if err != nil {
		return nil, err
	}
	g := &Graph{membership: m}
	seen := map[string]bool{}
	for index, id := range append([]string{root}, dependencies...) {
		if seen[id] {
			return nil, object.ErrIntegrity
		}
		seen[id] = true
		pkg, a, err := r.load(ctx, c, workspace, id)
		if err != nil {
			return nil, err
		}
		if index == 0 {
			g.root = pkg
		} else {
			g.dependencies = append(g.dependencies, pkg)
		}
		g.artifacts = append(g.artifacts, GraphArtifact{a.Identity, a.PackageID, a.ContentHash, a.PolicyDigest, a.ValidationDigest})
	}
	if err = VerifyExactGraph(g.root, g.dependencies); err != nil {
		return nil, err
	}
	sort.Slice(g.dependencies, func(i, j int) bool {
		return g.dependencies[i].ExactLock().Root() < g.dependencies[j].ExactLock().Root()
	})
	return g, nil
}

// VerifyExactGraph also checks each artifact's complete reachable lock, including
// feature selections and transitive edges. It grants no workspace access.
func VerifyExactGraph(root *archive.Package, dependencies []*archive.Package) error {
	if root == nil || len(dependencies) > 127 {
		return object.ErrIntegrity
	}
	lock := root.ExactLock()
	items := append([]*archive.Package{root}, dependencies...)
	if len(lock.Packages()) != len(items) {
		return object.ErrIntegrity
	}
	seen := map[string]bool{}
	for _, pkg := range items {
		if pkg == nil {
			return object.ErrIntegrity
		}
		d, err := pkg.Manifest()
		if err != nil || d.Package == nil {
			return object.ErrIntegrity
		}
		id := string(d.Package.PackageID)
		if seen[id] {
			return object.ErrIntegrity
		}
		seen[id] = true
		node, ok := lock.Package(d.Package.PackageID)
		if !ok || node.ContentHash != pkg.ContentHash() || node.Version != d.Package.Version {
			return object.ErrIntegrity
		}
		for _, sub := range pkg.ExactLock().Packages() {
			outer, ok := lock.Package(sub.PackageID)
			a, _ := json.Marshal(sub)
			b, _ := json.Marshal(outer)
			if !ok || !bytes.Equal(a, b) {
				return object.ErrIntegrity
			}
		}
	}
	return nil
}
