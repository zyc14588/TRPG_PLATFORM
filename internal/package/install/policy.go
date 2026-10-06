// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package install

import (
	"bytes"
	"crypto/ed25519"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/manifest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
)

var ErrPolicy = errors.New("installation policy denied")

// These are internal operator policy records, not a package signature format.
// Detached attestations are verified against configured keys and exact bytes;
// there is no publisher, certification, or revocation service in this batch.
type Key struct {
	Public                         ed25519.PublicKey
	Publisher                      string
	Certification                  bool
	State                          string
	NotBefore, NotAfter, RetiredAt int64
}
type Attestation struct {
	KeyID     string
	SignedAt  int64
	Signature []byte
}
type Evidence struct{ Publisher, Certification *Attestation }
type Test struct {
	Name                 string
	Source               []byte
	Capability, Behavior string
	Host                 *HostCase `json:",omitempty"`
}
type Approval struct {
	RightsDigest, Retention, Safety string
	Tests                           []Test
	Host                            *HostContract `json:",omitempty"`
}
type PolicyConfig struct {
	Context              string // development, ci, or production; never taken from a package
	HostMajor, HostMinor uint32
	Keys                 map[string]Key
	Artifacts            map[string]Approval // exact artifact identity -> trusted rights/safety
	Host                 *HostAuthorization  `json:",omitempty"`
}
type Policy struct {
	config PolicyConfig
	digest string
}

func NewPolicy(c PolicyConfig) (*Policy, error) {
	if c.Context != "development" && c.Context != "ci" && c.Context != "production" {
		return nil, ErrPolicy
	}
	// JSON copies every slice/map; subsequent caller mutations cannot change policy.
	raw, err := json.Marshal(c)
	if err != nil {
		return nil, err
	}
	var owned PolicyConfig
	if err = json.Unmarshal(raw, &owned); err != nil {
		return nil, err
	}
	if owned.Host != nil {
		if _, _, err = owned.Host.grants(); err != nil {
			return nil, ErrPolicy
		}
		if !checkpoint.IsDigest(owned.Host.RunnerHash) || owned.Host.Limits.Validate() != nil {
			return nil, ErrPolicy
		}
	}
	for id, k := range owned.Keys {
		if id == "" || len(k.Public) != ed25519.PublicKeySize || k.NotBefore <= 0 || k.NotAfter <= k.NotBefore {
			return nil, ErrPolicy
		}
		switch k.State {
		case "ACTIVE", "RETIRED", "REVOKED", "COMPROMISED", "EXPIRED":
		default:
			return nil, ErrPolicy
		}
		if k.State == "RETIRED" && k.RetiredAt <= k.NotBefore {
			return nil, ErrPolicy
		}
	}
	for id, a := range owned.Artifacts {
		if len(id) != 71 || a.RightsDigest == "" || a.Retention == "" || len(a.Retention) > 128 || len(a.Tests) > 256 {
			return nil, ErrPolicy
		}
		seen := map[string]bool{}
		if a.Host != nil && (owned.Host == nil || a.Host.validate() != nil) {
			return nil, ErrPolicy
		}
		for _, test := range a.Tests {
			if test.Name == "" || seen[test.Name] || (test.Host == nil && profile.ValidateSource(test.Source) != nil) || (test.Host != nil && (a.Host == nil || len(test.Source) != 0 || test.Capability != "" || test.Host.validate() != nil)) {
				return nil, ErrPolicy
			}
			seen[test.Name] = true
		}
	}
	return &Policy{owned, object.Hash(raw)}, nil
}
func (p *Policy) Digest() string { return p.digest }
func RightsDigest(pkg manifest.Package) string {
	raw, _ := json.Marshal(pkg.Rights)
	return object.Hash(raw)
}
func SuiteDigest(tests []Test) string { raw, _ := json.Marshal(tests); return object.Hash(raw) }

