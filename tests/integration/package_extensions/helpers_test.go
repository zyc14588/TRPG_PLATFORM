// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package package_extensions_test

import (
	"archive/zip"
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
)

const (
	probeNamespace   = "third.party.probe"
	probeSchemaPath  = "extensions/third.party.probe/value.schema.json"
	probePayloadPath = "extensions/third.party.probe/value.json"
)

var objectSchema = []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["count"],"properties":{"count":{"type":"integer","minimum":0}}}`)

type extensionFixture struct {
	namespace       string
	required        bool
	contractVersion uint32
	schema          []byte
	payload         []byte
}

func v1Files(t *testing.T) map[string][]byte {
	t.Helper()
	return map[string][]byte{
		archive.ManifestPath: fixtureBytes(t, "package.toml"),
		"package.lock.json":  fixtureBytes(t, "package.lock.json"),
		"content/raw.bin":    {0, 1, 2, 3},
	}
}

func v2Files(t *testing.T, fixtures ...extensionFixture) map[string][]byte {
	t.Helper()
	manifestText := strings.Replace(string(fixtureBytes(t, "package.toml")), "schema_version = 1", "schema_version = 2", 1)
	files := map[string][]byte{
		"package.lock.json": fixtureBytes(t, "package.lock.json"),
		"content/raw.bin":   {0, 1, 2, 3},
	}
	for _, fixture := range fixtures {
		schema := fixture.schema
		if schema == nil {
			schema = objectSchema
		}
		payload := fixture.payload
		if payload == nil {
			payload = []byte(" { \"count\" : 1 } \n")
		}
		schemaPath := "extensions/" + fixture.namespace + "/value.schema.json"
		payloadPath := "extensions/" + fixture.namespace + "/value.json"
		digest := sha256.Sum256(schema)
		manifestText += fmt.Sprintf("\n[[extensions]]\nnamespace = %q\nrequired = %t\ncontract_version = %d\nschema_path = %q\nschema_sha256 = %q\npayload_path = %q\nhost_api_major = 1\nhost_api_min_minor = 0\nhost_api_max_minor = 0\n",
			fixture.namespace, fixture.required, fixture.contractVersion, schemaPath,
			"sha256:"+hex.EncodeToString(digest[:]), payloadPath)
		files[schemaPath] = append([]byte(nil), schema...)
		files[payloadPath] = append([]byte(nil), payload...)
	}
	files[archive.ManifestPath] = []byte(manifestText)
	return files
}

func fixtureBytes(t *testing.T, name string) []byte {
	t.Helper()
	path := filepath.Join(repositoryRoot(t), "internal", "package", "testdata", name)
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return data
}

func repositoryRoot(t *testing.T) string {
	t.Helper()
	_, source, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("locate integration fixture source")
	}
	return filepath.Clean(filepath.Join(filepath.Dir(source), "..", "..", ".."))
}

func fixtureLock(t *testing.T) dependency.ExactLock {
	t.Helper()
	lock, err := dependency.ParseExactLock(fixtureBytes(t, "package.lock.json"))
	if err != nil {
		t.Fatal(err)
	}
	return lock
}

func buildPackage(t *testing.T, files map[string][]byte) *archive.Package {
	t.Helper()
	return buildPackageWithLock(t, files, fixtureLock(t))
}

func buildPackageWithLock(t *testing.T, files map[string][]byte, lock dependency.ExactLock) *archive.Package {
	t.Helper()
	pkg, err := archive.FromFiles(files, lock, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	return pkg
}

func exportPackage(t *testing.T, pkg *archive.Package) archive.Snapshot {
	t.Helper()
	snapshot, err := pkg.Export()
	if err != nil {
		t.Fatal(err)
	}
	return snapshot
}

func extensionNamespaces(documents []extension.Document) []string {
	result := make([]string, 0, len(documents))
	for _, document := range documents {
		result = append(result, document.Descriptor.Namespace)
	}
	return result
}

func sortedKeys(files map[string][]byte) []string {
	result := make([]string, 0, len(files))
	for name := range files {
		result = append(result, name)
	}
	sort.Strings(result)
	return result
}

func cloneFiles(files map[string][]byte) map[string][]byte {
	result := make(map[string][]byte, len(files))
	for name, data := range files {
		result[name] = append([]byte(nil), data...)
	}
	return result
}

func reverseInsertionOrder(files map[string][]byte) map[string][]byte {
	names := sortedKeys(files)
	result := make(map[string][]byte, len(files))
	for index := len(names) - 1; index >= 0; index-- {
		result[names[index]] = append([]byte(nil), files[names[index]]...)
	}
	return result
}

func packageVariant(t *testing.T, packageID, namespace string) (map[string][]byte, dependency.ExactLock) {
	t.Helper()
	files := v2Files(t, extensionFixture{namespace: namespace, required: true, contractVersion: 1})
	const fixtureID = "example.rules/hidden-cards"
	files[archive.ManifestPath] = []byte(strings.ReplaceAll(string(files[archive.ManifestPath]), fixtureID, packageID))
	lockJSON := strings.ReplaceAll(string(fixtureBytes(t, "package.lock.json")), fixtureID, packageID)
	lock, err := dependency.ParseExactLock([]byte(lockJSON))
	if err != nil {
		t.Fatal(err)
	}
	return files, lock
}

func replaceManifest(t *testing.T, files map[string][]byte, old, replacement string) {
	t.Helper()
	manifestText := string(files[archive.ManifestPath])
	if strings.Count(manifestText, old) != 1 {
		t.Fatalf("manifest replacement source %q count = %d", old, strings.Count(manifestText, old))
	}
	files[archive.ManifestPath] = []byte(strings.Replace(manifestText, old, replacement, 1))
}

func requireExtensionCode(t *testing.T, err error, code extension.ErrorCode) *extension.ContractError {
	t.Helper()
	var contract *extension.ContractError
	if !errors.As(err, &contract) || contract.Code != code {
		t.Fatalf("extension error = %T %v, want %s", err, err, code)
	}
	return contract
}

func writeProjectFiles(t *testing.T, files map[string][]byte) string {
	t.Helper()
	root := t.TempDir()
	for name, data := range files {
		path := filepath.Join(root, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, data, 0o600); err != nil {
			t.Fatal(err)
		}
	}
	return root
}

func zipWithSymlink(t *testing.T, name, target string) []byte {
	t.Helper()
	var output bytes.Buffer
	writer := zip.NewWriter(&output)
	header := &zip.FileHeader{Name: name, Method: zip.Store}
	header.SetMode(os.ModeSymlink | fs.FileMode(0o777))
	entry, err := writer.CreateHeader(header)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := entry.Write([]byte(target)); err != nil {
		t.Fatal(err)
	}
	if err := writer.Close(); err != nil {
		t.Fatal(err)
	}
	return output.Bytes()
}

func packageEntries(pkg *archive.Package) map[string][]byte {
	result := make(map[string][]byte)
	for _, entry := range pkg.Entries() {
		result[entry.Path()] = entry.Bytes()
	}
	return result
}
