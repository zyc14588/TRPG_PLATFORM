// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package manifest_test

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/manifest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

var packageSchemaFiles = []string{
	"manifest-v1.schema.json",
	"lock-v1.schema.json",
	"artifact-identity-v1.schema.json",
}

func compiledPackageSchemas(t *testing.T) *manifest.SchemaConformance {
	t.Helper()
	var resources manifest.SchemaResources
	for _, filename := range packageSchemaFiles {
		data, err := os.ReadFile(filepath.Join("..", "..", "..", "schemas", "package", filename))
		if err != nil {
			t.Fatal(err)
		}
		switch filename {
		case string(manifest.ManifestSchemaDocument):
			resources.Manifest = data
		case string(manifest.LockSchemaDocument):
			resources.Lock = data
		case string(manifest.ArtifactIdentitySchemaDocument):
			resources.ArtifactIdentity = data
		default:
			t.Fatalf("unknown package schema %q", filename)
		}
	}
	validator, err := manifest.NewSchemaConformance(resources)
	if err != nil {
		t.Fatal(err)
	}
	return validator
}

func TestDraft202012SchemaConformanceCorpus(t *testing.T) {
	t.Parallel()
	validator := compiledPackageSchemas(t)
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
				document := schemaDocumentForFixture(t, filepath.Base(path))
				err = validator.Validate(document, data)
				if corpus.wantValid && err != nil {
					t.Fatalf("valid fixture failed %s public conformance: %v", document, err)
				}
				if !corpus.wantValid && err == nil {
					t.Fatalf("invalid fixture passed %s public conformance", document)
				}
			})
		}
	}
}

func schemaDocumentForFixture(t *testing.T, fixtureName string) manifest.SchemaDocument {
	t.Helper()
	switch {
	case strings.HasPrefix(fixtureName, "manifest-"):
		return manifest.ManifestSchemaDocument
	case strings.HasPrefix(fixtureName, "lock-") || fixtureName == "lock.json":
		return manifest.LockSchemaDocument
	case strings.HasPrefix(fixtureName, "artifact-"):
		return manifest.ArtifactIdentitySchemaDocument
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
	validator := compiledPackageSchemas(t)
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
						document manifest.SchemaDocument
						fixture  string
					}{
						{document: manifest.ManifestSchemaDocument, fixture: "manifest-package.json"},
						{document: manifest.ArtifactIdentitySchemaDocument, fixture: "artifact-identity.json"},
					} {
						instance := readCorpusJSONObject(t, "valid", target.fixture)
						setIdentityField(instance, target.document, test.field, value)
						gotValid := validator.Validate(target.document, marshalJSONObject(t, instance)) == nil
						if gotValid != wantValid {
							t.Errorf("%s accepts %s=%q: %t, Go accepts: %t", target.document, test.field, value, gotValid, wantValid)
						}
					}
				})
			}
		})
	}
}

func setIdentityField(instance map[string]any, document manifest.SchemaDocument, field, value string) {
	instance[field] = value
	if document != manifest.ArtifactIdentitySchemaDocument {
		return
	}
	lock := instance["dependency_lock"].(map[string]any)
	root := lock["packages"].([]any)[0].(map[string]any)
	switch field {
	case "package_id":
		lock["root"] = value
		root["package_id"] = value
	case "version":
		root["version"] = value
	}
}

