// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package package_extensions_test

import (
	"bytes"
	"encoding/json"
	"go/ast"
	"go/parser"
	"go/token"
	"io/fs"
	"os"
	"path/filepath"
	"reflect"
	"strconv"
	"strings"
	"testing"
	"unicode"

	"github.com/zyc14588/TRPG_PLATFORM/apps/creator-studio/creator"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
)

func TestN01UndeclaredExtensionPayloadIsRejected(t *testing.T) {
	files := v2Files(t, extensionFixture{namespace: probeNamespace, contractVersion: 1})
	files["extensions/third.party.probe/undeclared.json"] = []byte(`{"hidden":true}`)
	_, err := archive.FromFiles(files, fixtureLock(t), extension.DefaultSupport)
	if err == nil || !strings.Contains(err.Error(), "not declared") {
		t.Fatalf("undeclared payload error = %v", err)
	}
}

func TestN02DuplicateNamespaceIsRejected(t *testing.T) {
	files := v2Files(t,
		extensionFixture{namespace: probeNamespace, contractVersion: 1},
		extensionFixture{namespace: probeNamespace, contractVersion: 1},
	)
	_, err := archive.FromFiles(files, fixtureLock(t), extension.DefaultSupport)
	requireExtensionCode(t, err, extension.ErrInvalid)
}

func TestN03InvalidNamespaceIsRejected(t *testing.T) {
	files := v2Files(t, extensionFixture{namespace: "Invalid.Namespace.Probe", contractVersion: 1})
	_, err := archive.FromFiles(files, fixtureLock(t), extension.DefaultSupport)
	requireExtensionCode(t, err, extension.ErrInvalid)
}

func TestN04ReservedPlatformNamespaceSpoofIsRejected(t *testing.T) {
	files := v2Files(t, extensionFixture{namespace: "trpg.platform.spoof", contractVersion: 1})
	_, err := archive.FromFiles(files, fixtureLock(t), extension.DefaultSupport)
	requireExtensionCode(t, err, extension.ErrInvalid)
}

func TestN05SchemaDigestMismatchIsRejectedForOptionalToo(t *testing.T) {
	files := v2Files(t, extensionFixture{namespace: probeNamespace, contractVersion: 1})
	files[probeSchemaPath] = append(files[probeSchemaPath], '\n')
	_, err := archive.FromFiles(files, fixtureLock(t), extension.DefaultSupport)
	requireExtensionCode(t, err, extension.ErrSchemaDigestMismatch)
}

func TestN06RequiredMissingSchemaReturnsTypedFailure(t *testing.T) {
	files := v2Files(t, extensionFixture{namespace: probeNamespace, required: true, contractVersion: 1})
	delete(files, probeSchemaPath)
	_, err := archive.FromFiles(files, fixtureLock(t), extension.DefaultSupport)
	requireExtensionCode(t, err, extension.ErrRequiredUnsupported)
}

func TestN07DescriptorPathTraversalIsRejected(t *testing.T) {
	files := v2Files(t, extensionFixture{namespace: probeNamespace, contractVersion: 1})
	replaceManifest(t, files,
		`schema_path = "extensions/third.party.probe/value.schema.json"`,
		`schema_path = "extensions/third.party.probe/../escape.schema.json"`)
	_, err := archive.FromFiles(files, fixtureLock(t), extension.DefaultSupport)
	requireExtensionCode(t, err, extension.ErrInvalid)
}

func TestN08AbsoluteDescriptorPathIsRejected(t *testing.T) {
	files := v2Files(t, extensionFixture{namespace: probeNamespace, contractVersion: 1})
	replaceManifest(t, files,
		`schema_path = "extensions/third.party.probe/value.schema.json"`,
		`schema_path = "/tmp/escape.schema.json"`)
	_, err := archive.FromFiles(files, fixtureLock(t), extension.DefaultSupport)
	requireExtensionCode(t, err, extension.ErrInvalid)
}

