// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
)

const (
	mainTestNamespace   = "third.party.probe"
	mainTestSchemaPath  = "extensions/third.party.probe/value.schema.json"
	mainTestPayloadPath = "extensions/third.party.probe/value.json"
)

func TestIdentityReportsRunningBinaryAndEmbeddedFields(t *testing.T) {
	oldVersion, oldCommit, oldTree, oldCommand := binaryVersion, platformCommit, platformTree, buildCommand
	binaryVersion = "test-version"
	platformCommit = strings.Repeat("1", 40)
	platformTree = strings.Repeat("2", 40)
	buildCommand = "go build -trimpath ./apps/creator-studio"
	t.Cleanup(func() {
		binaryVersion, platformCommit, platformTree, buildCommand = oldVersion, oldCommit, oldTree, oldCommand
	})
	var stdout, stderr bytes.Buffer
	if code := run([]string{"identity"}, &stdout, &stderr); code != 0 {
		t.Fatalf("identity exit = %d, stderr = %q", code, stderr.String())
	}
	var identity binaryIdentity
	if err := json.Unmarshal(stdout.Bytes(), &identity); err != nil {
		t.Fatal(err)
	}
	wantDigest, err := executableSHA256()
	if err != nil {
		t.Fatal(err)
	}
	if identity.BinarySHA256 != wantDigest || identity.BinaryVersion != binaryVersion ||
		identity.PlatformCommit != platformCommit || identity.PlatformTree != platformTree || identity.BuildCommand != buildCommand {
		t.Fatalf("identity = %#v, digest = %s", identity, wantDigest)
	}
}

func TestBuiltBinaryIdentityMatchesExactExecutableBytes(t *testing.T) {
	name := "creator-studio"
	if runtime.GOOS == "windows" {
		name += ".exe"
	}
	binary := filepath.Join(t.TempDir(), name)
	command := exec.Command(
		"go", "build", "-trimpath", "-buildvcs=false",
		"-ldflags", "-X main.platformCommit="+strings.Repeat("a", 40)+
			" -X main.platformTree="+strings.Repeat("b", 40)+
			" -X main.binaryVersion=test-version -X main.buildCommand=test-build",
		"-o", binary, ".",
	)
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("build creator-studio: %v\n%s", err, output)
	}
	output, err := exec.Command(binary, "identity").Output()
	if err != nil {
		t.Fatal(err)
	}
	var identity binaryIdentity
	if err := json.Unmarshal(output, &identity); err != nil {
		t.Fatal(err)
	}
	file, err := os.Open(binary)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.New()
	if _, err := io.Copy(digest, file); err != nil {
		_ = file.Close()
		t.Fatal(err)
	}
	if err := file.Close(); err != nil {
		t.Fatal(err)
	}
	want := "sha256:" + hex.EncodeToString(digest.Sum(nil))
	if identity.BinarySHA256 != want || identity.PlatformCommit != strings.Repeat("a", 40) ||
		identity.PlatformTree != strings.Repeat("b", 40) || identity.BinaryVersion != "test-version" || identity.BuildCommand != "test-build" {
		t.Fatalf("built identity = %#v, want digest %s", identity, want)
	}
}