func TestCanonicalRuleFixtureMap(t *testing.T) {
	t.Parallel()
	validator := compiledPackageSchemas(t)
	tests := []struct {
		rule     string
		document manifest.SchemaDocument
		positive string
		negative string
	}{
		{rule: "manifest display_name non-blank", document: manifest.ManifestSchemaDocument, positive: "manifest-package.json", negative: "manifest-blank-display-name.json"},
		{rule: "manifest canonical relative entrypoint", document: manifest.ManifestSchemaDocument, positive: "manifest-unique-dependencies.json", negative: "manifest-unsafe-entrypoint.json"},
		{rule: "manifest Host API ordered range", document: manifest.ManifestSchemaDocument, positive: "manifest-unique-dependencies.json", negative: "manifest-inverted-host-api-range.json"},
		{rule: "manifest dependency package identity uniqueness", document: manifest.ManifestSchemaDocument, positive: "manifest-unique-dependencies.json", negative: "manifest-duplicate-dependency.json"},
		{rule: "manifest package kind registry", document: manifest.ManifestSchemaDocument, positive: "manifest-package.json", negative: "manifest-invalid-package-kind.json"},
		{rule: "manifest capability registry", document: manifest.ManifestSchemaDocument, positive: "manifest-package.json", negative: "manifest-invalid-capability.json"},
		{rule: "manifest package identity", document: manifest.ManifestSchemaDocument, positive: "manifest-package.json", negative: "manifest-malformed-package-id.json"},
		{rule: "manifest semantic version", document: manifest.ManifestSchemaDocument, positive: "manifest-package.json", negative: "manifest-invalid-semver.json"},
		{rule: "artifact build provenance non-blank", document: manifest.ArtifactIdentitySchemaDocument, positive: "artifact-identity.json", negative: "artifact-blank-provenance-source.json"},
		{rule: "artifact rights authors non-blank", document: manifest.ArtifactIdentitySchemaDocument, positive: "artifact-identity.json", negative: "artifact-blank-rights-author.json"},
		{rule: "artifact package identity non-empty", document: manifest.ArtifactIdentitySchemaDocument, positive: "artifact-identity.json", negative: "artifact-empty-package-id.json"},
		{rule: "artifact package identity canonical", document: manifest.ArtifactIdentitySchemaDocument, positive: "artifact-identity.json", negative: "artifact-malformed-package-id.json"},
		{rule: "artifact semantic version", document: manifest.ArtifactIdentitySchemaDocument, positive: "artifact-identity.json", negative: "artifact-invalid-semver.json"},
		{rule: "lock package identity uniqueness", document: manifest.LockSchemaDocument, positive: "lock-unique-package-identities.json", negative: "lock-duplicate-package-identity.json"},
		{rule: "lock dependency identity", document: manifest.LockSchemaDocument, positive: "lock-unique-package-identities.json", negative: "lock-malformed-dependency-identity.json"},
	}
	for _, test := range tests {
		test := test
		t.Run(test.rule, func(t *testing.T) {
			t.Parallel()
			if err := validator.Validate(test.document, readCorpusFixture(t, "valid", test.positive)); err != nil {
				t.Fatalf("positive fixture %s: %v", test.positive, err)
			}
			if err := validator.Validate(test.document, readCorpusFixture(t, "invalid", test.negative)); err == nil {
				t.Fatalf("negative fixture %s passed", test.negative)
			}
		})
	}
}

type canonicalParityCase struct {
	name   string
	want   bool
	mutate func(map[string]any)
}

func TestManifestSchemaCanonicalParity(t *testing.T) {
	t.Parallel()
	validator := compiledPackageSchemas(t)
	tests := []canonicalParityCase{
		{name: "valid display_name", want: true},
		{name: "blank display_name", mutate: func(value map[string]any) { value["display_name"] = " " }},
		{name: "Unicode blank display_name", mutate: func(value map[string]any) { value["display_name"] = "\u00a0\u3000" }},
		{name: "non-space BOM display_name", want: true, mutate: func(value map[string]any) { value["display_name"] = "\ufeff" }},
		{name: "valid relative entrypoint", want: true},
		{name: "trimmed valid relative entrypoint", want: true, mutate: func(value map[string]any) { value["entrypoint"] = " lua/main.lua " }},
		{name: "parent traversal entrypoint", mutate: func(value map[string]any) { value["entrypoint"] = "../escape.lua" }},
		{name: "absolute entrypoint", mutate: func(value map[string]any) { value["entrypoint"] = "/lua/main.lua" }},
		{name: "empty entrypoint segment", mutate: func(value map[string]any) { value["entrypoint"] = "lua//main.lua" }},
		{name: "dot entrypoint segment", mutate: func(value map[string]any) { value["entrypoint"] = "lua/./main.lua" }},
		{name: "backslash entrypoint", mutate: func(value map[string]any) { value["entrypoint"] = `lua\main.lua` }},
		{name: "non-Lua entrypoint", mutate: func(value map[string]any) { value["entrypoint"] = "lua/main.txt" }},
		{name: "valid Host API range", want: true},
		{name: "inverted Host API range", mutate: func(value map[string]any) {
			hostAPI := value["host_api"].(map[string]any)
			hostAPI["min_minor"], hostAPI["max_minor"] = float64(3), float64(2)
		}},
		{name: "zero Host API major", mutate: func(value map[string]any) { value["host_api"].(map[string]any)["major"] = float64(0) }},
		{name: "unique dependency package IDs", want: true},
		{name: "duplicate dependency package ID", mutate: func(value map[string]any) {
			dependencies := value["dependencies"].([]any)
			value["dependencies"] = append(dependencies, map[string]any{
				"package_id": "example.assets/cards", "version": "3.0.0", "optional": true, "features": []any{},
			})
		}},
		{name: "invalid package kind", mutate: func(value map[string]any) { value["package_kind"] = "bundle" }},
		{name: "invalid capability", mutate: func(value map[string]any) {
			value["capabilities"].(map[string]any)["required"] = []any{"host.unknown"}
		}},
		{name: "malformed package ID", mutate: func(value map[string]any) { value["package_id"] = "malformed" }},
		{name: "invalid SemVer", mutate: func(value map[string]any) { value["version"] = "1.2" }},
		{name: "blank build source", mutate: func(value map[string]any) { value["build"].(map[string]any)["source"] = " " }},
		{name: "blank rights author", mutate: func(value map[string]any) { value["rights"].(map[string]any)["authors"] = []any{" "} }},
		{name: "duplicate trimmed rights author", mutate: func(value map[string]any) {
			value["rights"].(map[string]any)["authors"] = []any{"Example Studio", " Example Studio "}
		}},
		{name: "blank optional fallback", mutate: func(value map[string]any) {
			optional := value["capabilities"].(map[string]any)["optional"].([]any)
			optional[0].(map[string]any)["fallback"] = " "
		}},
		{name: "display_name over 200 UTF-8 bytes", mutate: func(value map[string]any) { value["display_name"] = strings.Repeat("界", 67) }},
		{name: "invalid Lua profile", mutate: func(value map[string]any) { value["lua_profile"] = "Platform Lua" }},
		{name: "trimmed valid Lua profile", want: true, mutate: func(value map[string]any) { value["lua_profile"] = " platform-lua-5.5-p1 " }},
		{name: "blank Lua profile", mutate: func(value map[string]any) { value["lua_profile"] = " " }},
	}
	assertCanonicalParity(t, validator, manifest.ManifestSchemaDocument, "manifest-unique-dependencies.json", tests)
}