func TestN09DirectoryAndZIPSymlinkEscapeAreRejected(t *testing.T) {
	t.Run("project symlink", func(t *testing.T) {
		root := writeProjectFiles(t, v2Files(t, extensionFixture{namespace: probeNamespace, contractVersion: 1}))
		payload := filepath.Join(root, filepath.FromSlash(probePayloadPath))
		if err := os.Remove(payload); err != nil {
			t.Fatal(err)
		}
		outside := filepath.Join(t.TempDir(), "outside.json")
		if err := os.WriteFile(outside, []byte(`{"count":1}`), 0o600); err != nil {
			t.Fatal(err)
		}
		if err := os.Symlink(outside, payload); err != nil {
			t.Skipf("symlink unavailable: %v", err)
		}
		if _, err := archive.ImportProject(root, fixtureLock(t), extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "symbolic link") {
			t.Fatalf("project symlink error = %v", err)
		}
	})
	t.Run("ZIP symlink", func(t *testing.T) {
		data := zipWithSymlink(t, probePayloadPath, "../../outside.json")
		if _, err := archive.ImportBytes(data, extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "directory, symlink, or special file") {
			t.Fatalf("ZIP symlink preflight error = %v", err)
		}
	})
}

func TestN10RemoteAndCrossFileSchemaReferencesAreUnavailable(t *testing.T) {
	for _, test := range []struct {
		name string
		ref  string
	}{
		{name: "remote", ref: "https://example.invalid/escape.schema.json"},
		{name: "cross-file", ref: "other.schema.json#/$defs/value"},
	} {
		t.Run(test.name, func(t *testing.T) {
			schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","$ref":"` + test.ref + `"}`)
			files := v2Files(t, extensionFixture{
				namespace: probeNamespace, required: true, contractVersion: 1, schema: schema, payload: []byte(`{}`),
			})
			_, err := archive.FromFiles(files, fixtureLock(t), extension.DefaultSupport)
			requireExtensionCode(t, err, extension.ErrInvalid)
		})
	}
}

func TestN11RequiredUnsupportedContractReturnsTypedFailure(t *testing.T) {
	files := v2Files(t, extensionFixture{namespace: probeNamespace, required: true, contractVersion: 2})
	_, err := archive.FromFiles(files, fixtureLock(t), extension.DefaultSupport)
	requireExtensionCode(t, err, extension.ErrRequiredUnsupported)
}

func TestN12ExtensionCannotOverrideCorePathOrField(t *testing.T) {
	t.Run("descriptor core path", func(t *testing.T) {
		files := v2Files(t, extensionFixture{namespace: probeNamespace, contractVersion: 1})
		replaceManifest(t, files,
			`payload_path = "extensions/third.party.probe/value.json"`,
			`payload_path = "package.toml"`)
		_, err := archive.FromFiles(files, fixtureLock(t), extension.DefaultSupport)
		requireExtensionCode(t, err, extension.ErrInvalid)
	})
	t.Run("ordinary JSON member remains isolated", func(t *testing.T) {
		schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["package_id"],"properties":{"package_id":{"type":"string"}}}`)
		files := v2Files(t, extensionFixture{
			namespace: probeNamespace, contractVersion: 1, schema: schema,
			payload: []byte(`{"package_id":"attacker.scope/fake"}`),
		})
		pkg := buildPackage(t, files)
		document, err := pkg.Manifest()
		if err != nil {
			t.Fatal(err)
		}
		if got := document.Package.PackageID.String(); got != "example.rules/hidden-cards" {
			t.Fatalf("payload member overrode core package_id: %q", got)
		}
	})
}

func TestN13PayloadOverFourMiBIsRejected(t *testing.T) {
	payload := bytes.Repeat([]byte{'0'}, extension.MaxPayloadBytes+1)
	files := v2Files(t, extensionFixture{
		namespace: probeNamespace, contractVersion: 1, payload: payload,
	})
	_, err := archive.FromFiles(files, fixtureLock(t), extension.DefaultSupport)
	contract := requireExtensionCode(t, err, extension.ErrInvalid)
	if !strings.Contains(contract.Detail, "payload exceeds") {
		t.Fatalf("oversized payload detail = %q", contract.Detail)
	}
}

func TestN14JSONNestingBeyondSixtyFourIsRejected(t *testing.T) {
	const excessiveDepth = 65
	payload := []byte(strings.Repeat("[", excessiveDepth) + "0" + strings.Repeat("]", excessiveDepth))
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema"}`)
	files := v2Files(t, extensionFixture{
		namespace: probeNamespace, required: true, contractVersion: 1, schema: schema, payload: payload,
	})
	_, err := archive.FromFiles(files, fixtureLock(t), extension.DefaultSupport)
	contract := requireExtensionCode(t, err, extension.ErrInvalid)
	if !strings.Contains(contract.Detail, "nesting exceeds 64") {
		t.Fatalf("excess depth detail = %q", contract.Detail)
	}
}

