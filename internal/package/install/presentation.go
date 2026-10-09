// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package install

import (
	"context"
	"slices"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

// PresentationPackage contains public manifest facts only. A projection never
// transfers the authenticated factory, graph, credentials or execution handles.
type PresentationPackage struct {
	PackageID      string   `json:"package_id"`
	Version        string   `json:"version"`
	Title          string   `json:"title"`
	ArtifactDigest string   `json:"artifact_digest"`
	RightsDigest   string   `json:"rights_digest"`
	License        string   `json:"license"`
	Permissions    []string `json:"permissions"`
}

// Presentation authenticates exactly the startup graph, including current ACL,
// policy, rights attestations and the bounded production quarantine suite. It
// does not call open, Create, Resume or ProvisionGraph and creates no game VM.
func (f *SessionFactory) Presentation(ctx context.Context, r SessionRequest) (string, []PresentationPackage, error) {
	if f == nil || ctx == nil || ctx.Err() != nil || len(r.Dependencies) > 63 {
		return "", nil, ErrPolicy
	}
	p, e := f.prepare(ctx, r)
	if e != nil {
		return "", nil, e
	}
	if ctx.Err() != nil || p.graph == nil || p.graph.Root() == nil || !checkpoint.IsDigest(p.binding.GraphHash) || p.binding.Workspace != r.Workspace || p.binding.Session != r.Session {
		return "", nil, ErrPolicy
	}
	all := append([]*archive.Package{p.graph.Root()}, p.graph.Dependencies()...)
	if len(all) < 1 || len(all) > 64 {
		return "", nil, ErrPolicy
	}
	out := make([]PresentationPackage, 0, len(all))
	seen := map[string]bool{}
	for i, pkg := range all {
		if ctx.Err() != nil || pkg == nil {
			return "", nil, ErrPolicy
		}
		m, err := pkg.Manifest()
		if err != nil || m.Package == nil || i == 0 && m.Package.PackageKind != model.PackageKindGameSystem {
			return "", nil, ErrPolicy
		}
		x := m.Package
		if seen[string(x.PackageID)] {
			return "", nil, ErrPolicy
		}
		seen[string(x.PackageID)] = true
		permissions := []string{}
		for _, required := range x.Capabilities.Required {
			permissions = append(permissions, string(required))
		}
		for _, optional := range x.Capabilities.Optional {
			permissions = append(permissions, string(optional.Name))
		}
		slices.Sort(permissions)
		if len(permissions) > 64 || len(slices.Compact(slices.Clone(permissions))) != len(permissions) {
			return "", nil, ErrPolicy
		}
		artifact, rights := string(pkg.ArtifactIdentity().Digest()), RightsDigest(*x)
		if !checkpoint.IsDigest(artifact) || !checkpoint.IsDigest(rights) {
			return "", nil, ErrPolicy
		}
		out = append(out, PresentationPackage{string(x.PackageID), string(x.Version), x.DisplayName, artifact, rights, x.Rights.LicenseExpression, permissions})
	}
	slices.SortFunc(out, func(a, b PresentationPackage) int {
		if a.PackageID < b.PackageID {
			return -1
		}
		if a.PackageID > b.PackageID {
			return 1
		}
		return 0
	})
	return p.binding.GraphHash, out, nil
}
