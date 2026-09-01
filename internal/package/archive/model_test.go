// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package archive

import (
	"archive/zip"
	"bytes"
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"io"
	"os"
	"sort"
	"strconv"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/manifest"
)

func TestSourceBuildRebindsRootAndImportNeverDoes(t *testing.T) {
	t.Parallel()
	files := v1Files(t)
	lock := fixtureLock(t)
	pkg, err := FromFiles(files, lock, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	root, exists := pkg.ExactLock().Package(pkg.ExactLock().Root())
	if !exists || root.ContentHash != pkg.ContentHash() {
		t.Fatalf("rebound root = %#v, content hash = %s", root, pkg.ContentHash())
	}
	originalRoot, _ := lock.Package(lock.Root())
	if originalRoot.ContentHash == pkg.ContentHash() {
		t.Fatal("test source lock unexpectedly already matched content")
	}

	snapshot, err := pkg.Export()
	if err != nil {
		t.Fatal(err)
	}
	archived, err := readSnapshot(snapshot)
	if err != nil {
		t.Fatal(err)
	}
	archived["content/raw.bin"] = []byte("tampered")
	forged := zipFileMap(t, archived)
	if _, err := ImportBytes(forged, extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "content hash") {
		t.Fatalf("strict imported-lock result = %v", err)
	}

	changed := cloneTestFiles(files)
	changed["package.lock.json"] = append(changed["package.lock.json"], '\n')
	second, err := FromFiles(changed, lock, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if second.ContentHash() == pkg.ContentHash() {
		t.Fatal("ordinary package.lock.json bytes did not enter the content hash")
	}
	root, _ = second.ExactLock().Package(second.ExactLock().Root())
	if root.ContentHash != second.ContentHash() {
		t.Fatal("second source build did not rebind root hash")
	}
}

func TestV1RawManifestAndDeterministicRepack(t *testing.T) {
	t.Parallel()
	files := v1Files(t)
	files[ManifestPath] = append(files[ManifestPath], []byte("\n# preserve-v1-raw\n")...)
	pkg, err := FromFiles(files, fixtureLock(t), extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(pkg.ManifestBytes(), files[ManifestPath]) || len(pkg.Extensions()) != 0 {
		t.Fatal("v1 raw manifest or zero-extension behavior changed")
	}
	if _, exists := pkg.SourceArchiveHash(); exists {
		t.Fatal("source project unexpectedly has an archive conflict token")
	}
	first, err := pkg.Export()
	if err != nil {
		t.Fatal(err)
	}
	second, err := pkg.Export()
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(first.Bytes(), second.Bytes()) || first.Hash() != second.Hash() {
		t.Fatal("identical v1 exports differ")
	}
	assertDeterministicZIPMetadata(t, first)
	reloaded, err := Import(first, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(reloaded.ManifestBytes(), files[ManifestPath]) || reloaded.ContentHash() != pkg.ContentHash() {
		t.Fatal("v1 import changed raw bytes or content identity")
	}
	if hash, exists := reloaded.SourceArchiveHash(); !exists || hash != first.Hash() {
		t.Fatalf("source archive hash = %s, %v", hash, exists)
	}
	repacked, err := reloaded.Export()
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(first.Bytes(), repacked.Bytes()) {
		t.Fatal("v1 deterministic repack differs")
	}
}

func TestV2CanonicalRoundtripAndValidatedEdit(t *testing.T) {
	t.Parallel()
	files := v2Files(t, false, 1)
	originalPayload := append([]byte(nil), files[v2PayloadPath]...)
	pkg, err := FromFiles(files, fixtureLock(t), extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	parsed, err := manifest.Parse(files[ManifestPath])
	if err != nil {
		t.Fatal(err)
	}
	canonicalManifest, err := manifest.CanonicalTOML(parsed)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(pkg.ManifestBytes(), canonicalManifest) {
		t.Fatal("v2 manifest was not canonicalized")
	}
	payload, exists := pkg.Entry(v2PayloadPath)
	if !exists || string(payload.Bytes()) != `{"count":1}` {
		t.Fatalf("canonical payload = %q", payload.Bytes())
	}
	files[v2PayloadPath][0] = 'x'
	if string(payload.Bytes()) != `{"count":1}` || string(originalPayload) == string(files[v2PayloadPath]) {
		t.Fatal("model retained caller-owned payload bytes")
	}

	snapshot, err := pkg.Export()
	if err != nil {
		t.Fatal(err)
	}
	reloaded, err := Import(snapshot, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if reloaded.ContentHash() != pkg.ContentHash() || len(reloaded.Extensions()) != 1 || reloaded.Extensions()[0].Status != extension.Supported {
		t.Fatal("v2 semantic reload changed the canonical model")
	}
	edited, err := reloaded.ReplaceExtension("third.party.probe", []byte(` { "count" : 2 } `))
	if err != nil {
		t.Fatal(err)
	}
	if edited.ContentHash() == reloaded.ContentHash() {
		t.Fatal("validated edit did not change content identity")
	}
	editedRoot, _ := edited.ExactLock().Package(edited.ExactLock().Root())
	if editedRoot.ContentHash != edited.ContentHash() {
		t.Fatal("validated edit did not rebind the exact lock root")
	}
	editedPayload, _ := edited.Entry(v2PayloadPath)
	oldPayload, _ := reloaded.Entry(v2PayloadPath)
	if string(editedPayload.Bytes()) != `{"count":2}` || string(oldPayload.Bytes()) != `{"count":1}` {
		t.Fatal("edit was not canonical or mutated the input model")
	}
	if _, err := reloaded.ReplaceExtension("third.party.probe", []byte(`{"count":"wrong"}`)); !extension.IsCode(err, extension.ErrSchemaValidation) {
		t.Fatalf("invalid edit result = %v", err)
	}
	editedSnapshot, err := edited.Export()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := Import(editedSnapshot, extension.DefaultSupport); err != nil {
		t.Fatalf("edited archive re-import: %v", err)
	}
}

func TestOptionalUnsupportedExtensionIsRawAndReadOnly(t *testing.T) {
	t.Parallel()
	files := v2Files(t, false, 2)
	files[v2PayloadPath] = []byte("{ \"duplicate\" : 1, \"duplicate\" : 2 }\n")
	pkg, err := FromFiles(files, fixtureLock(t), extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	documents := pkg.Extensions()
	if len(documents) != 1 || documents[0].Status != extension.ReadOnly || !bytes.Equal(documents[0].PayloadBytes(), files[v2PayloadPath]) {
		t.Fatal("optional unsupported bytes were not preserved read-only")
	}
	if _, err := pkg.ReplaceExtension("third.party.probe", []byte(`{}`)); !extension.IsCode(err, extension.ErrReadOnly) {
		t.Fatalf("read-only edit result = %v", err)
	}
	snapshot, err := pkg.Export()
	if err != nil {
		t.Fatal(err)
	}
	reloaded, err := Import(snapshot, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	payload, _ := reloaded.Entry(v2PayloadPath)
	if !bytes.Equal(payload.Bytes(), files[v2PayloadPath]) {
		t.Fatal("optional unsupported raw payload changed on repack")
	}

	required := v2Files(t, true, 2)
	if _, err := FromFiles(required, fixtureLock(t), extension.DefaultSupport); !extension.IsCode(err, extension.ErrRequiredUnsupported) {
		t.Fatalf("required unsupported result = %v", err)
	}
}

func TestOuterEnvelopeIsExactAndMETAINFIsReserved(t *testing.T) {
	t.Parallel()
	pkg, err := FromFiles(v1Files(t), fixtureLock(t), extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err := pkg.Export()
	if err != nil {
		t.Fatal(err)
	}
	base, err := readSnapshot(snapshot)
	if err != nil {
		t.Fatal(err)
	}

	noncanonicalLock := cloneTestFiles(base)
	noncanonicalLock[PlatformLockPath] = append(noncanonicalLock[PlatformLockPath], '\n')
	if _, err := ImportBytes(zipFileMap(t, noncanonicalLock), extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "not exact canonical") {
		t.Fatalf("noncanonical lock result = %v", err)
	}
	tamperedArtifact := cloneTestFiles(base)
	tamperedArtifact[ArtifactPath] = append(tamperedArtifact[ArtifactPath], '\n')
	if _, err := ImportBytes(zipFileMap(t, tamperedArtifact), extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "does not match") {
		t.Fatalf("tampered artifact result = %v", err)
	}
	extraEnvelope := cloneTestFiles(base)
	extraEnvelope["META-INF/extra.json"] = []byte(`{}`)
	if _, err := ImportBytes(zipFileMap(t, extraEnvelope), extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "platform-owned META-INF") {
		t.Fatalf("extra envelope result = %v", err)
	}
	project := v1Files(t)
	project[PlatformLockPath] = []byte(`{}`)
	if _, err := FromFiles(project, fixtureLock(t), extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "platform-owned META-INF") {
		t.Fatalf("project META-INF result = %v", err)
	}
}

func TestContentHashUsesLengthFraming(t *testing.T) {
	t.Parallel()
	entries := []Entry{{path: "a", data: []byte("bc")}, {path: "ab", data: []byte("c")}}
	got, err := computeContentHash(entries)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.New()
	var pathLength [4]byte
	var contentLength [8]byte
	for _, entry := range entries {
		binary.BigEndian.PutUint32(pathLength[:], uint32(len(entry.path)))
		binary.BigEndian.PutUint64(contentLength[:], uint64(len(entry.data)))
		digest.Write(pathLength[:])
		digest.Write([]byte(entry.path))
		digest.Write(contentLength[:])
		digest.Write(entry.data)
	}
	want := "sha256:" + hex.EncodeToString(digest.Sum(nil))
	if got.String() != want {
		t.Fatalf("content hash = %s, want %s", got, want)
	}
}

const (
	v2SchemaPath  = "extensions/third.party.probe/value.schema.json"
	v2PayloadPath = "extensions/third.party.probe/value.json"
)

func v1Files(t *testing.T) map[string][]byte {
	t.Helper()
	return map[string][]byte{
		ManifestPath:        fixtureBytes(t, "package.toml"),
		"package.lock.json": fixtureBytes(t, "package.lock.json"),
		"content/raw.bin":   []byte{0, 1, 2, 3},
	}
}

func v2Files(t *testing.T, required bool, contractVersion int) map[string][]byte {
	t.Helper()
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["count"],"properties":{"count":{"type":"integer"}}}`)
	digest := sha256.Sum256(schema)
	manifestText := strings.Replace(string(fixtureBytes(t, "package.toml")), "schema_version = 1", "schema_version = 2", 1)
	manifestText += "\n[[extensions]]\n" +
		"namespace = \"third.party.probe\"\n" +
		"required = " + map[bool]string{true: "true", false: "false"}[required] + "\n" +
		"contract_version = " + strconv.Itoa(contractVersion) + "\n" +
		"schema_path = \"" + v2SchemaPath + "\"\n" +
		"schema_sha256 = \"sha256:" + hex.EncodeToString(digest[:]) + "\"\n" +
		"payload_path = \"" + v2PayloadPath + "\"\n" +
		"host_api_major = 1\n" +
		"host_api_min_minor = 0\n" +
		"host_api_max_minor = 0\n"
	return map[string][]byte{
		ManifestPath:        []byte(manifestText),
		"package.lock.json": fixtureBytes(t, "package.lock.json"),
		v2SchemaPath:        schema,
		v2PayloadPath:       []byte(" { \"count\" : 1 } \n"),
	}
}

func fixtureLock(t *testing.T) dependency.ExactLock {
	t.Helper()
	lock, err := dependency.ParseExactLock(fixtureBytes(t, "package.lock.json"))
	if err != nil {
		t.Fatal(err)
	}
	return lock
}

func fixtureBytes(t *testing.T, name string) []byte {
	t.Helper()
	data, err := os.ReadFile("../testdata/" + name)
	if err != nil {
		t.Fatal(err)
	}
	return data
}

func cloneTestFiles(files map[string][]byte) map[string][]byte {
	result := make(map[string][]byte, len(files))
	for name, data := range files {
		result[name] = append([]byte(nil), data...)
	}
	return result
}

func zipFileMap(t *testing.T, files map[string][]byte) []byte {
	t.Helper()
	names := make([]string, 0, len(files))
	for name := range files {
		names = append(names, name)
	}
	sort.Strings(names)
	var output bytes.Buffer
	writer := zip.NewWriter(&output)
	for _, name := range names {
		file, err := writer.CreateHeader(&zip.FileHeader{Name: name, Method: zip.Store})
		if err != nil {
			t.Fatal(err)
		}
		if _, err := file.Write(files[name]); err != nil {
			t.Fatal(err)
		}
	}
	if err := writer.Close(); err != nil {
		t.Fatal(err)
	}
	return output.Bytes()
}

func assertDeterministicZIPMetadata(t *testing.T, snapshot Snapshot) {
	t.Helper()
	reader, err := zip.NewReader(bytes.NewReader(snapshot.data), int64(len(snapshot.data)))
	if err != nil {
		t.Fatal(err)
	}
	previous := ""
	for _, file := range reader.File {
		if previous != "" && previous >= file.Name {
			t.Fatalf("archive order %q then %q", previous, file.Name)
		}
		previous = file.Name
		if file.Method != zip.Store || file.Mode().Perm() != 0644 || !file.Mode().IsRegular() || file.ModifiedDate != 1<<5|1 || file.ModifiedTime != 0 || len(file.Extra) != 0 || file.Comment != "" {
			t.Fatalf("noncanonical ZIP metadata for %q: %#v", file.Name, file.FileHeader)
		}
		stream, err := file.Open()
		if err != nil {
			t.Fatal(err)
		}
		if _, err := io.Copy(io.Discard, stream); err != nil {
			t.Fatal(err)
		}
		if err := stream.Close(); err != nil {
			t.Fatal(err)
		}
	}
}

func TestArchiveAccessorsAreDefensive(t *testing.T) {
	t.Parallel()
	pkg, err := FromFiles(v2Files(t, false, 1), fixtureLock(t), extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	manifestBytes := pkg.ManifestBytes()
	manifestBytes[0] ^= 0xff
	if bytes.Equal(manifestBytes, pkg.ManifestBytes()) {
		t.Fatal("manifest bytes accessor aliases model")
	}
	document, err := pkg.Manifest()
	if err != nil {
		t.Fatal(err)
	}
	document.Package.DisplayName = "mutated"
	second, _ := pkg.Manifest()
	if second.Package.DisplayName == "mutated" {
		t.Fatal("manifest document accessor aliases model")
	}
	extensions := pkg.Extensions()
	extensions[0].Descriptor.Namespace = "mutated.namespace.value"
	if pkg.Extensions()[0].Descriptor.Namespace == "mutated.namespace.value" {
		t.Fatal("extension accessor aliases model")
	}
	entries := pkg.Entries()
	entryBytes := entries[0].Bytes()
	entryBytes[0] ^= 0xff
	actual, _ := pkg.Entry(entries[0].Path())
	if bytes.Equal(entryBytes, actual.Bytes()) {
		t.Fatal("entry bytes accessor aliases model")
	}
	artifact := pkg.ArtifactBytes()
	artifact[0] ^= 0xff
	if bytes.Equal(artifact, pkg.ArtifactBytes()) {
		t.Fatal("artifact accessor aliases model")
	}
}

func TestUndeclaredExtensionEntryIsRejected(t *testing.T) {
	t.Parallel()
	files := v2Files(t, false, 1)
	files["extensions/third.party.probe/execute.lua"] = []byte("return true")
	if _, err := FromFiles(files, fixtureLock(t), extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "not declared") {
		t.Fatalf("undeclared extension code result = %v", err)
	}
	if _, err := FromFiles(v1FilesWithExtension(t), fixtureLock(t), extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "not declared") {
		t.Fatalf("v1 undeclared extension result = %v", err)
	}
}

func v1FilesWithExtension(t *testing.T) map[string][]byte {
	files := v1Files(t)
	files["extensions/third.party.probe/value.json"] = []byte(`{}`)
	return files
}