func TestN15SerializationAndBuildRemainDeterministicUnderDifferentialInputs(t *testing.T) {
	files := v2Files(t, extensionFixture{namespace: probeNamespace, contractVersion: 1})
	baseline := exportPackage(t, buildPackage(t, cloneFiles(files)))
	for iteration := 0; iteration < 16; iteration++ {
		candidateFiles := reverseInsertionOrder(files)
		if iteration%2 != 0 {
			candidateFiles[probePayloadPath] = []byte("\n { \"count\" : 1 } \n")
			candidateFiles[archive.ManifestPath] = append(candidateFiles[archive.ManifestPath], []byte("\n# canonical input permutation\n")...)
		}
		candidate := exportPackage(t, buildPackage(t, candidateFiles))
		if !bytes.Equal(candidate.Bytes(), baseline.Bytes()) || candidate.Hash() != baseline.Hash() {
			t.Fatalf("differential iteration %d exposed nondeterministic output", iteration)
		}
	}
}

func TestN16CreatorEditPreservesEveryUneditedField(t *testing.T) {
	rawSchema := []byte("{ opaque-unknown-schema\n")
	rawPayload := []byte("opaque optional payload\x00\xff")
	files := v2Files(t,
		extensionFixture{namespace: probeNamespace, required: true, contractVersion: 1},
		extensionFixture{namespace: "vendor.opaque.probe", contractVersion: 2, schema: rawSchema, payload: rawPayload},
	)
	original := buildPackage(t, files)
	directory := t.TempDir()
	source := filepath.Join(directory, "source.trpgpkg")
	if err := os.WriteFile(source, exportPackage(t, original).Bytes(), 0o600); err != nil {
		t.Fatal(err)
	}
	service := creator.NewService()
	before, err := service.ImportArchive(source)
	if err != nil {
		t.Fatal(err)
	}
	result, err := service.Edit(before.ConflictToken, probeNamespace, `{"count":7}`)
	if err != nil {
		t.Fatal(err)
	}
	afterEdit, err := service.Inspect()
	if err != nil {
		t.Fatal(err)
	}
	assertInspectionFieldsPreserved(t, before, afterEdit, result.CanonicalJSON)
	output := filepath.Join(directory, "edited.trpgpkg")
	exported, err := service.Export(result.ConflictToken, output)
	if err != nil {
		t.Fatal(err)
	}
	afterReload, err := service.ImportArchive(exported.Path)
	if err != nil {
		t.Fatal(err)
	}
	assertInspectionFieldsPreserved(t, before, afterReload, result.CanonicalJSON)
	reloaded, err := archive.ImportFile(output, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(reloaded.ManifestBytes(), original.ManifestBytes()) {
		t.Fatal("Creator edit lost or rewrote manifest fields")
	}
	beforeEntries, afterEntries := packageEntries(original), packageEntries(reloaded)
	for name, data := range beforeEntries {
		if name == probePayloadPath {
			continue
		}
		if !bytes.Equal(afterEntries[name], data) {
			t.Fatalf("Creator edit changed unrelated entry %q", name)
		}
	}
}

func TestN17HostRoundtripPreservesEveryCanonicalAndOpaqueField(t *testing.T) {
	rawSchema := []byte("not-json-but-opaque\n")
	rawPayload := []byte("opaque\x00payload\xff")
	pkg := buildPackage(t, v2Files(t,
		extensionFixture{namespace: probeNamespace, required: true, contractVersion: 1},
		extensionFixture{namespace: "vendor.opaque.probe", contractVersion: 2, schema: rawSchema, payload: rawPayload},
	))
	first := exportPackage(t, pkg)
	reloaded, err := archive.Import(first, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(pkg.ManifestBytes(), reloaded.ManifestBytes()) ||
		!bytes.Equal(pkg.LockBytes(), reloaded.LockBytes()) ||
		!bytes.Equal(pkg.ArtifactBytes(), reloaded.ArtifactBytes()) ||
		!reflect.DeepEqual(packageEntries(pkg), packageEntries(reloaded)) ||
		pkg.ContentHash() != reloaded.ContentHash() {
		t.Fatal("Host reload lost a canonical package field")
	}
	beforeDocuments, afterDocuments := pkg.Extensions(), reloaded.Extensions()
	if len(beforeDocuments) != len(afterDocuments) {
		t.Fatalf("Host extension count changed: %d -> %d", len(beforeDocuments), len(afterDocuments))
	}
	for index := range beforeDocuments {
		if !reflect.DeepEqual(beforeDocuments[index].Descriptor, afterDocuments[index].Descriptor) ||
			beforeDocuments[index].Status != afterDocuments[index].Status ||
			beforeDocuments[index].ReadOnlyReason != afterDocuments[index].ReadOnlyReason ||
			!bytes.Equal(beforeDocuments[index].SchemaBytes(), afterDocuments[index].SchemaBytes()) ||
			!bytes.Equal(beforeDocuments[index].CanonicalPayload(), afterDocuments[index].CanonicalPayload()) {
			t.Fatalf("Host extension %d changed during reload", index)
		}
		if beforeDocuments[index].Status == extension.ReadOnly && !bytes.Equal(beforeDocuments[index].PayloadBytes(), afterDocuments[index].PayloadBytes()) {
			t.Fatalf("Host opaque payload %d changed during reload", index)
		}
	}
	repacked := exportPackage(t, reloaded)
	if !bytes.Equal(first.Bytes(), repacked.Bytes()) {
		t.Fatal("Host field preservation changed deterministic archive bytes")
	}
}

func TestN18ExecutableExtensionPathsAreRejectedButCodeLikeJSONIsData(t *testing.T) {
	t.Run("executable path", func(t *testing.T) {
		files := v2Files(t, extensionFixture{namespace: probeNamespace, contractVersion: 1})
		const codePath = "extensions/third.party.probe/execute.lua"
		replaceManifest(t, files,
			`payload_path = "extensions/third.party.probe/value.json"`,
			`payload_path = "extensions/third.party.probe/execute.lua"`)
		delete(files, probePayloadPath)
		files[codePath] = []byte("return true")
		_, err := archive.FromFiles(files, fixtureLock(t), extension.DefaultSupport)
		requireExtensionCode(t, err, extension.ErrInvalid)
	})
	t.Run("code-like string", func(t *testing.T) {
		sentinel := filepath.Join(t.TempDir(), "must-remain")
		if err := os.WriteFile(sentinel, []byte("present"), 0o600); err != nil {
			t.Fatal(err)
		}
		value := `return os.remove("` + sentinel + `"); local a="payload.lua"; local b="module.wasm"; eval("x"); file:///etc/passwd`
		payload, err := json.Marshal(value)
		if err != nil {
			t.Fatal(err)
		}
		schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"string"}`)
		pkg := buildPackage(t, v2Files(t, extensionFixture{
			namespace: probeNamespace, contractVersion: 1, schema: schema, payload: payload,
		}))
		document := pkg.Extensions()[0]
		if document.Status != extension.Supported || string(document.CanonicalPayload()) != string(payload) {
			t.Fatalf("ordinary JSON string was not preserved as data: %q", document.CanonicalPayload())
		}
		if _, err := os.Stat(sentinel); err != nil {
			t.Fatalf("code-like JSON string was executed: %v", err)
		}
	})
	t.Run("raw code in JSON path", func(t *testing.T) {
		schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"string"}`)
		files := v2Files(t, extensionFixture{
			namespace: probeNamespace, contractVersion: 1, schema: schema, payload: []byte("return true"),
		})
		_, err := archive.FromFiles(files, fixtureLock(t), extension.DefaultSupport)
		contract := requireExtensionCode(t, err, extension.ErrInvalid)
		if !strings.Contains(contract.Detail, "payload JSON") || !strings.Contains(contract.Detail, "unexpected JSON token") {
			t.Fatalf("raw code JSON error = %q", contract.Detail)
		}
	})
}

