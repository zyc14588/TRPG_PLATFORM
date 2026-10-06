// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package install

import (
	"context"
	"encoding/json"

	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/vm"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

// These are immutable operator records. Neither an archive nor an extension can
// supply them. Nil retains the existing zero-grant policy and attestation bytes.
type HostAuthorization struct {
	Trust      map[capability.TrustLevel][]string
	Execution  []string
	RunnerHash string
	Limits     profile.Limits
}

func (h *HostAuthorization) grants() (capability.TrustPolicy, capability.GrantSet, error) {
	values := map[capability.TrustLevel][]string{capability.TrustOfficial: {}, capability.TrustSigned: {}, capability.TrustPrivateUnverified: {}, capability.TrustDevelopment: {}}
	var names []string
	if h != nil {
		values = h.Trust
		names = h.Execution
	}
	p, err := capability.NewTrustPolicy(values)
	if err != nil {
		return p, capability.GrantSet{}, err
	}
	g, err := capability.NewGrantSet(names)
	return p, g, err
}

type HostCase struct {
	Callback        string
	Input, Expected checkpoint.Value
	Time            int64
	Random          []int64
}

func (c HostCase) validate() error {
	if c.Callback != "command" && (!profile.IsStandardCallback(c.Callback) || c.Callback == "validate_command" || c.Callback == "execute_command") {
		return ErrPolicy
	}
	if checkpoint.Validate(c.Input) != nil || checkpoint.Validate(c.Expected) != nil || c.Time < 0 || len(c.Random) > profile.MaxHostCallbacks {
		return ErrPolicy
	}
	return nil
}

type SchemaReference struct {
	PackageID, Path, Digest string
	Seed                    checkpoint.Value
}
type NamespaceContract struct {
	Schema            SchemaReference
	Indices           map[string][]string
	MaxRows, MaxBytes int
}
type NamedContract struct {
	PackageID, ID, Plan string
	Input, Output       SchemaReference
}
type HostContract struct {
	State, Result   SchemaReference
	Results         map[string]SchemaReference
	Namespaces      map[string]NamespaceContract
	Events, Intents map[string]SchemaReference
	Named           map[string]NamedContract
	Budget          hostapi.Budget
}

func (h HostContract) validate() error {
	if h.Budget.Validate() != nil || len(h.Results)+len(h.Namespaces)+len(h.Events)+len(h.Intents)+len(h.Named) > 256 {
		return ErrPolicy
	}
	return nil
}

// Host-enabled attestations bind the authorization context and every schema,
// seed, budget and typed callback input. Legacy nil configuration is byte exact.
func (p *Policy) SigningBytes(kind string, pkg *archive.Package, signedAt int64) ([]byte, error) {
	if pkg == nil {
		return nil, ErrPolicy
	}
	a, ok := p.config.Artifacts[string(pkg.ArtifactIdentity().Digest())]
	if !ok {
		return nil, ErrPolicy
	}
	old, err := SigningBytes(kind, pkg, a.Tests, signedAt)
	if err != nil {
		return nil, err
	}
	if p.config.Host == nil && a.Host == nil {
		return old, nil
	}
	return json.Marshal(struct {
		Domain        string
		Base          json.RawMessage
		Context       string
		Authorization *HostAuthorization
		Contract      *HostContract
	}{"platform-internal-host-attestation/1", old, p.config.Context, p.config.Host, a.Host})
}

func (p *Policy) hostOptions(items []staged) (*vm.HostOptions, error) {
	upper, execution, err := p.config.Host.grants()
	if err != nil {
		return nil, err
	}
	trust := map[string]capability.TrustLevel{}
	for _, item := range items {
		if _, _, err = p.validate(item.pkg, item.evidence); err != nil {
			return nil, err
		}
		d, _ := item.pkg.Manifest()
		level := capability.TrustDevelopment
		if item.evidence.Publisher != nil && item.evidence.Certification != nil {
			level = capability.TrustSigned
		}
		trust[string(d.Package.PackageID)] = level
	}
	return &vm.HostOptions{Trust: trust, Policy: upper, Execution: execution}, nil
}

func (h HostContract) bind(session *vm.Session, packages map[string]*archive.Package, repo data.Repository, workspace string) (hostapi.Options, error) {
	bind := func(ref SchemaReference) (hostapi.Schema, error) {
		return hostapi.BindSchema(packages[ref.PackageID], ref.Path, ref.Digest, ref.Seed)
	}
	o := hostapi.Options{Session: session, Repository: repo, Packages: packages, Binding: data.Binding{Workspace: workspace, Session: session.SessionID(), GraphHash: session.GraphHash()}, Budget: h.Budget, Audit: func(data.Audit) error { return nil }, Validate: func(context.Context, data.Commit) error { return nil }, ResultSchemas: map[string]hostapi.Schema{}, Namespaces: map[string]hostapi.Namespace{}, EventSchemas: map[string]hostapi.Schema{}, IntentSchemas: map[string]hostapi.Schema{}, NamedOperations: map[string]hostapi.NamedOperation{}}
	var err error
	if o.StateSchema, err = bind(h.State); err != nil {
		return o, err
	}
	if o.ResultSchema, err = bind(h.Result); err != nil {
		return o, err
	}
	for key, ref := range h.Results {
		s, err := bind(ref)
		if err != nil {
			return o, err
		}
		o.ResultSchemas[key] = s
	}
	for key, n := range h.Namespaces {
		s, err := bind(n.Schema)
		if err != nil {
			return o, err
		}
		o.Namespaces[key] = hostapi.Namespace{Schema: s, Indices: n.Indices, MaxRows: n.MaxRows, MaxBytes: n.MaxBytes}
	}
	for key, ref := range h.Events {
		s, err := bind(ref)
		if err != nil {
			return o, err
		}
		o.EventSchemas[key] = s
	}
	for key, ref := range h.Intents {
		s, err := bind(ref)
		if err != nil {
			return o, err
		}
		o.IntentSchemas[key] = s
	}
	for key, n := range h.Named {
		in, err := bind(n.Input)
		if err != nil {
			return o, err
		}
		out, err := bind(n.Output)
		if err != nil {
			return o, err
		}
		o.NamedOperations[key] = hostapi.NamedOperation{PackageID: n.PackageID, ID: n.ID, Plan: n.Plan, Input: in, Output: out}
	}
	return o, nil
}

func graphPackages(root *archive.Package, dependencies []*archive.Package) map[string]*archive.Package {
	packages := map[string]*archive.Package{}
	for _, pkg := range append([]*archive.Package{root}, dependencies...) {
		d, _ := pkg.Manifest()
		packages[string(d.Package.PackageID)] = pkg
	}
	return packages
}

func validCaseName(name string) bool { return store.ValidID(name) }
