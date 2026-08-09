// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package manifest_test

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/manifest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

func fixture(t *testing.T, name string) []byte {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("..", "testdata", name))
	if err != nil {
		t.Fatal(err)
	}
	return data
}

func TestParseVersionedPackageManifest(t *testing.T) {
	t.Parallel()
	document, err := manifest.Parse(fixture(t, "package.toml"))
	if err != nil {
		t.Fatal(err)
	}
	if document.ArtifactType != model.ArtifactTypePackage || document.Package == nil || document.Bundle != nil {
		t.Fatalf("package variant = %#v", document)
	}
	if document.Package.SchemaVersion != manifest.SchemaVersion || document.Package.PackageKind != model.PackageKindGameSystem {
		t.Fatalf("package = %#v", document.Package)
	}
	if !document.CanStartSession() {
		t.Fatal("game-system manifest cannot start Session")
	}
	if got := len(document.Package.Dependencies); got != 1 {
		t.Fatalf("dependencies = %d", got)
	}
}

func TestParseBundleAsDistributionContainer(t *testing.T) {
	t.Parallel()
	document, err := manifest.Parse(fixture(t, "bundle.toml"))
	if err != nil {
		t.Fatal(err)
	}
	if document.ArtifactType != model.ArtifactTypeBundle || document.Bundle == nil || document.Package != nil {
		t.Fatalf("bundle variant = %#v", document)
	}
	if document.CanStartSession() {
		t.Fatal("Bundle can start a Session")
	}
	if got := len(document.Bundle.Artifacts); got != 2 {
		t.Fatalf("bundle artifacts = %d", got)
	}
}

func TestLibraryManifestCannotStartSession(t *testing.T) {
	t.Parallel()
	data := strings.Replace(string(fixture(t, "package.toml")), `package_kind = "game-system"`, `package_kind = "library"`, 1)
	document, err := manifest.Parse([]byte(data))
	if err != nil {
		t.Fatal(err)
	}
	if document.CanStartSession() {
		t.Fatal("library manifest can start a Session")
	}
}

func TestManifestFailsClosed(t *testing.T) {
	t.Parallel()
	base := string(fixture(t, "package.toml"))
	tests := []struct {
		name string
		data string
	}{
		{name: "unknown field", data: base + "\nunknown = true\n"},
		{name: "bundle runtime package kind", data: strings.Replace(base, `package_kind = "game-system"`, `package_kind = "bundle"`, 1)},
		{name: "unsupported schema", data: strings.Replace(base, "schema_version = 1", "schema_version = 2", 1)},
		{name: "unknown required capability", data: strings.Replace(base, `"host.event"`, `"host.unknown"`, 1)},
		{name: "unknown optional capability", data: strings.Replace(base, `name = "host.log"`, `name = "host.network"`, 1)},
		{name: "missing rights", data: strings.Replace(base, `license_expression = "LicenseRef-Example-Private"`, `license_expression = ""`, 1)},
		{name: "missing optional fallback", data: strings.Replace(base, `fallback = "continue without package log records"`, `fallback = ""`, 1)},
		{name: "partial runtime", data: strings.Replace(base, `lua_profile = "platform-lua-5.5-p1"`, `lua_profile = ""`, 1)},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			if _, err := manifest.Parse([]byte(test.data)); err == nil {
				t.Fatal("invalid manifest succeeded")
			}
		})
	}
}

func TestCanonicalEntrypointPathIsPlatformNeutral(t *testing.T) {
	t.Parallel()
	document, err := manifest.Parse(fixture(t, "package.toml"))
	if err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name  string
		path  string
		valid bool
	}{
		{name: "canonical relative Lua source", path: "lua/main.lua", valid: true},
		{name: "absolute", path: "/main.lua"},
		{name: "parent traversal", path: "../main.lua"},
		{name: "nested parent traversal", path: "a/../../main.lua"},
		{name: "upper-case Windows drive", path: "C:/escape.lua"},
		{name: "lower-case Windows drive", path: "c:/escape.lua"},
		{name: "Windows drive with backslashes", path: `C:\escape.lua`},
		{name: "slash UNC", path: "//server/share/a.lua"},
		{name: "backslash UNC", path: `\\server\share\a.lua`},
		{name: "Windows device", path: `\\?\C:\a.lua`},
		{name: "Windows local device", path: `\\.\C:\a.lua`},
		{name: "NUL", path: "lua/\x00.lua"},
		{name: "C0 control", path: "lua/\x1f.lua"},
		{name: "C1 control", path: "lua/\u0085.lua"},
		{name: "leading dot segment", path: "./lua/main.lua"},
		{name: "empty segment", path: "lua//main.lua"},
		{name: "cleaning traversal", path: "lua/../main.lua"},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			value := *document.Package
			value.Entrypoint = test.path
			_, err := manifest.NormalizePackage(value)
			if got := err == nil; got != test.valid {
				t.Fatalf("NormalizePackage entrypoint %q valid=%t, want %t; error=%v", test.path, got, test.valid, err)
			}
		})
	}
}

func TestPackageSchemasAreVersionedValidJSON(t *testing.T) {
	t.Parallel()
	paths, err := filepath.Glob(filepath.Join("..", "..", "..", "schemas", "package", "*.schema.json"))
	if err != nil {
		t.Fatal(err)
	}
	if len(paths) != 3 {
		t.Fatalf("schema count = %d, want 3", len(paths))
	}
	for _, path := range paths {
		path := path
		t.Run(filepath.Base(path), func(t *testing.T) {
			t.Parallel()
			data, err := os.ReadFile(path)
			if err != nil {
				t.Fatal(err)
			}
			var schema map[string]any
			if err := json.Unmarshal(data, &schema); err != nil {
				t.Fatalf("schema JSON: %v", err)
			}
			if schema["$schema"] != "https://json-schema.org/draft/2020-12/schema" || schema["$id"] == "" {
				t.Fatalf("schema metadata = %#v", schema)
			}
			if filepath.Base(path) == "manifest-v1.schema.json" {
				assertSchemaSeparatesPackageAndBundle(t, schema)
			}
		})
	}
}

func assertSchemaSeparatesPackageAndBundle(t *testing.T, schema map[string]any) {
	t.Helper()
	definitions, ok := schema["$defs"].(map[string]any)
	if !ok {
		t.Fatal("manifest schema has no $defs object")
	}
	packageDefinition, ok := definitions["packageManifest"].(map[string]any)
	if !ok {
		t.Fatal("manifest schema has no packageManifest definition")
	}
	bundleDefinition, ok := definitions["bundleManifest"].(map[string]any)
	if !ok {
		t.Fatal("manifest schema has no bundleManifest definition")
	}
	packageProperties := packageDefinition["properties"].(map[string]any)
	bundleProperties := bundleDefinition["properties"].(map[string]any)
	if packageProperties["artifact_type"].(map[string]any)["const"] != "package" || bundleProperties["artifact_type"].(map[string]any)["const"] != "bundle" {
		t.Fatal("manifest variants do not use distinct artifact_type constants")
	}
	kindValues := packageProperties["package_kind"].(map[string]any)["enum"].([]any)
	if len(kindValues) != 5 {
		t.Fatalf("runtime package kind count = %d", len(kindValues))
	}
	for _, value := range kindValues {
		if value == "bundle" {
			t.Fatal("Bundle appears in runtime package_kind enum")
		}
	}
}