func TestN19SchemaAndDescriptorCannotAccessHostPaths(t *testing.T) {
	t.Run("schema file URI", func(t *testing.T) {
		schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","$ref":"file:///etc/passwd"}`)
		files := v2Files(t, extensionFixture{
			namespace: probeNamespace, required: true, contractVersion: 1, schema: schema, payload: []byte(`{}`),
		})
		_, err := archive.FromFiles(files, fixtureLock(t), extension.DefaultSupport)
		requireExtensionCode(t, err, extension.ErrInvalid)
	})
	t.Run("descriptor host path", func(t *testing.T) {
		files := v2Files(t, extensionFixture{namespace: probeNamespace, contractVersion: 1})
		replaceManifest(t, files,
			`payload_path = "extensions/third.party.probe/value.json"`,
			`payload_path = "C:/Windows/System32/hosts.json"`)
		_, err := archive.FromFiles(files, fixtureLock(t), extension.DefaultSupport)
		requireExtensionCode(t, err, extension.ErrInvalid)
	})
}

func TestN20ProductionSourceHasNoIdentitySpecificBranch(t *testing.T) {
	root := repositoryRoot(t)
	targets := []string{
		filepath.Join(root, "internal", "package", "archive"),
		filepath.Join(root, "internal", "package", "extension"),
		filepath.Join(root, "internal", "package", "manifest"),
		filepath.Join(root, "internal", "package", "packagepath"),
		filepath.Join(root, "internal", "package", "jsondocument"),
		filepath.Join(root, "cmd", "creator-cli"),
		filepath.Join(root, "apps", "creator-studio"),
	}
	for _, target := range targets {
		err := filepath.WalkDir(target, func(name string, entry fs.DirEntry, walkErr error) error {
			if walkErr != nil {
				return walkErr
			}
			if entry.IsDir() || filepath.Ext(name) != ".go" || strings.HasSuffix(name, "_test.go") {
				return nil
			}
			data, err := os.ReadFile(name)
			if err != nil {
				return err
			}
			return scanGoIdentitySource(name, data)
		})
		if err != nil {
			t.Fatal(err)
		}
	}
	textTargets := []string{
		filepath.Join(root, "apps", "creator-studio", "frontend", "src"),
	}
	for _, target := range textTargets {
		err := filepath.WalkDir(target, func(name string, entry fs.DirEntry, walkErr error) error {
			if walkErr != nil {
				return walkErr
			}
			if entry.IsDir() || strings.Contains(filepath.Base(name), ".test.") {
				return nil
			}
			data, err := os.ReadFile(name)
			if err != nil {
				return err
			}
			return scanTextIdentitySource(name, data)
		})
		if err != nil {
			t.Fatal(err)
		}
	}
	schemaRoot := filepath.Join(root, "schemas", "package")
	if err := filepath.WalkDir(schemaRoot, func(name string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if entry.IsDir() {
			return nil
		}
		data, err := os.ReadFile(name)
		if err != nil {
			return err
		}
		return scanJSONSchemaIdentitySource(name, data)
	}); err != nil {
		t.Fatal(err)
	}

	canaries := map[string]string{
		"direct":   `package canary; func f(packageID string) { if packageID == "vendor.example/game" {} }`,
		"tagless":  `package canary; func f(packageID string) { switch { case packageID == "vendor.example/game": } }`,
		"alias":    `package canary; const selected = "vendor.example/game"; func f(packageID string) { if packageID == selected {} }`,
		"official": `package canary; func f(official bool) { if official {} }`,
		"named":    `package canary; const selected = "` + "rules-" + "residue" + `"`,
	}
	for name, source := range canaries {
		t.Run("scanner canary "+name, func(t *testing.T) {
			if err := scanGoIdentitySource(name+".go", []byte(source)); err == nil {
				t.Fatalf("identity scanner missed %s canary", name)
			}
		})
	}
	if err := scanGoIdentitySource("generic.go", []byte(`package generic; func f(packageID, expectedPackageID string) { if packageID != expectedPackageID {} }`)); err != nil {
		t.Fatalf("identity scanner rejected generic identity comparison: %v", err)
	}
	for name, source := range map[string]string{
		"direct":    `if (packageId === "vendor.example/game") { throw new Error("special") }`,
		"multiline": "if (\n packageId ===\n 'vendor.example/game'\n) { special() }",
	} {
		t.Run("text scanner canary "+name, func(t *testing.T) {
			if err := scanTextIdentitySource(name+".ts", []byte(source)); err == nil {
				t.Fatalf("text identity scanner missed %s canary", name)
			}
		})
	}
	schemaCanary := []byte(`{"if":{"properties":{"package_id":{"const":"vendor.example/game"}}},"then":false}`)
	if err := scanJSONSchemaIdentitySource("canary.schema.json", schemaCanary); err == nil {
		t.Fatal("schema identity scanner missed package_id const canary")
	}
}

