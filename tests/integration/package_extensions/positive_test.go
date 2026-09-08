// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package package_extensions_test

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/apps/creator-studio/creator"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/manifest"
)

func TestP01V1PackageWithoutExtensionsIsUnchanged(t *testing.T) {
	files := v1Files(t)
	files[archive.ManifestPath] = append(files[archive.ManifestPath], []byte("\n# preserve this v1 comment exactly\n")...)
	pkg := buildPackage(t, files)
	if len(pkg.Extensions()) != 0 {
		t.Fatalf("v1 extension count = %d, want 0", len(pkg.Extensions()))
	}
	if !bytes.Equal(pkg.ManifestBytes(), files[archive.ManifestPath]) {
		t.Fatal("v1 manifest bytes changed during source load")
	}
	reloaded, err := archive.Import(exportPackage(t, pkg), extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(reloaded.ManifestBytes(), files[archive.ManifestPath]) || len(reloaded.Extensions()) != 0 {
		t.Fatal("v1 manifest or zero-extension behavior changed after export/reload")
	}
}

func TestP02OneThirdPartyNamespaceLoadsEditable(t *testing.T) {
	pkg := buildPackage(t, v2Files(t, extensionFixture{
		namespace: probeNamespace, contractVersion: 1,
	}))
	documents := pkg.Extensions()
	if len(documents) != 1 || documents[0].Descriptor.Namespace != probeNamespace || documents[0].Status != extension.Supported {
		t.Fatalf("extension documents = %#v", documents)
	}
	if got := string(documents[0].CanonicalPayload()); got != `{"count":1}` {
		t.Fatalf("canonical payload = %q", got)
	}
}

func TestP03MultipleThirdPartyNamespacesAreUniqueAndOrdered(t *testing.T) {
	pkg := buildPackage(t, v2Files(t,
		extensionFixture{namespace: "vendor.zeta.probe", contractVersion: 1},
		extensionFixture{namespace: "alpha.third.probe", contractVersion: 1, payload: []byte(`{"count":2}`)},
	))
	want := []string{"alpha.third.probe", "vendor.zeta.probe"}
	if got := extensionNamespaces(pkg.Extensions()); !reflect.DeepEqual(got, want) {
		t.Fatalf("extension order = %v, want %v", got, want)
	}
}

func TestP04RequiredSupportedExtensionValidatesEdits(t *testing.T) {
	pkg := buildPackage(t, v2Files(t, extensionFixture{
		namespace: probeNamespace, required: true, contractVersion: 1,
	}))
	documents := pkg.Extensions()
	if len(documents) != 1 || !documents[0].Descriptor.Required || documents[0].Status != extension.Supported {
		t.Fatalf("required supported document = %#v", documents)
	}
	edited, err := pkg.ReplaceExtension(probeNamespace, []byte(` { "count" : 9 } `))
	if err != nil {
		t.Fatal(err)
	}
	entry, exists := edited.Entry(probePayloadPath)
	if !exists || string(entry.Bytes()) != `{"count":9}` {
		t.Fatalf("edited payload = %q, exists = %v", entry.Bytes(), exists)
	}
}

func TestP05OptionalUnknownExtensionIsReadOnlyAndLossless(t *testing.T) {
	rawSchema := []byte("{ definitely-not-json\n")
	rawPayload := []byte("{ \"duplicate\" : 1, \"duplicate\" : 2 }\n")
	files := v2Files(t, extensionFixture{
		namespace: probeNamespace, contractVersion: 2, schema: rawSchema, payload: rawPayload,
	})
	pkg := buildPackage(t, files)
	documents := pkg.Extensions()
	if len(documents) != 1 || documents[0].Status != extension.ReadOnly || documents[0].ReadOnlyReason == "" {
		t.Fatalf("optional unknown document = %#v", documents)
	}
	if !bytes.Equal(documents[0].SchemaBytes(), rawSchema) || !bytes.Equal(documents[0].PayloadBytes(), rawPayload) || documents[0].CanonicalPayload() != nil {
		t.Fatal("optional unknown schema or payload was interpreted or changed")
	}
	reloaded, err := archive.Import(exportPackage(t, pkg), extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	reloadedDocument := reloaded.Extensions()[0]
	if reloadedDocument.Status != extension.ReadOnly || !bytes.Equal(reloadedDocument.SchemaBytes(), rawSchema) || !bytes.Equal(reloadedDocument.PayloadBytes(), rawPayload) {
		t.Fatal("optional unknown schema or payload was not preserved across reload")
	}
}

func TestP06ParseLoadExportReloadIsSemanticallyStable(t *testing.T) {
	pkg := buildPackage(t, v2Files(t, extensionFixture{
		namespace: probeNamespace, contractVersion: 1,
	}))
	reloaded, err := archive.Import(exportPackage(t, pkg), extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	before, after := pkg.Extensions(), reloaded.Extensions()
	if len(before) != 1 || len(after) != 1 || !reflect.DeepEqual(before[0].Descriptor, after[0].Descriptor) ||
		before[0].Status != after[0].Status || !bytes.Equal(before[0].CanonicalPayload(), after[0].CanonicalPayload()) ||
		pkg.ContentHash() != reloaded.ContentHash() || !bytes.Equal(pkg.ManifestBytes(), reloaded.ManifestBytes()) {
		t.Fatalf("semantic model changed across reload: before=%#v after=%#v", before, after)
	}
}

func TestP07CanonicalJSONAndTOMLSerializationIsExact(t *testing.T) {
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["a","b"],"properties":{"a":{"type":"number"},"b":{"type":"array","items":{"type":"integer"}}}}`)
	pkg := buildPackage(t, v2Files(t, extensionFixture{
		namespace: probeNamespace, contractVersion: 1, schema: schema,
		payload: []byte(" { \"b\" : [3, 2, 1], \"a\" : 1.00e1 } \n"),
	}))
	payload, exists := pkg.Entry(probePayloadPath)
	if !exists || string(payload.Bytes()) != `{"a":10,"b":[3,2,1]}` {
		t.Fatalf("canonical JSON = %q", payload.Bytes())
	}
	document, err := manifest.Parse(pkg.ManifestBytes())
	if err != nil {
		t.Fatal(err)
	}
	canonical, err := manifest.CanonicalTOML(document)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(canonical, pkg.ManifestBytes()) || bytes.Contains(canonical, []byte("\r")) {
		t.Fatal("manifest is not exact canonical LF TOML")
	}
}

func TestP08PackageBuildAndRepackAreByteDeterministic(t *testing.T) {
	files := v2Files(t, extensionFixture{namespace: probeNamespace, contractVersion: 1})
	firstPackage := buildPackage(t, cloneFiles(files))
	secondPackage := buildPackage(t, reverseInsertionOrder(files))
	first, second := exportPackage(t, firstPackage), exportPackage(t, secondPackage)
	if !bytes.Equal(first.Bytes(), second.Bytes()) || first.Hash() != second.Hash() || firstPackage.ContentHash() != secondPackage.ContentHash() {
		t.Fatal("equivalent source insertion orders produced different archives")
	}
	reloaded, err := archive.Import(first, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	repacked := exportPackage(t, reloaded)
	if !bytes.Equal(first.Bytes(), repacked.Bytes()) || first.Hash() != repacked.Hash() {
		t.Fatal("canonical archive changed during deterministic repack")
	}
}

func TestP09CreatorServiceImportsAndExportsCanonicalArchive(t *testing.T) {
	pkg := buildPackage(t, v2Files(t, extensionFixture{namespace: probeNamespace, contractVersion: 1}))
	sourceSnapshot := exportPackage(t, pkg)
	directory := t.TempDir()
	source := filepath.Join(directory, "source.trpgpkg")
	if err := os.WriteFile(source, sourceSnapshot.Bytes(), 0o600); err != nil {
		t.Fatal(err)
	}
	service := creator.NewService()
	inspection, err := service.ImportArchive(source)
	if err != nil {
		t.Fatal(err)
	}
	if inspection.ConflictToken != sourceSnapshot.Hash().String() || len(inspection.Extensions) != 1 || !inspection.Extensions[0].Editable {
		t.Fatalf("Creator inspection = %#v", inspection)
	}
	output := filepath.Join(directory, "output.trpgpkg")
	exported, err := service.Export(inspection.ConflictToken, output)
	if err != nil {
		t.Fatal(err)
	}
	if exported.Path != output || exported.ArchiveHash != exported.ConflictToken || exported.ContentHash != pkg.ContentHash().String() {
		t.Fatalf("Creator export = %#v", exported)
	}
	reloaded, err := archive.ImportFile(output, extension.DefaultSupport)
	if err != nil || reloaded.ContentHash() != pkg.ContentHash() {
		t.Fatalf("Creator output reload = %v, err = %v", reloaded, err)
	}
}

func TestP11RawSchemaDigestBindsExactBytes(t *testing.T) {
	rawSchema := append([]byte(" \n"), objectSchema...)
	files := v2Files(t, extensionFixture{
		namespace: probeNamespace, contractVersion: 1, schema: rawSchema,
	})
	pkg := buildPackage(t, files)
	document := pkg.Extensions()[0]
	digest := sha256.Sum256(rawSchema)
	want := "sha256:" + hex.EncodeToString(digest[:])
	if document.Descriptor.SchemaSHA256 != want || !bytes.Equal(document.SchemaBytes(), rawSchema) {
		t.Fatal("descriptor digest does not bind the exact raw schema bytes")
	}
	reloaded, err := archive.Import(exportPackage(t, pkg), extension.DefaultSupport)
	if err != nil || !bytes.Equal(reloaded.Extensions()[0].SchemaBytes(), rawSchema) {
		t.Fatalf("raw schema bytes changed on reload: %v", err)
	}
}

func TestP12FirstAndThirdPartyPackagesUseTheSameGenericPath(t *testing.T) {
	type result struct {
		status    extension.Status
		canonical string
		validCode extension.ErrorCode
	}
	run := func(packageID, namespace string) result {
		files, lock := packageVariant(t, packageID, namespace)
		pkg := buildPackageWithLock(t, files, lock)
		document := pkg.Extensions()[0]
		_, err := pkg.ReplaceExtension(namespace, []byte(`{"count":"wrong"}`))
		contract, ok := err.(*extension.ContractError)
		if !ok {
			t.Fatalf("%s validation error = %T %v", packageID, err, err)
		}
		return result{status: document.Status, canonical: string(document.CanonicalPayload()), validCode: contract.Code}
	}
	first := run("first.party/probe", "first.party.probe")
	third := run("outside.vendor/probe", "third.party.probe")
	if !reflect.DeepEqual(first, third) || first.status != extension.Supported || first.validCode != extension.ErrSchemaValidation {
		t.Fatalf("generic path results differ: first=%#v third=%#v", first, third)
	}
}
