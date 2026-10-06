// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package migration_test

import (
	"encoding/json"
	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/migration"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/migration"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"sort"
	"testing"
)

func fixtureLock(t *testing.T) data.SessionLock {
	t.Helper()
	pair, err := fixture.BuildPair(install.RuntimeConfig{SHA256: eventstore.Digest("synthetic-unit-runner"), Limits: profile.DefaultLimits()}, true, false)
	if err != nil {
		t.Fatal(err)
	}
	g := pair.Old
	lock := data.SessionLock{Format: 1, RootIdentity: string(g.Root.ArtifactIdentity().Digest()), GraphHash: g.Hash(), DependencyLock: g.Root.LockBytes()}
	artifacts := []store.GraphArtifact{}
	for _, p := range append([]*archive.Package{g.Root}, g.Dependencies...) {
		d, _ := p.Manifest()
		v := data.LockedArtifact{Identity: string(p.ArtifactIdentity().Digest()), PackageID: string(d.Package.PackageID), Version: string(d.Package.Version), Kind: string(d.Package.PackageKind), ContentHash: string(p.ContentHash()), PolicyDigest: eventstore.Digest("unit-policy"), ValidationDigest: eventstore.Digest("unit-validation"), LuaProfile: d.Package.LuaProfile, Schemas: map[string]string{}}
		if h := d.Package.HostAPI; h != nil {
			v.HostAPI = &[3]uint32{h.Major, h.MinMinor, h.MaxMinor}
		}
		for _, entry := range p.Entries() {
			if len(entry.Path()) >= 12 && entry.Path()[len(entry.Path())-12:] == ".schema.json" {
				v.Schemas[entry.Path()] = eventstore.Digest(json.RawMessage(entry.Bytes()))
			}
		}
		lock.Packages = append(lock.Packages, v)
	}
	sort.Slice(lock.Packages, func(i, j int) bool { return lock.Packages[i].PackageID < lock.Packages[j].PackageID })
	for _, v := range lock.Packages {
		artifacts = append(artifacts, store.GraphArtifact{Identity: v.Identity, PackageID: v.PackageID, ContentHash: v.ContentHash, PolicyDigest: v.PolicyDigest, ValidationDigest: v.ValidationDigest})
	}
	lock.ArtifactSet, _ = json.Marshal(artifacts)
	lock.Hash = data.LockHash(lock)
	return lock
}
func TestExactLockRejectsUnauthenticatedGraph(t *testing.T) {
	if _, err := migration.ExactLock(nil); err == nil {
		t.Fatal("unauthenticated graph accepted")
	}
}
func TestFiveRoleLockRejectsSelfHashedSubstitution(t *testing.T) {
	original := fixtureLock(t)
	if data.ValidateSessionLock(original) != nil || len(original.Packages) != 5 {
		t.Fatal("valid fixture lock rejected")
	}
	cases := map[string]func(*data.SessionLock){"version": func(l *data.SessionLock) { l.Packages[0].Version = "^1.0.0" }, "content": func(l *data.SessionLock) { l.Packages[0].ContentHash = eventstore.Digest("other") }, "identity": func(l *data.SessionLock) { l.Packages[0].Identity = eventstore.Digest("other") }, "host-range": func(l *data.SessionLock) { l.Packages[0].HostAPI = &[3]uint32{1, 2, 1} }, "schema-path": func(l *data.SessionLock) {
		l.Packages[0].Schemas["../escape.schema.json"] = eventstore.Digest("escape")
	}, "schema-hash": func(l *data.SessionLock) {
		for k := range l.Packages[0].Schemas {
			l.Packages[0].Schemas[k] = "bad"
		}
	}, "package-role": func(l *data.SessionLock) { l.Packages[0].Kind = "service" }, "duplicate": func(l *data.SessionLock) { l.Packages = append(l.Packages, l.Packages[0]) }, "lock-range": func(l *data.SessionLock) { l.DependencyLock = []byte(`{"version":"^1.0.0"}`) }}
	for name, change := range cases {
		t.Run(name, func(t *testing.T) {
			l := eventstore.Copy(original)
			change(&l)
			l.Hash = data.LockHash(l)
			if data.ValidateSessionLock(l) == nil {
				t.Fatal("self-hashed lock substitution accepted")
			}
		})
	}
}