func assertInspectionFieldsPreserved(t *testing.T, before, after creator.Inspection, editedCanonical string) {
	t.Helper()
	if len(before.Extensions) != len(after.Extensions) {
		t.Fatalf("Creator extension count changed: %d -> %d", len(before.Extensions), len(after.Extensions))
	}
	for index := range before.Extensions {
		left, right := before.Extensions[index], after.Extensions[index]
		if !reflect.DeepEqual(left.Descriptor, right.Descriptor) || left.Status != right.Status || left.Editable != right.Editable || left.ReadOnlyReason != right.ReadOnlyReason {
			t.Fatalf("Creator extension metadata %d changed", index)
		}
		if left.Descriptor.Namespace == probeNamespace {
			if right.CanonicalJSON != editedCanonical {
				t.Fatalf("edited canonical JSON = %q, want %q", right.CanonicalJSON, editedCanonical)
			}
			continue
		}
		if left.RawPayloadBase64 != right.RawPayloadBase64 || left.RawPayloadSHA256 != right.RawPayloadSHA256 || left.RawPayloadBytes != right.RawPayloadBytes {
			t.Fatalf("Creator opaque fields %d changed", index)
		}
	}
}

type sourceScanError struct {
	path    string
	finding string
}

func (err *sourceScanError) Error() string { return err.path + ": " + err.finding }

