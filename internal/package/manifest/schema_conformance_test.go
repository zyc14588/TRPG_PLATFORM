// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package manifest_test

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"strings"
	"testing"

	"github.com/santhosh-tekuri/jsonschema/v6"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

const packageSchemaBaseID = "https://github.com/zyc14588/TRPG_PLATFORM/schemas/package/"

var packageSchemaFiles = []string{
	"manifest-v1.schema.json",
	"lock-v1.schema.json",
	"artifact-identity-v1.schema.json",
}

func compiledPackageSchemas(t *testing.T) map[string]*jsonschema.Schema {
	t.Helper()
	compiler := jsonschema.NewCompiler()
	compiler.DefaultDraft(jsonschema.Draft2020)
	for _, filename := range packageSchemaFiles {
		data, err := os.ReadFile(filepath.Join("..", "..", "..", "schemas", "package", filename))
		if err != nil {
			t.Fatal(err)
		}
		document, err := jsonschema.UnmarshalJSON(bytes.NewReader(data))
		if err != nil {
			t.Fatalf("decode %s as JSON: %v", filename, err)
		}
		if err := compiler.AddResource(packageSchemaBaseID+filename, document); err != nil {
			t.Fatalf("add %s to Draft 2020-12 compiler: %v", filename, err)
		}
	}
	compiled := make(map[string]*jsonschema.Schema, len(packageSchemaFiles))
	for _, filename := range packageSchemaFiles {
		schema, err := compiler.Compile(packageSchemaBaseID + filename)
		if err != nil {
			t.Fatalf("compile %s as Draft 2020-12: %v", filename, err)
		}
		compiled[filename] = schema
	}
	return compiled
}

func TestDraft202012SchemaConformanceCorpus(t *testing.T) {
	t.Parallel()
	schemas := compiledPackageSchemas(t)
	corpora := []struct {
		directory string
		wantValid bool
	}{
		{directory: "valid", wantValid: true},
		{directory: "invalid", wantValid: false},
	}
	for _, corpus := range corpora {
		paths, err := filepath.Glob(filepath.Join("..", "testdata", "schema", corpus.directory, "*.json"))
		if err != nil {
			t.Fatal(err)
		}
		if len(paths) == 0 {
			t.Fatalf("schema %s corpus is empty", corpus.directory)
		}
		sort.Strings(paths)
		for _, path := range paths {
			path := path
			t.Run(filepath.Join(corpus.directory, filepath.Base(path)), func(t *testing.T) {
				t.Parallel()
				data, err := os.ReadFile(path)
				if err != nil {
					t.Fatal(err)
				}
				instance, err := jsonschema.UnmarshalJSON(bytes.NewReader(data))
				if err != nil {
					t.Fatalf("fixture is not JSON: %v", err)
				}
				filename := schemaFilenameForFixture(t, filepath.Base(path))
				err = schemas[filename].Validate(instance)
				if corpus.wantValid && err != nil {
					t.Fatalf("valid fixture failed %s: %v", filename, err)
				}
				if !corpus.wantValid && err == nil {
					t.Fatalf("invalid fixture passed %s", filename)
				}
			})
		}
	}
}

func schemaFilenameForFixture(t *testing.T, fixtureName string) string {
	t.Helper()
	switch {
	case strings.HasPrefix(fixtureName, "manifest-"):
		return "manifest-v1.schema.json"
	case strings.HasPrefix(fixtureName, "lock-") || fixtureName == "lock.json":
		return "lock-v1.schema.json"
	case strings.HasPrefix(fixtureName, "artifact-"):
		return "artifact-identity-v1.schema.json"
	default:
		t.Fatalf("fixture %q has no schema mapping", fixtureName)
		return ""
	}
}