func TestHeadlessInspectEditValidateExportReimport(t *testing.T) {
	source := writeMainTestArchive(t, mainTestV2Files(t, false, 1), extension.DefaultSupport)
	var inspectOut, inspectErr bytes.Buffer
	if code := run([]string{"extension", "inspect", "--archive", source}, &inspectOut, &inspectErr); code != 0 {
		t.Fatalf("inspect exit = %d, stderr = %q, stdout = %q", code, inspectErr.String(), inspectOut.String())
	}
	var inspected commandResult
	if err := json.Unmarshal(inspectOut.Bytes(), &inspected); err != nil {
		t.Fatal(err)
	}
	if inspected.Operation != "extension.inspect" || inspected.Inspection == nil ||
		len(inspected.Inspection.Extensions) != 1 || inspected.Inspection.ConflictToken == "" {
		t.Fatalf("inspect result = %#v", inspected)
	}
	assertPhaseNames(t, inspected.Phases, "import", "inspect")

	directory := t.TempDir()
	jsonName := filepath.Join(directory, "replacement.json")
	if err := os.WriteFile(jsonName, []byte(" { \"count\" : 7 } \n"), 0o600); err != nil {
		t.Fatal(err)
	}
	outputName := filepath.Join(directory, "edited.trpgpkg")
	var editOut, editErr bytes.Buffer
	code := run([]string{
		"extension", "edit",
		"--archive", source,
		"--namespace", mainTestNamespace,
		"--json-file", jsonName,
		"--output", outputName,
		"--conflict-token", inspected.Inspection.ConflictToken,
	}, &editOut, &editErr)
	if code != 0 {
		t.Fatalf("edit exit = %d, stderr = %q, stdout = %q", code, editErr.String(), editOut.String())
	}
	var edited commandResult
	if err := json.Unmarshal(editOut.Bytes(), &edited); err != nil {
		t.Fatal(err)
	}
	assertPhaseNames(t, edited.Phases, "import", "inspect", "edit", "validate", "export", "reimport")
	if edited.Edit == nil || edited.Edit.CanonicalJSON != `{"count":7}` || edited.Export == nil || edited.Reimport == nil ||
		edited.Export.ConflictToken != edited.Reimport.ConflictToken || edited.Export.Path != outputName {
		t.Fatalf("edit result = %#v", edited)
	}
	reloaded, err := archive.ImportFile(outputName, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	payload, exists := reloaded.Entry(mainTestPayloadPath)
	if !exists || string(payload.Bytes()) != `{"count":7}` {
		t.Fatalf("reloaded payload = %q, exists = %v", payload.Bytes(), exists)
	}
}

func TestHeadlessEditReportsConflictAndSchemaValidation(t *testing.T) {
	source := writeMainTestArchive(t, mainTestV2Files(t, false, 1), extension.DefaultSupport)
	pkg, err := archive.ImportFile(source, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	token, exists := pkg.SourceArchiveHash()
	if !exists {
		t.Fatal("fixture has no source token")
	}
	directory := t.TempDir()
	jsonName := filepath.Join(directory, "replacement.json")
	if err := os.WriteFile(jsonName, []byte(`{"count":"wrong"}`), 0o600); err != nil {
		t.Fatal(err)
	}
	base := []string{
		"extension", "edit", "--archive", source, "--namespace", mainTestNamespace,
		"--json-file", jsonName, "--output", filepath.Join(directory, "output.trpgpkg"), "--conflict-token",
	}
	for _, test := range []struct {
		name  string
		token string
		code  string
		phase string
	}{
		{name: "conflict", token: "sha256:" + strings.Repeat("0", 64), code: "ERR_CREATOR_CONFLICT", phase: "edit"},
		{name: "schema", token: token.String(), code: "ERR_EXTENSION_SCHEMA_VALIDATION", phase: "validate"},
	} {
		t.Run(test.name, func(t *testing.T) {
			var stdout, stderr bytes.Buffer
			if code := run(append(append([]string(nil), base...), test.token), &stdout, &stderr); code != 1 {
				t.Fatalf("exit = %d, stderr = %q, stdout = %q", code, stderr.String(), stdout.String())
			}
			var result commandResult
			if err := json.Unmarshal(stdout.Bytes(), &result); err != nil {
				t.Fatal(err)
			}
			if result.Failure == nil || result.Failure.Code != test.code ||
				len(result.Phases) != 3 || result.Phases[2].Name != test.phase || result.Phases[2].Status != "error" {
				t.Fatalf("failure result = %#v", result)
			}
		})
	}
}

func TestHeadlessJSONInputIsBoundedAndNeverFollowsSymlink(t *testing.T) {
	directory := t.TempDir()
	realName := filepath.Join(directory, "real.json")
	if err := os.WriteFile(realName, []byte(`{}`), 0o600); err != nil {
		t.Fatal(err)
	}
	t.Run("symlink", func(t *testing.T) {
		linkName := filepath.Join(directory, "link.json")
		if err := os.Symlink(realName, linkName); err != nil {
			t.Skipf("symlink unavailable: %v", err)
		}
		if _, err := readBoundedRegularFile(linkName, extension.MaxPayloadBytes); err == nil || !strings.Contains(err.Error(), "regular file") {
			t.Fatalf("symlink JSON input error = %v", err)
		}
	})
	tooLarge := filepath.Join(directory, "large.json")
	file, err := os.Create(tooLarge)
	if err != nil {
		t.Fatal(err)
	}
	if err := file.Truncate(extension.MaxPayloadBytes + 1); err != nil {
		_ = file.Close()
		t.Fatal(err)
	}
	if err := file.Close(); err != nil {
		t.Fatal(err)
	}
	if _, err := readBoundedRegularFile(tooLarge, extension.MaxPayloadBytes); err == nil || !strings.Contains(err.Error(), "exceeds") {
		t.Fatalf("oversized JSON input error = %v", err)
	}
}

func assertPhaseNames(t *testing.T, phases []commandPhase, names ...string) {
	t.Helper()
	if len(phases) != len(names) {
		t.Fatalf("phases = %#v, want %v", phases, names)
	}
	for index, name := range names {
		if phases[index].Name != name || phases[index].Status != "ok" {
			t.Fatalf("phase %d = %#v, want %s/ok", index, phases[index], name)
		}
	}
}

func writeMainTestArchive(t *testing.T, files map[string][]byte, support extension.Support) string {
	t.Helper()
	lock, err := dependency.ParseExactLock(mainTestFixtureBytes(t, "package.lock.json"))
	if err != nil {
		t.Fatal(err)
	}
	pkg, err := archive.FromFiles(files, lock, support)
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err := pkg.Export()
	if err != nil {
		t.Fatal(err)
	}
	name := filepath.Join(t.TempDir(), "source.trpgpkg")
	if err := os.WriteFile(name, snapshot.Bytes(), 0o600); err != nil {
		t.Fatal(err)
	}
	return name
}

func mainTestV2Files(t *testing.T, required bool, contractVersion int) map[string][]byte {
	t.Helper()
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["count"],"properties":{"count":{"type":"integer"}}}`)
	digest := sha256.Sum256(schema)
	manifestText := strings.Replace(string(mainTestFixtureBytes(t, "package.toml")), "schema_version = 1", "schema_version = 2", 1)
	manifestText += "\n[[extensions]]\n" +
		"namespace = \"" + mainTestNamespace + "\"\n" +
		"required = " + map[bool]string{true: "true", false: "false"}[required] + "\n" +
		"contract_version = " + strconv.Itoa(contractVersion) + "\n" +
		"schema_path = \"" + mainTestSchemaPath + "\"\n" +
		"schema_sha256 = \"sha256:" + hex.EncodeToString(digest[:]) + "\"\n" +
		"payload_path = \"" + mainTestPayloadPath + "\"\n" +
		"host_api_major = 1\n" +
		"host_api_min_minor = 0\n" +
		"host_api_max_minor = 0\n"
	return map[string][]byte{
		archive.ManifestPath: []byte(manifestText),
		"package.lock.json":  mainTestFixtureBytes(t, "package.lock.json"),
		mainTestSchemaPath:   schema,
		mainTestPayloadPath:  []byte(`{"count":1}`),
	}
}

func mainTestFixtureBytes(t *testing.T, name string) []byte {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("..", "..", "internal", "package", "testdata", name))
	if err != nil {
		t.Fatal(err)
	}
	return data
}