func scanGoIdentitySource(name string, source []byte) error {
	parsed, err := parser.ParseFile(token.NewFileSet(), name, source, 0)
	if err != nil {
		return err
	}
	constants := make(map[string]string)
	ast.Inspect(parsed, func(node ast.Node) bool {
		declaration, ok := node.(*ast.GenDecl)
		if !ok || declaration.Tok != token.CONST {
			return true
		}
		for _, specification := range declaration.Specs {
			valueSpec := specification.(*ast.ValueSpec)
			for index, identifier := range valueSpec.Names {
				if index >= len(valueSpec.Values) {
					continue
				}
				if value, ok := stringLiteralValue(valueSpec.Values[index]); ok {
					constants[identifier.Name] = value
				}
			}
		}
		return false
	})
	var finding string
	ast.Inspect(parsed, func(node ast.Node) bool {
		if finding != "" || node == nil {
			return finding == ""
		}
		if literal, ok := node.(*ast.BasicLit); ok && literal.Kind == token.STRING {
			value, err := strconv.Unquote(literal.Value)
			if err != nil {
				finding = "invalid Go string literal"
				return false
			}
			if containsForbiddenIdentityLiteral(value) {
				finding = "identity-specific literal"
				return false
			}
		}
		switch statement := node.(type) {
		case *ast.IfStmt:
			if identityConditionIsSpecial(statement.Cond, constants) {
				finding = "package identity, publisher, or official conditional branch"
				return false
			}
		case *ast.SwitchStmt:
			for _, item := range statement.Body.List {
				clause := item.(*ast.CaseClause)
				for _, expression := range clause.List {
					condition := expression
					if statement.Tag != nil {
						condition = &ast.BinaryExpr{X: statement.Tag, Op: token.EQL, Y: expression}
					}
					if identityConditionIsSpecial(condition, constants) {
						finding = "package identity, publisher, or official switch branch"
						return false
					}
				}
			}
		}
		return true
	})
	if finding != "" {
		return &sourceScanError{path: name, finding: finding}
	}
	return nil
}