func TestSchemaEnumsExactlyMatchGoRegistries(t *testing.T) {
	t.Parallel()
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "schemas", "package", "manifest-v1.schema.json"))
	if err != nil {
		t.Fatal(err)
	}
	var schema map[string]any
	if err := json.Unmarshal(data, &schema); err != nil {
		t.Fatal(err)
	}
	definitions := schema["$defs"].(map[string]any)

	wantCapabilities := make([]string, len(capability.RegisteredNames()))
	for index, name := range capability.RegisteredNames() {
		wantCapabilities[index] = string(name)
	}
	gotCapabilities := stringEnum(t, definitions["capabilityName"].(map[string]any)["enum"])
	if !reflect.DeepEqual(gotCapabilities, wantCapabilities) {
		t.Fatalf("Schema capabilities = %#v, Go registry = %#v", gotCapabilities, wantCapabilities)
	}

	packageDefinition := definitions["packageManifest"].(map[string]any)
	properties := packageDefinition["properties"].(map[string]any)
	gotKinds := stringEnum(t, properties["package_kind"].(map[string]any)["enum"])
	wantKinds := make([]string, len(model.PackageKinds()))
	for index, kind := range model.PackageKinds() {
		wantKinds[index] = string(kind)
	}
	if !reflect.DeepEqual(gotKinds, wantKinds) {
		t.Fatalf("Schema package kinds = %#v, Go registry = %#v", gotKinds, wantKinds)
	}
	artifactSchema := readJSONObject(t, filepath.Join("..", "..", "..", "schemas", "package", "artifact-identity-v1.schema.json"))
	artifactProperties := artifactSchema["properties"].(map[string]any)
	artifactKinds := stringEnum(t, artifactProperties["package_kind"].(map[string]any)["enum"])
	if !reflect.DeepEqual(artifactKinds, wantKinds) {
		t.Fatalf("Artifact Schema package kinds = %#v, Go registry = %#v", artifactKinds, wantKinds)
	}
}

func stringEnum(t *testing.T, value any) []string {
	t.Helper()
	items, ok := value.([]any)
	if !ok {
		t.Fatalf("enum = %#v", value)
	}
	result := make([]string, len(items))
	for index, item := range items {
		text, ok := item.(string)
		if !ok {
			t.Fatalf("enum item = %#v", item)
		}
		result[index] = text
	}
	return result
}

func TestIdentitySchemasMatchGoCanonicalValidation(t *testing.T) {
	t.Parallel()
	schemas := compiledPackageSchemas(t)
	tests := []struct {
		name     string
		field    string
		values   []string
		goAccept func(string) bool
	}{
		{
			name: "package_id", field: "package_id",
			values: []string{"example.rules/schema-fixture", "a/b", "", "Example/name", "example", "example/../name"},
			goAccept: func(value string) bool {
				_, err := model.ParsePackageID(value)
				return err == nil
			},
		},
		{
			name: "semantic version", field: "version",
			values: []string{"0.0.0", "1.2.3", "1.2.3-alpha.1+build.7", "v1.2.3", "1.2", "01.2.3", "1.2.3-01", "1.2.3+"},
			goAccept: func(value string) bool {
				_, err := model.ParseVersion(value)
				return err == nil
			},
		},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			for _, value := range test.values {
				value := value
				t.Run(value, func(t *testing.T) {
					t.Parallel()
					wantValid := test.goAccept(value)
					for _, target := range []struct {
						schema  string
						fixture string
					}{
						{schema: "manifest-v1.schema.json", fixture: "manifest-package.json"},
						{schema: "artifact-identity-v1.schema.json", fixture: "artifact-identity.json"},
					} {
						instance := readJSONObject(t, filepath.Join("..", "testdata", "schema", "valid", target.fixture))
						instance[test.field] = value
						gotValid := schemas[target.schema].Validate(instance) == nil
						if gotValid != wantValid {
							t.Errorf("%s accepts %s=%q: %t, Go accepts: %t", target.schema, test.field, value, gotValid, wantValid)
						}
					}
				})
			}
		})
	}
}

func readJSONObject(t *testing.T, path string) map[string]any {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var result map[string]any
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatal(err)
	}
	return result
}