// SigningBytes is the internal, domain-separated attestation message used by
// trusted callers. A certification binds the exact lock, runtime and test inputs.
func SigningBytes(kind string, pkg *archive.Package, tests []Test, signedAt int64) ([]byte, error) {
	if kind != "publisher" && kind != "certification" {
		return nil, ErrPolicy
	}
	d, err := pkg.Manifest()
	if err != nil {
		return nil, err
	}
	lock, err := pkg.ExactLock().Digest()
	if err != nil {
		return nil, err
	}
	return json.Marshal(struct {
		Domain, Kind, Artifact, Content, Lock, Profile, Runtime, Suite string
		HostAPI                                                        *manifest.HostAPIRange
		SignedAt                                                       int64
	}{"platform-internal-install-attestation/1", kind, string(pkg.ArtifactIdentity().Digest()), string(pkg.ContentHash()), string(lock), profile.ID, profile.RuntimeVersion, SuiteDigest(tests), d.Package.HostAPI, signedAt})
}

func (p *Policy) validate(pkg *archive.Package, e Evidence) (Approval, capability.Resolution, error) {
	identity := string(pkg.ArtifactIdentity().Digest())
	a, ok := p.config.Artifacts[identity]
	d, err := pkg.Manifest()
	if err != nil {
		return Approval{}, capability.Resolution{}, err
	}
	reject := func() (Approval, capability.Resolution, error) { return Approval{}, capability.Resolution{}, ErrPolicy }
	if !ok || a.Safety != "ACTIVE" || RightsDigest(*d.Package) != a.RightsDigest {
		return reject()
	}
	if d.Package.HostAPI != nil {
		h := d.Package.HostAPI
		if h.Major != p.config.HostMajor || h.MinMinor > p.config.HostMinor || h.MaxMinor < p.config.HostMinor {
			return reject()
		}
	}
	if d.Package.LuaProfile != "" && d.Package.LuaProfile != profile.ID {
		return reject()
	}
	level := capability.TrustDevelopment
	if e.Publisher == nil {
		if p.config.Context == "production" || e.Certification != nil {
			return reject()
		}
	} else {
		if err = p.verify("publisher", pkg, e.Publisher); err != nil {
			return reject()
		}
		if e.Certification != nil {
			if err = p.verify("certification", pkg, e.Certification); err != nil {
				return reject()
			}
			if bytes.Equal(p.config.Keys[e.Publisher.KeyID].Public, p.config.Keys[e.Certification.KeyID].Public) {
				return reject()
			}
			level = capability.TrustSigned
		} else if p.config.Context == "production" {
			return reject()
		}
	}
	upper, execution, err := p.config.Host.grants()
	if err != nil {
		return reject()
	}
	resolved, err := capability.Resolve(d.Package.Capabilities, level, upper, execution)
	if err != nil {
		return a, resolved, fmt.Errorf("capabilities: %w", err)
	}
	for _, fallback := range resolved.Fallbacks {
		found := false
		for _, test := range a.Tests {
			if test.Capability == string(fallback.Name) && test.Behavior == fallback.Behavior {
				found = true
			}
		}
		if !found {
			return reject()
		}
	}
	return a, resolved, nil
}

func (p *Policy) verify(kind string, pkg *archive.Package, a *Attestation) error {
	k, ok := p.config.Keys[a.KeyID]
	if !ok || k.State != "ACTIVE" && k.State != "RETIRED" || a.SignedAt < k.NotBefore || a.SignedAt > k.NotAfter || a.SignedAt > time.Now().Unix() {
		return ErrPolicy
	}
	if k.State == "RETIRED" && a.SignedAt >= k.RetiredAt {
		return ErrPolicy
	}
	d, err := pkg.Manifest()
	if err != nil {
		return err
	}
	publisher, _, _ := strings.Cut(string(d.Package.PackageID), "/")
	if kind == "publisher" && (k.Certification || k.Publisher != publisher) || kind == "certification" && !k.Certification {
		return ErrPolicy
	}
	message, err := p.SigningBytes(kind, pkg, a.SignedAt)
	if err != nil {
		return err
	}
	if !ed25519.Verify(k.Public, message, a.Signature) {
		return ErrPolicy
	}
	return nil
}
