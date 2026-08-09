// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"bytes"
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/manifest"
)

type packageContractParityCase struct {
	name       string
	want       bool
	content    bool
	mutateJSON func(map[string]any)
	mutateTOML func(*testing.T, string) string
}

func TestPackageContractPublicGoCLIParity(t *testing.T) {
	validator := compiledCLIParitySchemas(t)
	runtimeTOML := readCLIParityFixture(t, "package.toml")
	contentTOML := contentManifestWithoutRuntime(t, runtimeTOML)
	tests := []packageContractParityCase{
		{name: "valid canonical package", want: true},
		{name: "entrypoint absent", want: true, content: true},
		{name: "entrypoint explicit empty", content: true,
			mutateJSON: func(value map[string]any) { value["entrypoint"] = "" },
			mutateTOML: func(t *testing.T, value string) string { return addTopLevelTOMLString(t, value, "entrypoint", "") }},
		{name: "entrypoint explicit whitespace", content: true,
			mutateJSON: func(value map[string]any) { value["entrypoint"] = " " },
			mutateTOML: func(t *testing.T, value string) string { return addTopLevelTOMLString(t, value, "entrypoint", " ") }},
		{name: "entrypoint valid canonical value", want: true},
		{name: "lua_profile absent", want: true, content: true},
		{name: "lua_profile explicit empty", content: true,
			mutateJSON: func(value map[string]any) { value["lua_profile"] = "" },
			mutateTOML: func(t *testing.T, value string) string { return addTopLevelTOMLString(t, value, "lua_profile", "") }},
		{name: "lua_profile explicit whitespace", content: true,
			mutateJSON: func(value map[string]any) { value["lua_profile"] = " " },
			mutateTOML: func(t *testing.T, value string) string { return addTopLevelTOMLString(t, value, "lua_profile", " ") }},
		{name: "lua_profile valid canonical value", want: true},
		{name: "rights statement absent", want: true},
		{name: "rights statement explicit empty",
			mutateJSON: func(value map[string]any) { value["rights"].(map[string]any)["statement"] = "" },
			mutateTOML: func(t *testing.T, value string) string { return addRightsTOMLString(t, value, "statement", "") }},
		{name: "rights statement explicit whitespace",
			mutateJSON: func(value map[string]any) { value["rights"].(map[string]any)["statement"] = " " },
			mutateTOML: func(t *testing.T, value string) string { return addRightsTOMLString(t, value, "statement", " ") }},
		{name: "rights statement valid canonical value", want: true,
			mutateJSON: func(value map[string]any) { value["rights"].(map[string]any)["statement"] = "Private rights" },
			mutateTOML: func(t *testing.T, value string) string {
				return addRightsTOMLString(t, value, "statement", "Private rights")
			}},
		{name: "license expression absent", want: true,
			mutateJSON: func(value map[string]any) {
				rights := value["rights"].(map[string]any)
				delete(rights, "license_expression")
				rights["statement"] = "Private rights"
			},
			mutateTOML: func(t *testing.T, value string) string {
				return replaceParityOnce(t, value, `license_expression = "LicenseRef-Example-Private"`, `statement = "Private rights"`)
			}},
		{name: "license expression explicit empty",
			mutateJSON: func(value map[string]any) {
				rights := value["rights"].(map[string]any)
				rights["license_expression"] = ""
				rights["statement"] = "Private rights"
			},
			mutateTOML: func(t *testing.T, value string) string {
				return replaceParityOnce(t, value, `license_expression = "LicenseRef-Example-Private"`, "license_expression = \"\"\nstatement = \"Private rights\"")
			}},
		{name: "license expression explicit whitespace",
			mutateJSON: func(value map[string]any) {
				rights := value["rights"].(map[string]any)
				rights["license_expression"] = " "
				rights["statement"] = "Private rights"
			},
			mutateTOML: func(t *testing.T, value string) string {
				return replaceParityOnce(t, value, `license_expression = "LicenseRef-Example-Private"`, "license_expression = \" \"\nstatement = \"Private rights\"")
			}},
		{name: "license expression valid canonical value", want: true},
		{name: "invalid package kind",
			mutateJSON: func(value map[string]any) { value["package_kind"] = "bundle" },
			mutateTOML: func(t *testing.T, value string) string {
				return replaceParityOnce(t, value, `package_kind = "game-system"`, `package_kind = "bundle"`)
			}},
		{name: "unknown capability",
			mutateJSON: func(value map[string]any) { value["capabilities"].(map[string]any)["required"] = []any{"host.unknown"} },
			mutateTOML: func(t *testing.T, value string) string {
				return replaceParityOnce(t, value, `"host.event"`, `"host.unknown"`)
			}},
		{name: "malformed package ID",
			mutateJSON: func(value map[string]any) { value["package_id"] = "malformed" },
			mutateTOML: func(t *testing.T, value string) string {
				return replaceParityOnce(t, value, `package_id = "example.rules/hidden-cards"`, `package_id = "malformed"`)
			}},
		{name: "invalid SemVer",
			mutateJSON: func(value map[string]any) { value["version"] = "1.2" },
			mutateTOML: func(t *testing.T, value string) string {
				return replaceParityOnce(t, value, `version = "1.2.3"`, `version = "1.2"`)
			}},
		{name: "blank build provenance source",
			mutateJSON: func(value map[string]any) { value["build"].(map[string]any)["source"] = " " },
			mutateTOML: func(t *testing.T, value string) string {
				return replaceParityOnce(t, value, `source = "https://example.invalid/hidden-cards"`, `source = " "`)
			}},
		{name: "blank rights author",
			mutateJSON: func(value map[string]any) { value["rights"].(map[string]any)["authors"] = []any{" "} },
			mutateTOML: func(t *testing.T, value string) string {
				return replaceParityOnce(t, value, `authors = ["Example Studio"]`, `authors = [" "]`)
			}},
		{name: "malformed dependency identity",
			mutateJSON: func(value map[string]any) {
				value["dependencies"].([]any)[0].(map[string]any)["package_id"] = "malformed"
			},
			mutateTOML: func(t *testing.T, value string) string {
				return replaceParityOnce(t, value, `package_id = "example.shared/card-library"`, `package_id = "malformed"`)
			}},
	}

	paths := []struct {
		name  string
		value string
		want  bool
	}{
		{name: "canonical relative path", value: "lua/main.lua", want: true},
		{name: "ordinary repeated dots segment", value: ".../x.lua", want: true},
		{name: "ordinary embedded dots segment", value: "foo..bar/x.lua", want: true},
		{name: "absolute path", value: "/main.lua"},
		{name: "parent traversal", value: "../main.lua"},
		{name: "nested parent traversal", value: "a/../../main.lua"},
		{name: "upper-case Windows drive", value: "C:/escape.lua"},
		{name: "lower-case Windows drive", value: "c:/escape.lua"},
		{name: "Windows drive backslash", value: `C:\escape.lua`},
		{name: "embedded Windows drive", value: "a/C:/escape.lua"},
		{name: "embedded drive-relative", value: "a/z:relative.lua"},
		{name: "colon before separator", value: "foo:/bar.lua"},
		{name: "colon inside segment", value: "x:y.lua"},
		{name: "slash UNC", value: "//server/share/a.lua"},
		{name: "backslash UNC", value: `\\server\share\a.lua`},
		{name: "Windows device", value: `\\?\C:\a.lua`},
		{name: "Windows local device", value: `\\.\C:\a.lua`},
		{name: "NUL", value: "lua/\x00.lua"},
		{name: "C0 control", value: "lua/\x1f.lua"},
		{name: "C1 control", value: "lua/\u0085.lua"},
		{name: "leading dot segment", value: "./lua/main.lua"},
		{name: "empty segment", value: "lua//main.lua"},
		{name: "cleaning traversal", value: "lua/../main.lua"},
	}
	for _, pathCase := range paths {
		pathCase := pathCase
		tests = append(tests, packageContractParityCase{
			name: "portable path / " + pathCase.name,
			want: pathCase.want,
			mutateJSON: func(value map[string]any) {
				value["entrypoint"] = pathCase.value
			},
			mutateTOML: func(t *testing.T, value string) string {
				return replaceParityOnce(t, value, `entrypoint = "lua/main.lua"`, "entrypoint = "+tomlParityString(t, pathCase.value))
			},
		})
	}

	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			jsonFixture := "manifest-unique-dependencies.json"
			tomlInput := runtimeTOML
			if test.content {
				jsonFixture = "manifest-package.json"
				tomlInput = contentTOML
			}
			instance := readCLIParityJSON(t, jsonFixture)
			if test.mutateJSON != nil {
				test.mutateJSON(instance)
			}
			jsonInput, err := json.Marshal(instance)
			if err != nil {
				t.Fatal(err)
			}
			if test.mutateTOML != nil {
				tomlInput = test.mutateTOML(t, tomlInput)
			}

			structuralAccept := validator.ValidateStructure(manifest.ManifestSchemaDocument, jsonInput) == nil
			goCanonicalAccept := validator.ValidateCanonical(manifest.ManifestSchemaDocument, jsonInput) == nil
			publicAccept := validator.Validate(manifest.ManifestSchemaDocument, jsonInput) == nil
			if publicAccept != (structuralAccept && goCanonicalAccept) {
				t.Fatalf("public=%t, structural=%t, Go canonical=%t", publicAccept, structuralAccept, goCanonicalAccept)
			}

			manifestPath := writeCLIInput(t, "parity.toml", tomlInput)
			var stdout bytes.Buffer
			var stderr bytes.Buffer
			exitCode := run(context.Background(), []string{
				"package", "validate", "--manifest", manifestPath, "--lock", packageFixture("package.lock.json"),
			}, &stdout, &stderr)
			cliAccept := exitCode == 0
			if cliAccept != strings.Contains(stdout.String(), `"valid":true`) {
				t.Fatalf("CLI exit=%d stdout=%q stderr=%q", exitCode, stdout.String(), stderr.String())
			}
			finalVerdict := "REJECT"
			if publicAccept && goCanonicalAccept && cliAccept {
				finalVerdict = "ACCEPT"
			}
			t.Logf("structural=%s public=%s Go=%s CLI=%s exit=%d final=%s",
				parityDecision(structuralAccept), parityDecision(publicAccept), parityDecision(goCanonicalAccept), parityDecision(cliAccept), exitCode, finalVerdict)
			if publicAccept != goCanonicalAccept || publicAccept != cliAccept {
				t.Fatalf("public=%t, Go canonical=%t, CLI=%t (structural=%t exit=%d stderr=%q)", publicAccept, goCanonicalAccept, cliAccept, structuralAccept, exitCode, stderr.String())
			}
			if publicAccept != test.want {
				t.Fatalf("accept=%t, want=%t (structural=%t Go canonical=%t CLI exit=%d)", publicAccept, test.want, structuralAccept, goCanonicalAccept, exitCode)
			}
		})
	}
}