func scanTextIdentitySource(name string, source []byte) error {
	lower := strings.ToLower(string(source))
	for _, forbidden := range forbiddenIdentityLiterals() {
		if strings.Contains(lower, forbidden) {
			return &sourceScanError{path: name, finding: "identity-specific literal"}
		}
	}
	compact := strings.Map(func(character rune) rune {
		if unicode.IsSpace(character) {
			return -1
		}
		return character
	}, lower)
	for _, marker := range []string{"package_id", "packageid"} {
		for offset := 0; ; {
			index := strings.Index(compact[offset:], marker)
			if index < 0 {
				break
			}
			index += offset
			end := index + 256
			if end > len(compact) {
				end = len(compact)
			}
			window := compact[index:end]
			if strings.Contains(window, "==") && (strings.Contains(window, `"`) || strings.Contains(window, `'`)) {
				return &sourceScanError{path: name, finding: "identity-specific text branch"}
			}
			offset = index + len(marker)
		}
	}
	for _, line := range strings.Split(lower, "\n") {
		classification := (strings.Contains(line, "official") || strings.Contains(line, "publisher")) &&
			(strings.Contains(line, "if") || strings.Contains(line, "?"))
		if classification {
			return &sourceScanError{path: name, finding: "identity-specific text branch"}
		}
	}
	return nil
}

func scanJSONSchemaIdentitySource(name string, source []byte) error {
	if containsForbiddenIdentityLiteral(string(source)) {
		return &sourceScanError{path: name, finding: "identity-specific literal"}
	}
	var document any
	if err := json.Unmarshal(source, &document); err != nil {
		return err
	}
	if schemaSpecializesPackageID(document, false) {
		return &sourceScanError{path: name, finding: "package_id has a value-specific const or enum"}
	}
	return nil
}

func schemaSpecializesPackageID(value any, packageIDScope bool) bool {
	switch item := value.(type) {
	case []any:
		for _, child := range item {
			if schemaSpecializesPackageID(child, packageIDScope) {
				return true
			}
		}
	case map[string]any:
		for key, child := range item {
			if packageIDScope && (key == "const" || key == "enum") {
				return true
			}
			if key == "properties" {
				if properties, ok := child.(map[string]any); ok {
					for property, schema := range properties {
						if schemaSpecializesPackageID(schema, property == "package_id") {
							return true
						}
					}
					continue
				}
			}
			if schemaSpecializesPackageID(child, packageIDScope) {
				return true
			}
		}
	}
	return false
}

func identityConditionIsSpecial(node ast.Node, constants map[string]string) bool {
	if node == nil {
		return false
	}
	packageIdentity := false
	classification := false
	specificValue := false
	ast.Inspect(node, func(item ast.Node) bool {
		switch value := item.(type) {
		case *ast.Ident:
			lower := strings.ToLower(value.Name)
			if strings.Contains(lower, "packageid") {
				packageIdentity = true
			}
			if strings.Contains(lower, "official") || strings.Contains(lower, "publisher") {
				classification = true
			}
			if constants[value.Name] != "" {
				specificValue = true
			}
		case *ast.BasicLit:
			if literal, ok := stringLiteralValue(value); ok && literal != "" {
				specificValue = true
			}
		}
		return true
	})
	return classification || packageIdentity && specificValue
}

func stringLiteralValue(node ast.Node) (string, bool) {
	literal, ok := node.(*ast.BasicLit)
	if !ok || literal.Kind != token.STRING {
		return "", false
	}
	value, err := strconv.Unquote(literal.Value)
	return value, err == nil
}

func containsForbiddenIdentityLiteral(value string) bool {
	lower := strings.ToLower(value)
	for _, forbidden := range forbiddenIdentityLiterals() {
		if strings.Contains(lower, forbidden) {
			return true
		}
	}
	return false
}

func forbiddenIdentityLiterals() []string {
	return []string{
		"rules-" + "residue",
		"example." + "rules/hidden-cards",
		"first." + "party/probe",
	}
}
