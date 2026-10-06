// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package manifest_test

import (
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/manifest"
)

func TestEveryHostCategoryHasIdenticalGoV1V2DeclarationConstraints(t *testing.T) {
	t.Parallel()
	validator := compiledPackageSchemas(t)
	versions := []struct {
		name, fixture string
		document      manifest.SchemaDocument
	}{
		{"v1", "manifest-package.json", manifest.ManifestSchemaDocument},
		{"v2", "manifest-v2-extension.json", manifest.ManifestV2SchemaDocument},
	}
	for _, version := range versions {
		t.Run(version.name, func(t *testing.T) {
			t.Parallel()
			for _, name := range capability.RegisteredNames() {
				t.Run(string(name), func(t *testing.T) {
					t.Parallel()
					raw := string(name)
					optional := func(fallback string) map[string]any {
						return map[string]any{"name": raw, "fallback": fallback}
					}
					cases := []struct {
						name         string
						capabilities map[string]any
						valid        bool
					}{
						{"required", map[string]any{"required": []string{raw}}, true},
						{"optional", map[string]any{"optional": []any{optional("disable")}}, true},
						{"duplicate-required", map[string]any{"required": []string{raw, raw}}, false},
						{"duplicate-optional-same-fallback", map[string]any{"optional": []any{optional("disable"), optional("disable")}}, false},
						{"duplicate-optional-distinct-fallback", map[string]any{"optional": []any{optional("disable"), optional("another fallback")}}, false},
						{"overlap", map[string]any{"required": []string{raw}, "optional": []any{optional("disable")}}, false},
						{"missing-fallback", map[string]any{"optional": []any{map[string]any{"name": raw}}}, false},
						{"empty-fallback", map[string]any{"optional": []any{optional("")}}, false},
						{"blank-fallback", map[string]any{"optional": []any{optional(" \t\n")}}, false},
					}
					for _, test := range cases {
						t.Run(test.name, func(t *testing.T) {
							t.Parallel()
							value := readCorpusJSONObject(t, "valid", version.fixture)
							value["capabilities"] = test.capabilities
							data := marshalJSONObject(t, value)
							for _, layer := range []struct {
								name     string
								validate func(manifest.SchemaDocument, []byte) error
							}{
								{"schema", validator.ValidateStructure},
								{"go", validator.ValidateCanonical},
								{"combined", validator.Validate},
							} {
								if err := layer.validate(version.document, data); (err == nil) != test.valid {
									t.Fatalf("%s valid=%v, want %v: %v", layer.name, err == nil, test.valid, err)
								}
							}
						})
					}
				})
			}
		})
	}
}

func TestHostCategorySchemaRemainsClosedForPrivilegedNames(t *testing.T) {
	t.Parallel()
	validator := compiledPackageSchemas(t)
	for _, raw := range []string{"host.network", "host.filesystem", "host.db.raw-sql", "host.db.ddl", "host.task.execute", "host.unknown"} {
		t.Run(raw, func(t *testing.T) {
			t.Parallel()
			for _, fixture := range []string{"manifest-package.json", "manifest-v2-extension.json"} {
				value := readCorpusJSONObject(t, "valid", fixture)
				value["capabilities"] = map[string]any{"required": []string{raw}}
				document := manifest.ManifestSchemaDocument
				if fixture == "manifest-v2-extension.json" {
					document = manifest.ManifestV2SchemaDocument
				}
				data := marshalJSONObject(t, value)
				if err := validator.ValidateStructure(document, data); err == nil {
					t.Fatal("public schema accepted an unregistered privileged name")
				}
				if err := validator.ValidateCanonical(document, data); err == nil {
					t.Fatal("Go validation accepted an unregistered privileged name")
				}
			}
		})
	}
}