func TestExtraUndeclaredFeaturePublicGoCLIParity(t *testing.T) {
	manifestDocument, err := manifest.Parse([]byte(readCLIParityFixture(t, "package.toml")))
	if err != nil {
		t.Fatal(err)
	}
	lockInput := replaceParityOnce(t, readCLIParityFixture(t, "package.lock.json"),
		`"features": ["standard-deck"]`, `"features": ["extra-feature", "standard-deck"]`)
	lock, err := dependency.ParseExactLock([]byte(lockInput))
	if err != nil {
		t.Fatalf("standalone exact lock should remain structurally canonical: %v", err)
	}
	root, exists := lock.Package(lock.Root())
	if !exists {
		t.Fatal("exact lock has no root package")
	}
	_, publicErr := manifest.BuildArtifactIdentity(*manifestDocument.Package, root.ContentHash, lock)
	goErr := dependency.ValidateRequirements(lock, manifestDocument.Package.PackageID, manifestDocument.Package.Dependencies)

	lockPath := writeCLIInput(t, "extra-feature.lock.json", lockInput)
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	exitCode := run(context.Background(), []string{
		"package", "validate", "--manifest", packageFixture("package.toml"), "--lock", lockPath,
	}, &stdout, &stderr)
	publicAccept := publicErr == nil
	goAccept := goErr == nil
	cliAccept := exitCode == 0
	t.Logf("public=%s Go=%s CLI=%s exit=%d final=REJECT",
		parityDecision(publicAccept), parityDecision(goAccept), parityDecision(cliAccept), exitCode)
	if publicAccept || goAccept || cliAccept {
		t.Fatalf("extra undeclared Feature escaped validation: public error=%v Go error=%v CLI stdout=%q stderr=%q", publicErr, goErr, stdout.String(), stderr.String())
	}
	if strings.Contains(stdout.String(), `"valid":true`) {
		t.Fatalf("rejected extra Feature reported valid:true: %s", stdout.String())
	}
}