func TestArtifactIdentitySchemaCanonicalParity(t *testing.T) {
	t.Parallel()
	validator := compiledPackageSchemas(t)
	tests := []canonicalParityCase{
		{name: "valid canonical identity", want: true},
		{name: "valid provenance source", want: true},
		{name: "blank build provenance source", mutate: func(value map[string]any) { value["build_provenance"].(map[string]any)["source"] = " " }},
		{name: "valid rights author", want: true},
		{name: "blank rights author", mutate: func(value map[string]any) { value["rights"].(map[string]any)["authors"] = []any{" "} }},
		{name: "duplicate trimmed rights author", mutate: func(value map[string]any) {
			value["rights"].(map[string]any)["authors"] = []any{"Example Studio", " Example Studio "}
		}},
		{name: "empty package ID", mutate: func(value map[string]any) {
			setIdentityField(value, manifest.ArtifactIdentitySchemaDocument, "package_id", "")
		}},
		{name: "malformed package ID", mutate: func(value map[string]any) {
			setIdentityField(value, manifest.ArtifactIdentitySchemaDocument, "package_id", "malformed")
		}},
		{name: "invalid SemVer", mutate: func(value map[string]any) {
			setIdentityField(value, manifest.ArtifactIdentitySchemaDocument, "version", "1.2")
		}},
		{name: "invalid content hash", mutate: func(value map[string]any) {
			setArtifactContentHash(value, "sha256:NOT-A-HASH")
		}},
		{name: "artifact and lock version mismatch", mutate: func(value map[string]any) { value["version"] = "2.0.0" }},
	}
	assertCanonicalParity(t, validator, manifest.ArtifactIdentitySchemaDocument, "artifact-identity.json", tests)
}

func setArtifactContentHash(instance map[string]any, value string) {
	instance["content_hash"] = value
	lock := instance["dependency_lock"].(map[string]any)
	lock["packages"].([]any)[0].(map[string]any)["content_hash"] = value
}

