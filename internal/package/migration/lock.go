// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package migration defines bounded exact-lock and operator migration contracts.
// Runtime packages never receive these contracts or database handles.
package migration

import (
	"encoding/json"
	"sort"
	"strings"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

func ExactLock(g *store.Graph) (data.SessionLock, error) {
	if g == nil || g.Root() == nil {
		return data.SessionLock{}, data.ErrDenied
	}
	artifacts := g.Artifacts()
	sort.Slice(artifacts, func(i, j int) bool { return artifacts[i].PackageID < artifacts[j].PackageID })
	raw, err := json.Marshal(artifacts)
	if err != nil {
		return data.SessionLock{}, err
	}
	digest, err := g.Root().ExactLock().Digest()
	if err != nil {
		return data.SessionLock{}, err
	}
	l := data.SessionLock{Format: 1, RootIdentity: string(g.Root().ArtifactIdentity().Digest()), GraphHash: string(digest), DependencyLock: g.Root().LockBytes(), ArtifactSet: raw}
	packages := map[string]*archive.Package{}
	for _, p := range append([]*archive.Package{g.Root()}, g.Dependencies()...) {
		d, e := p.Manifest()
		if e != nil {
			return l, e
		}
		packages[string(d.Package.PackageID)] = p
	}
	for _, a := range artifacts {
		p := packages[a.PackageID]
		if p == nil {
			return l, data.ErrDenied
		}
		d, e := p.Manifest()
		if e != nil {
			return l, e
		}
		v := d.Package
		n := data.LockedArtifact{Identity: a.Identity, PackageID: a.PackageID, Version: string(v.Version), Kind: string(v.PackageKind), ContentHash: a.ContentHash, PolicyDigest: a.PolicyDigest, ValidationDigest: a.ValidationDigest, LuaProfile: v.LuaProfile, Schemas: map[string]string{}}
		if v.HostAPI != nil {
			n.HostAPI = &[3]uint32{v.HostAPI.Major, v.HostAPI.MinMinor, v.HostAPI.MaxMinor}
		}
		for _, entry := range p.Entries() {
			if strings.HasSuffix(entry.Path(), ".schema.json") {
				n.Schemas[entry.Path()] = checkpoint.Hash(entry.Bytes())
			}
		}
		l.Packages = append(l.Packages, n)
	}
	l.Hash = data.LockHash(l)
	if err = data.ValidateSessionLock(l); err != nil {
		return data.SessionLock{}, err
	}
	return l, nil
}
