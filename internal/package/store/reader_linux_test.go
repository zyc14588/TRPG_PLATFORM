//go:build linux

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package store

import (
	"context"
	"encoding/json"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
	"os"
	"testing"
)

type readRepository struct {
	Repository
	artifact Artifact
}

func (r readRepository) Lookup(context.Context, string, string, string) (Artifact, error) {
	return r.artifact, nil
}

func TestReaderVerifiesCompleteContentReferencesAndMetadata(t *testing.T) {
	ctx := context.Background()
	root := t.TempDir()
	if err := os.Chmod(root, 0700); err != nil {
		t.Fatal(err)
	}
	objects, err := object.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	defer objects.Close()
	p, err := fixtures.Build(fixtures.Files("test.publisher/reader", "assets", ""))
	if err != nil {
		t.Fatal(err)
	}
	raw, err := fixtures.Archive(p)
	if err != nil {
		t.Fatal(err)
	}
	archiveKey, err := objects.Put(ctx, raw)
	if err != nil {
		t.Fatal(err)
	}
	d, _ := p.Manifest()
	rights, _ := json.Marshal(d.Package.Rights)
	lock, _ := p.ExactLock().Digest()
	identity := string(p.ArtifactIdentity().Digest())
	a := Artifact{Identity: identity, PackageID: string(d.Package.PackageID), Version: string(d.Package.Version), ContentHash: string(p.ContentHash()), LockHash: string(lock), SourceArchiveHash: object.Hash(raw), ArchiveKey: archiveKey, Manifest: p.ManifestBytes(), IdentityBytes: p.ArtifactBytes(), LockBytes: p.LockBytes(), Rights: rights, PolicyDigest: object.Hash([]byte("policy")), ValidationDigest: object.Hash([]byte("validation")), Retention: "fixture"}
	for _, entry := range p.Entries() {
		key, err := objects.Put(ctx, entry.Bytes())
		if err != nil {
			t.Fatal(err)
		}
		a.Objects = append(a.Objects, ObjectRef{Path: entry.Path(), Key: key})
	}
	a.Objects = append(a.Objects, ObjectRef{Path: "@archive", Key: archiveKey})
	credential := Credential("fixture-reader-credential")
	access, err := NewAccess(map[Credential][]Membership{credential: {{Principal: "alice", Workspace: "a", Read: true}}})
	if err != nil {
		t.Fatal(err)
	}
	reader, err := NewReader(readRepository{artifact: a}, objects, access, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = reader.Load(ctx, credential, "a", identity); err != nil {
		t.Fatal(err)
	}
	wrongKey, err := objects.Put(ctx, []byte("valid immutable but unrelated bytes"))
	if err != nil {
		t.Fatal(err)
	}
	for name, mutate := range map[string]func(*Artifact){
		"missing-reference": func(a *Artifact) { a.Objects = a.Objects[1:] }, "wrong-content-key": func(a *Artifact) { a.Objects[0].Key = wrongKey },
		"duplicate-reference": func(a *Artifact) { a.Objects = append(a.Objects, a.Objects[0]) }, "wrong-path": func(a *Artifact) { a.Objects[0].Path = "content/other.txt" },
		"metadata-manifest": func(a *Artifact) { a.Manifest = []byte("wrong") }, "metadata-rights": func(a *Artifact) { a.Rights = []byte("wrong") },
	} {
		t.Run(name, func(t *testing.T) {
			copyA := a
			copyA.Objects = append([]ObjectRef(nil), a.Objects...)
			mutate(&copyA)
			reader, err := NewReader(readRepository{artifact: copyA}, objects, access, extension.DefaultSupport)
			if err != nil {
				t.Fatal(err)
			}
			if _, err = reader.Load(ctx, credential, "a", identity); err == nil {
				t.Fatal("unbound stored metadata/content accepted")
			}
		})
	}
}