func TestDependencySchemaCanonicalParity(t *testing.T) {
	t.Parallel()
	validator := compiledPackageSchemas(t)
	tests := []canonicalParityCase{
		{name: "valid unique dependency graph representation", want: true},
		{name: "duplicate package identity", mutate: func(value map[string]any) {
			packages := value["packages"].([]any)
			value["packages"] = append(packages, map[string]any{
				"package_id":   "example.rules/schema-fixture",
				"version":      "2.0.0",
				"content_hash": "sha256:3333333333333333333333333333333333333333333333333333333333333333",
				"features":     []any{}, "dependencies": []any{"example.shared/card-library"},
			})
		}},
		{name: "malformed dependency identity", mutate: func(value map[string]any) {
			value["packages"].([]any)[0].(map[string]any)["dependencies"] = []any{"malformed"}
		}},
		{name: "unlocked dependency", mutate: func(value map[string]any) {
			value["packages"].([]any)[0].(map[string]any)["dependencies"] = []any{"example.missing/package"}
		}},
		{name: "dependency cycle", mutate: func(value map[string]any) {
			value["packages"].([]any)[1].(map[string]any)["dependencies"] = []any{"example.rules/schema-fixture"}
		}},
		{name: "unreachable package", mutate: func(value map[string]any) {
			packages := value["packages"].([]any)
			value["packages"] = append(packages, map[string]any{
				"package_id": "example.unused/package", "version": "1.0.0",
				"content_hash": "sha256:4444444444444444444444444444444444444444444444444444444444444444",
				"features":     []any{}, "dependencies": []any{},
			})
		}},
	}
	assertCanonicalParity(t, validator, manifest.LockSchemaDocument, "lock-unique-package-identities.json", tests)
}

func assertCanonicalParity(t *testing.T, validator *manifest.SchemaConformance, document manifest.SchemaDocument, fixture string, tests []canonicalParityCase) {
	t.Helper()
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			instance := readCorpusJSONObject(t, "valid", fixture)
			if test.mutate != nil {
				test.mutate(instance)
			}
			data := marshalJSONObject(t, instance)
			structuralAccept := validator.ValidateStructure(document, data) == nil
			goCanonicalAccept := validator.ValidateCanonical(document, data) == nil
			publicAccept := validator.Validate(document, data) == nil
			if publicAccept != goCanonicalAccept {
				t.Fatalf("public conformance accepts=%t, Go canonical accepts=%t (structural accepts=%t)", publicAccept, goCanonicalAccept, structuralAccept)
			}
			if publicAccept != test.want {
				t.Fatalf("accepts=%t, want=%t (structural=%t canonical=%t)", publicAccept, test.want, structuralAccept, goCanonicalAccept)
			}
		})
	}
}

func TestR2AdversarialCanonicalConformanceProbes(t *testing.T) {
	t.Parallel()
	validator := compiledPackageSchemas(t)
	tests := []struct {
		name           string
		document       manifest.SchemaDocument
		fixture        string
		wantStructural bool
	}{
		{name: "blank display_name", document: manifest.ManifestSchemaDocument, fixture: "manifest-blank-display-name.json"},
		{name: "../escape.lua", document: manifest.ManifestSchemaDocument, fixture: "manifest-unsafe-entrypoint.json", wantStructural: true},
		{name: "min_minor 3 / max_minor 2", document: manifest.ManifestSchemaDocument, fixture: "manifest-inverted-host-api-range.json", wantStructural: true},
		{name: "duplicate dependency package_id", document: manifest.ManifestSchemaDocument, fixture: "manifest-duplicate-dependency.json", wantStructural: true},
		{name: "blank provenance source", document: manifest.ArtifactIdentitySchemaDocument, fixture: "artifact-blank-provenance-source.json"},
		{name: "blank rights author", document: manifest.ArtifactIdentitySchemaDocument, fixture: "artifact-blank-rights-author.json"},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			data := readCorpusFixture(t, "invalid", test.fixture)
			structuralAccept := validator.ValidateStructure(test.document, data) == nil
			canonicalAccept := validator.ValidateCanonical(test.document, data) == nil
			goAccept := canonicalAccept
			finalAccept := validator.Validate(test.document, data) == nil
			t.Logf("structural=%s canonical=%s Go=%s final=%s expected=REJECT", acceptance(structuralAccept), acceptance(canonicalAccept), acceptance(goAccept), acceptance(finalAccept))
			if structuralAccept != test.wantStructural {
				t.Errorf("structural acceptance = %t, want %t", structuralAccept, test.wantStructural)
			}
			if canonicalAccept || goAccept || finalAccept {
				t.Fatal("adversarial probe was not rejected by every canonical/public layer")
			}
		})
	}
}

func acceptance(accepted bool) string {
	if accepted {
		return "ACCEPT"
	}
	return "REJECT"
}

func readCorpusFixture(t *testing.T, corpus, name string) []byte {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("..", "testdata", "schema", corpus, name))
	if err != nil {
		t.Fatal(err)
	}
	return data
}

func readCorpusJSONObject(t *testing.T, corpus, name string) map[string]any {
	t.Helper()
	var result map[string]any
	if err := json.Unmarshal(readCorpusFixture(t, corpus, name), &result); err != nil {
		t.Fatal(err)
	}
	return result
}

func marshalJSONObject(t *testing.T, value map[string]any) []byte {
	t.Helper()
	data, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return data
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