func parityDecision(accepted bool) string {
	if accepted {
		return "ACCEPT"
	}
	return "REJECT"
}

func compiledCLIParitySchemas(t *testing.T) *manifest.SchemaConformance {
	t.Helper()
	read := func(name string) []byte {
		data, err := os.ReadFile(filepath.Join("..", "..", "schemas", "package", name))
		if err != nil {
			t.Fatal(err)
		}
		return data
	}
	validator, err := manifest.NewSchemaConformance(manifest.SchemaResources{
		Manifest:         read(string(manifest.ManifestSchemaDocument)),
		Lock:             read(string(manifest.LockSchemaDocument)),
		ArtifactIdentity: read(string(manifest.ArtifactIdentitySchemaDocument)),
	})
	if err != nil {
		t.Fatal(err)
	}
	return validator
}

func readCLIParityFixture(t *testing.T, name string) string {
	t.Helper()
	data, err := os.ReadFile(packageFixture(name))
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

func readCLIParityJSON(t *testing.T, name string) map[string]any {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("..", "..", "internal", "package", "testdata", "schema", "valid", name))
	if err != nil {
		t.Fatal(err)
	}
	var value map[string]any
	if err := json.Unmarshal(data, &value); err != nil {
		t.Fatal(err)
	}
	return value
}

