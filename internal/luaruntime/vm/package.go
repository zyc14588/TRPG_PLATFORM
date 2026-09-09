// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package vm

import (
	"strings"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
)

// FallbackProof is provided by a trusted package validation caller. It binds an
// actually tested fallback to immutable content, rather than granting callbacks.
type FallbackProof struct {
	PackageHash    string
	Behavior       string
	EvidenceDigest string
}

func adapt(options Options) (checkpoint.Binding, profile.Config, []byte, error) {
	b := checkpoint.Binding{SessionID: options.SessionID, StateVersion: options.State.Version, LuaProfile: profile.ID, RuntimeVersion: profile.RuntimeVersion, PackageHashes: map[string]string{}}
	c := profile.Config{Limits: options.Limits, Modules: map[string][]byte{}}
	reject := func() (checkpoint.Binding, profile.Config, []byte, error) {
		return b, c, nil, profile.Fail(profile.ErrConfiguration)
	}
	if options.Package == nil || options.Limits.Validate() != nil {
		return reject()
	}
	document, err := options.Package.Manifest()
	if err != nil || !document.CanStartSession() {
		return reject()
	}
	if document.Package.LuaProfile != profile.ID || document.Package.HostAPI == nil {
		return reject()
	}
	entry, ok := options.Package.Entry(document.Package.Entrypoint)
	if !ok {
		return reject()
	}
	lock := options.Package.ExactLock()
	lockJSON, err := lock.CanonicalJSON()
	if err != nil {
		return reject()
	}
	b.DependencyLock = checkpoint.Hash(lockJSON)
	packages := map[string]*archive.Package{string(document.Package.PackageID): options.Package}
	for _, pkg := range options.Dependencies {
		if pkg == nil {
			return reject()
		}
		d, err := pkg.Manifest()
		if err != nil || d.Package == nil {
			return reject()
		}
		id := string(d.Package.PackageID)
		if _, exists := packages[id]; exists {
			return reject()
		}
		packages[id] = pkg
	}
	if len(packages) != len(lock.Packages()) {
		return reject()
	}
	policy, err := capability.NewTrustPolicy(map[capability.TrustLevel][]string{capability.TrustOfficial: {}, capability.TrustSigned: {}, capability.TrustPrivateUnverified: {}, capability.TrustDevelopment: {}})
	if err != nil {
		return reject()
	}
	execution, err := capability.NewGrantSet(nil)
	if err != nil {
		return reject()
	}
	moduleBytes := 0
	for _, locked := range lock.Packages() {
		id := string(locked.PackageID)
		pkg, ok := packages[id]
		if !ok || pkg.ContentHash() != locked.ContentHash {
			return reject()
		}
		d, err := pkg.Manifest()
		if err != nil || d.Package == nil || d.Package.Version != locked.Version {
			return reject()
		}
		if d.Package.LuaProfile != "" && d.Package.LuaProfile != profile.ID {
			return reject()
		}
		resolution, err := capability.Resolve(d.Package.Capabilities, capability.TrustPrivateUnverified, policy, execution)
		if err != nil {
			return b, c, nil, profile.Fail(profile.ErrCapability)
		}
		for _, fallback := range resolution.Fallbacks {
			proof, ok := options.Fallbacks[id+":"+string(fallback.Name)]
			if !ok || proof.PackageHash != string(pkg.ContentHash()) || proof.Behavior != fallback.Behavior || !checkpoint.IsDigest(proof.EvidenceDigest) {
				return b, c, nil, profile.Fail(profile.ErrCapability)
			}
		}
		b.PackageHashes[id] = string(pkg.ContentHash())
		for _, file := range pkg.Entries() {
			if !strings.HasSuffix(file.Path(), ".lua") {
				continue
			}
			name := file.Path()
			if pkg != options.Package {
				name = id + "/" + name
			}
			if _, collision := c.Modules[name]; collision {
				return reject()
			}
			source := file.Bytes()
			moduleBytes += len(source)
			if profile.ValidateSource(source) != nil || moduleBytes > profile.MaxModuleBytes || len(c.Modules) >= 256 {
				return reject()
			}
			c.Modules[name] = source
		}
	}
	if b.Validate() != nil || profile.ValidateSource(entry.Bytes()) != nil {
		return reject()
	}
	return b, c, entry.Bytes(), nil
}
