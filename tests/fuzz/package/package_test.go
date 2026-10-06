// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package package_test

import (
	"bytes"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/manifest"
	"github.com/zyc14588/TRPG_PLATFORM/tests/fuzz/support"
)

func FuzzPackageArchive(f *testing.F) {
	f.Add(support.Archive())
	f.Add([]byte("PK malformed bounded archive"))
	f.Fuzz(func(t *testing.T, raw []byte) {
		if len(raw) > support.MaxInput {
			return
		}
		before := append([]byte(nil), raw...)
		p, err := archive.ImportBytes(raw, extension.DefaultSupport)
		if !bytes.Equal(raw, before) {
			t.Fatal("archive importer mutated input")
		}
		if err != nil {
			return
		}
		s, err := p.Export()
		if err != nil {
			t.Fatal("accepted archive cannot export", err)
		}
		round, err := archive.Import(s, extension.DefaultSupport)
		if err != nil || round.ContentHash() != p.ContentHash() || round.ArtifactIdentity().Digest() != p.ArtifactIdentity().Digest() || string(round.LockBytes()) != string(p.LockBytes()) {
			t.Fatal("accepted archive lost exact content/lock/identity")
		}
		if copy := s.Bytes(); len(copy) > 0 {
			copy[0] ^= 255
			if bytes.Equal(copy, s.Bytes()) {
				t.Fatal("snapshot is not immutable")
			}
		}
	})
}
func FuzzPackageManifestTOML(f *testing.F) {
	f.Add(support.Manifest())
	f.Add([]byte("schema_version=1\nschema_version=2\n"))
	f.Fuzz(func(t *testing.T, raw []byte) {
		if len(raw) > support.MaxInput {
			return
		}
		before := append([]byte(nil), raw...)
		doc, err := manifest.Parse(raw)
		if !bytes.Equal(raw, before) {
			t.Fatal("manifest mutated input")
		}
		if err != nil {
			return
		}
		if (doc.Package == nil) == (doc.Bundle == nil) {
			t.Fatal("accepted manifest has ambiguous variants")
		}
		if doc.Package != nil && (len(doc.Package.Rights.Authors) == 0 || doc.Package.Rights.LicenseExpression == "") {
			t.Fatal("accepted manifest lacks rights")
		}
	})
}
func FuzzPackageSchema(f *testing.F) {
	f.Add([]byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"boolean"}`))
	f.Add([]byte(`{"$ref":"https://example.invalid/remote.schema.json"}`))
	f.Fuzz(func(t *testing.T, raw []byte) {
		if len(raw) > 16<<10 {
			return
		}
		before := append([]byte(nil), raw...)
		schema, err := support.WithSchema(raw)
		if !bytes.Equal(raw, before) {
			t.Fatal("schema loader mutated input")
		}
		if err != nil {
			return
		}
		if schema.Digest() != checkpoint.Hash(raw) || schema.Validate(checkpoint.Bool(true)) != nil {
			t.Fatal("accepted bound schema lost its digest or seed")
		}
		first, second := schema.Validate(checkpoint.Bool(false)), schema.Validate(checkpoint.Bool(false))
		if (first == nil) != (second == nil) {
			t.Fatal("immutable schema became nondeterministic")
		}
	})
}