func contentManifestWithoutRuntime(t *testing.T, value string) string {
	t.Helper()
	value = replaceParityOnce(t, value, `package_kind = "game-system"`, `package_kind = "content"`)
	return replaceParityOnce(t, value, `entrypoint = "lua/main.lua"
lua_profile = "platform-lua-5.5-p1"

[host_api]
major = 1
min_minor = 0
max_minor = 2

`, "")
}

func addTopLevelTOMLString(t *testing.T, value, field, fieldValue string) string {
	t.Helper()
	anchor := `display_name = "Hidden Cards Fixture"`
	return replaceParityOnce(t, value, anchor, anchor+"\n"+field+" = "+tomlParityString(t, fieldValue))
}

func addRightsTOMLString(t *testing.T, value, field, fieldValue string) string {
	t.Helper()
	anchor := `license_expression = "LicenseRef-Example-Private"`
	return replaceParityOnce(t, value, anchor, anchor+"\n"+field+" = "+tomlParityString(t, fieldValue))
}

func tomlParityString(t *testing.T, value string) string {
	t.Helper()
	encoded, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return string(encoded)
}

func replaceParityOnce(t *testing.T, value, old, replacement string) string {
	t.Helper()
	if strings.Count(value, old) != 1 {
		t.Fatalf("parity fixture contains %q %d times, want once", old, strings.Count(value, old))
	}
	return strings.Replace(value, old, replacement, 1)
}
