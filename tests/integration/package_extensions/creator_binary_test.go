// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package package_extensions_test

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"runtime"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/apps/creator-studio/creator"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
)

const (
	creatorAcceptanceVersion   = "0.1.0-m1"
	creatorAcceptanceCommitEnv = "TRPG_CREATOR_ACCEPTANCE_COMMIT"
	creatorAcceptanceTreeEnv   = "TRPG_CREATOR_ACCEPTANCE_TREE"
	creatorBinaryPlaceholder   = "<CREATOR_BINARY>"
)

type creatorAcceptanceBinding struct {
	commit           string
	tree             string
	evidenceEligible bool
}

type creatorBinaryIdentity struct {
	PlatformCommit string `json:"platform_commit"`
	PlatformTree   string `json:"platform_tree"`
	BinaryVersion  string `json:"binary_version"`
	BuildCommand   string `json:"build_command"`
	BinarySHA256   string `json:"binary_sha256"`
}

type creatorCommandPhase struct {
	Name   string `json:"name"`
	Status string `json:"status"`
}

type creatorCommandFailure struct {
	Code   string `json:"code"`
	Detail string `json:"detail"`
}

type creatorCommandResult struct {
	Operation  string                 `json:"operation"`
	Identity   creatorBinaryIdentity  `json:"identity"`
	Phases     []creatorCommandPhase  `json:"phases"`
	Inspection *creator.Inspection    `json:"inspection"`
	Edit       *creator.EditResult    `json:"edit"`
	Export     *creator.ExportResult  `json:"export"`
	Reimport   *creator.Inspection    `json:"reimport"`
	Failure    *creatorCommandFailure `json:"error"`
}

type creatorExecution struct {
	Argv     []string `json:"argv"`
	Stdout   string   `json:"stdout"`
	Stderr   string   `json:"stderr"`
	ExitCode int      `json:"exit_code"`
}

type creatorAcceptanceEvidence struct {
	EvidenceEligible bool                  `json:"evidence_eligible"`
	BuildRecipe      string                `json:"build_recipe"`
	ActualBuildArgv  []string              `json:"actual_build_argv"`
	Identity         creatorBinaryIdentity `json:"identity"`
	SourceHash       string                `json:"source_hash"`
	FirstOutputHash  string                `json:"first_output_hash"`
	SecondOutputHash string                `json:"second_output_hash"`
	Executions       []creatorExecution    `json:"executions"`
}

func TestP10CreatorBinaryEditsValidatesExportsReimportsAndRepeats(t *testing.T) {
	if runtime.GOOS != "linux" && runtime.GOOS != "windows" {
		t.Skip("Creator Studio acceptance binary is required on Linux and Windows")
	}
	root := repositoryRoot(t)
	binding := resolveCreatorAcceptanceBinding(t, root)
	binary, recipe, actualBuildArgv := buildCreatorAcceptanceBinary(t, root, binding)
	identityExecution := executeCreatorBinary(binary, "identity")
	if identityExecution.ExitCode != 0 {
		t.Fatalf("identity exit = %d, stderr = %q", identityExecution.ExitCode, identityExecution.Stderr)
	}
	var identity creatorBinaryIdentity
	decodeCreatorJSON(t, identityExecution.Stdout, &identity)
	wantBinaryHash := sha256File(t, binary)
	assertCreatorIdentity(t, identity, binding, recipe, wantBinaryHash)

	pkg := buildPackage(t, v2Files(t, extensionFixture{
		namespace: probeNamespace, required: true, contractVersion: 1,
	}))
	sourceSnapshot := exportPackage(t, pkg)
	directory := t.TempDir()
	source := filepath.Join(directory, "source.trpgpkg")
	if err := os.WriteFile(source, sourceSnapshot.Bytes(), 0o600); err != nil {
		t.Fatal(err)
	}

	inspectExecution := executeCreatorBinary(binary, "extension", "inspect", "--archive", source)
	inspect := requireCreatorCommandSuccess(t, inspectExecution, binding, recipe, wantBinaryHash)
	if inspect.Operation != "extension.inspect" || inspect.Inspection == nil || len(inspect.Inspection.Extensions) != 1 ||
		inspect.Inspection.ConflictToken != sourceSnapshot.Hash().String() || inspect.Inspection.Extensions[0].Descriptor.Namespace != probeNamespace {
		t.Fatalf("Creator binary inspect result = %#v", inspect)
	}
	assertCreatorPhases(t, inspect.Phases, "import", "inspect")

	validJSON := filepath.Join(directory, "valid.json")
	if err := os.WriteFile(validJSON, []byte(" { \"count\" : 7 } \n"), 0o600); err != nil {
		t.Fatal(err)
	}
	invalidJSON := filepath.Join(directory, "invalid.json")
	if err := os.WriteFile(invalidJSON, []byte(`{"count":"wrong"}`), 0o600); err != nil {
		t.Fatal(err)
	}
	invalidOutput := filepath.Join(directory, "must-not-exist.trpgpkg")
	invalidExecution := executeCreatorBinary(binary, creatorEditArgs(source, invalidJSON, invalidOutput, inspect.Inspection.ConflictToken)...)
	if invalidExecution.ExitCode == 0 {
		t.Fatal("schema-invalid Creator edit unexpectedly succeeded")
	}
	var invalidResult creatorCommandResult
	decodeCreatorJSON(t, invalidExecution.Stdout, &invalidResult)
	assertCreatorIdentity(t, invalidResult.Identity, binding, recipe, wantBinaryHash)
	if invalidResult.Failure == nil || invalidResult.Failure.Code != string(extension.ErrSchemaValidation) {
		t.Fatalf("schema validation failure = %#v", invalidResult)
	}
	assertCreatorFailedPhase(t, invalidResult.Phases, "validate")
	if _, err := os.Lstat(invalidOutput); !os.IsNotExist(err) {
		t.Fatalf("schema-invalid edit created output: %v", err)
	}

	firstOutput := filepath.Join(directory, "first.trpgpkg")
	secondOutput := filepath.Join(directory, "second.trpgpkg")
	firstExecution := executeCreatorBinary(binary, creatorEditArgs(source, validJSON, firstOutput, inspect.Inspection.ConflictToken)...)
	secondExecution := executeCreatorBinary(binary, creatorEditArgs(source, validJSON, secondOutput, inspect.Inspection.ConflictToken)...)
	first := requireCreatorCommandSuccess(t, firstExecution, binding, recipe, wantBinaryHash)
	second := requireCreatorCommandSuccess(t, secondExecution, binding, recipe, wantBinaryHash)
	for _, result := range []creatorCommandResult{first, second} {
		if result.Operation != "extension.edit" || result.Edit == nil || result.Edit.CanonicalJSON != `{"count":7}` ||
			result.Export == nil || result.Reimport == nil || result.Export.ConflictToken != result.Reimport.ConflictToken ||
			result.Export.ContentHash != result.Reimport.ContentHash {
			t.Fatalf("Creator binary roundtrip result = %#v", result)
		}
		assertCreatorPhases(t, result.Phases, "import", "inspect", "edit", "validate", "export", "reimport")
	}
	firstBytes, err := os.ReadFile(firstOutput)
	if err != nil {
		t.Fatal(err)
	}
	secondBytes, err := os.ReadFile(secondOutput)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(firstBytes, secondBytes) || first.Export.ArchiveHash != second.Export.ArchiveHash {
		t.Fatal("two real Creator binary runs produced different archive bytes or hashes")
	}
	reloaded, err := archive.ImportFile(firstOutput, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	payload, exists := reloaded.Entry(probePayloadPath)
	if !exists || string(payload.Bytes()) != `{"count":7}` || reloaded.ContentHash().String() != first.Export.ContentHash {
		t.Fatalf("real Creator output payload/content hash = %q/%s", payload.Bytes(), reloaded.ContentHash())
	}

	evidence := creatorAcceptanceEvidence{
		EvidenceEligible: binding.evidenceEligible,
		BuildRecipe:      recipe,
		ActualBuildArgv:  actualBuildArgv,
		Identity:         identity,
		SourceHash:       sourceSnapshot.Hash().String(),
		FirstOutputHash:  first.Export.ArchiveHash,
		SecondOutputHash: second.Export.ArchiveHash,
		Executions:       []creatorExecution{identityExecution, inspectExecution, invalidExecution, firstExecution, secondExecution},
	}
	encodedEvidence, err := json.Marshal(evidence)
	if err != nil {
		t.Fatal(err)
	}
	label, err := creatorAcceptanceEvidenceLabel(root, binding)
	if err != nil {
		t.Fatalf("final Creator acceptance binding verification: %v", err)
	}
	t.Logf("%s %s", label, encodedEvidence)
}

func TestCreatorAcceptanceBindingRejectsPostCheckDrift(t *testing.T) {
	for _, test := range []struct {
		name   string
		mutate func(*testing.T, string)
	}{
		{
			name: "tracked",
			mutate: func(t *testing.T, root string) {
				t.Helper()
				if err := os.WriteFile(filepath.Join(root, "tracked.txt"), []byte("changed\n"), 0o600); err != nil {
					t.Fatal(err)
				}
			},
		},
		{
			name: "untracked",
			mutate: func(t *testing.T, root string) {
				t.Helper()
				if err := os.WriteFile(filepath.Join(root, "untracked.txt"), []byte("new\n"), 0o600); err != nil {
					t.Fatal(err)
				}
			},
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			root, binding := newCreatorBindingRepository(t)
			if err := verifyCreatorAcceptanceBinding(root, binding); err != nil {
				t.Fatalf("clean binding rejected: %v", err)
			}
			test.mutate(t, root)
			label, err := creatorAcceptanceEvidenceLabel(root, binding)
			if err == nil || label != "" {
				t.Fatalf("dirty binding produced label %q, error %v", label, err)
			}
		})
	}
}

func resolveCreatorAcceptanceBinding(t *testing.T, root string) creatorAcceptanceBinding {
	t.Helper()
	commit, commitSet := os.LookupEnv(creatorAcceptanceCommitEnv)
	tree, treeSet := os.LookupEnv(creatorAcceptanceTreeEnv)
	if !commitSet && !treeSet {
		return creatorAcceptanceBinding{commit: strings.Repeat("a", 40), tree: strings.Repeat("b", 40)}
	}
	if !commitSet || !treeSet || !isLowerHex40(commit) || !isLowerHex40(tree) {
		t.Fatalf("%s and %s must both be exact lower-case 40-hex values", creatorAcceptanceCommitEnv, creatorAcceptanceTreeEnv)
	}
	binding := creatorAcceptanceBinding{commit: commit, tree: tree, evidenceEligible: true}
	if err := verifyCreatorAcceptanceBinding(root, binding); err != nil {
		t.Fatal(err)
	}
	return binding
}

func buildCreatorAcceptanceBinary(t *testing.T, root string, binding creatorAcceptanceBinding) (string, string, []string) {
	t.Helper()
	tags := "production"
	if runtime.GOOS == "linux" {
		tags = "production,webkit2_41"
	}
	frontendArgv := []string{"pnpm", "--dir", "apps/creator-studio/frontend", "build"}
	baseGoArgv := []string{"go", "build", "-trimpath", "-buildvcs=false", "-tags", tags, "-o", creatorBinaryPlaceholder, "./apps/creator-studio"}
	recipe := canonicalCreatorBuildRecipe(t, frontendArgv, baseGoArgv)
	frontend := exec.Command(frontendArgv[0], frontendArgv[1:]...)
	frontend.Dir = root
	if output, err := frontend.CombinedOutput(); err != nil {
		t.Fatalf("build Creator frontend: %v\n%s", err, output)
	}
	requireCreatorAcceptanceBinding(t, root, binding, "after frontend build")
	binaryName := "creator-studio"
	if runtime.GOOS == "windows" {
		binaryName += ".exe"
	}
	binary := filepath.Join(t.TempDir(), binaryName)
	ldflags := strings.Join([]string{
		"-X", "main.platformCommit=" + binding.commit,
		"-X", "main.platformTree=" + binding.tree,
		"-X", "main.binaryVersion=" + creatorAcceptanceVersion,
		"-X", "main.buildCommand=" + recipe,
	}, " ")
	goArgs := []string{
		"build", "-trimpath", "-buildvcs=false", "-tags", tags,
		"-ldflags", ldflags, "-o", binary, "./apps/creator-studio",
	}
	actualArgv := append([]string{"go"}, goArgs...)
	if normalized := normalizeCreatorBuildArgv(actualArgv, binary); !reflect.DeepEqual(normalized, baseGoArgv) {
		t.Fatalf("normalized build argv = %q, canonical = %q", normalized, baseGoArgv)
	}
	for _, injected := range []string{
		"main.platformCommit=" + binding.commit,
		"main.platformTree=" + binding.tree,
		"main.binaryVersion=" + creatorAcceptanceVersion,
		"main.buildCommand=" + recipe,
	} {
		if !strings.Contains(ldflags, injected) {
			t.Fatalf("actual ldflags omit identity injection %q", injected)
		}
	}
	command := exec.Command("go", goArgs...)
	command.Dir = root
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("build Creator acceptance binary: %v\n%s", err, output)
	}
	requireCreatorAcceptanceBinding(t, root, binding, "after binary build")
	return binary, recipe, actualArgv
}

func requireCreatorAcceptanceBinding(t *testing.T, root string, binding creatorAcceptanceBinding, phase string) {
	t.Helper()
	if !binding.evidenceEligible {
		return
	}
	if err := verifyCreatorAcceptanceBinding(root, binding); err != nil {
		t.Fatalf("Creator acceptance binding changed %s: %v", phase, err)
	}
}

func creatorAcceptanceEvidenceLabel(root string, binding creatorAcceptanceBinding) (string, error) {
	if !binding.evidenceEligible {
		return "CREATOR_ACCEPTANCE_PROBE", nil
	}
	if err := verifyCreatorAcceptanceBinding(root, binding); err != nil {
		return "", err
	}
	return "CREATOR_ACCEPTANCE_RESULT", nil
}

func verifyCreatorAcceptanceBinding(root string, binding creatorAcceptanceBinding) error {
	status, err := creatorGitValue(root, "status", "--porcelain", "--untracked-files=all")
	if err != nil {
		return err
	}
	if status != "" {
		return fmt.Errorf("Creator acceptance evidence requires a clean worktree, status = %q", status)
	}
	actualCommit, err := creatorGitValue(root, "rev-parse", "HEAD^{commit}")
	if err != nil {
		return err
	}
	actualTree, err := creatorGitValue(root, "rev-parse", "HEAD^{tree}")
	if err != nil {
		return err
	}
	if binding.commit != actualCommit || binding.tree != actualTree {
		return fmt.Errorf("Creator acceptance binding = %s/%s, checkout = %s/%s",
			binding.commit, binding.tree, actualCommit, actualTree)
	}
	return nil
}

func canonicalCreatorBuildRecipe(t *testing.T, frontendArgv, goArgv []string) string {
	t.Helper()
	for _, argument := range append(append([]string(nil), frontendArgv...), goArgv...) {
		if argument == "" || strings.ContainsAny(argument, "|\r\n") {
			t.Fatalf("Creator build argv cannot be represented canonically: %q", argument)
		}
	}
	return "argv-v1:" + strings.Join(frontendArgv, "|") + "||" + strings.Join(goArgv, "|")
}

func normalizeCreatorBuildArgv(actual []string, binary string) []string {
	result := make([]string, 0, len(actual))
	for index := 0; index < len(actual); index++ {
		if actual[index] == "-ldflags" && index+1 < len(actual) {
			index++
			continue
		}
		if actual[index] == binary {
			result = append(result, creatorBinaryPlaceholder)
			continue
		}
		result = append(result, actual[index])
	}
	return result
}

func executeCreatorBinary(binary string, args ...string) creatorExecution {
	command := exec.Command(binary, args...)
	var stdout, stderr bytes.Buffer
	command.Stdout = &stdout
	command.Stderr = &stderr
	err := command.Run()
	exitCode := 0
	if err != nil {
		exitCode = -1
		var exitError *exec.ExitError
		if errors.As(err, &exitError) {
			exitCode = exitError.ExitCode()
		}
	}
	return creatorExecution{
		Argv: append([]string{binary}, args...), Stdout: stdout.String(), Stderr: stderr.String(), ExitCode: exitCode,
	}
}

func creatorEditArgs(source, jsonName, output, token string) []string {
	return []string{
		"extension", "edit", "--archive", source, "--namespace", probeNamespace,
		"--json-file", jsonName, "--output", output, "--conflict-token", token,
	}
}

func requireCreatorCommandSuccess(t *testing.T, execution creatorExecution, binding creatorAcceptanceBinding, recipe, binaryHash string) creatorCommandResult {
	t.Helper()
	if execution.ExitCode != 0 {
		t.Fatalf("Creator command exit = %d, stderr = %q, stdout = %q", execution.ExitCode, execution.Stderr, execution.Stdout)
	}
	var result creatorCommandResult
	decodeCreatorJSON(t, execution.Stdout, &result)
	if result.Failure != nil {
		t.Fatalf("Creator command returned failure: %#v", result.Failure)
	}
	assertCreatorIdentity(t, result.Identity, binding, recipe, binaryHash)
	return result
}

func assertCreatorIdentity(t *testing.T, identity creatorBinaryIdentity, binding creatorAcceptanceBinding, recipe, binaryHash string) {
	t.Helper()
	if identity.PlatformCommit != binding.commit || identity.PlatformTree != binding.tree ||
		identity.BinaryVersion != creatorAcceptanceVersion || identity.BuildCommand != recipe || identity.BinarySHA256 != binaryHash {
		t.Fatalf("Creator binary identity = %#v, want commit/tree/version/recipe/hash %s/%s/%s/%q/%s",
			identity, binding.commit, binding.tree, creatorAcceptanceVersion, recipe, binaryHash)
	}
}

func assertCreatorPhases(t *testing.T, phases []creatorCommandPhase, names ...string) {
	t.Helper()
	if len(phases) != len(names) {
		t.Fatalf("Creator phases = %#v, want %v", phases, names)
	}
	for index, name := range names {
		if phases[index].Name != name || phases[index].Status != "ok" {
			t.Fatalf("Creator phase %d = %#v, want %s/ok", index, phases[index], name)
		}
	}
}

func assertCreatorFailedPhase(t *testing.T, phases []creatorCommandPhase, name string) {
	t.Helper()
	if len(phases) == 0 || phases[len(phases)-1].Name != name || phases[len(phases)-1].Status != "error" {
		t.Fatalf("Creator failure phases = %#v, want final %s/error", phases, name)
	}
}

func decodeCreatorJSON(t *testing.T, text string, target any) {
	t.Helper()
	decoder := json.NewDecoder(strings.NewReader(text))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(target); err != nil {
		t.Fatalf("decode Creator JSON: %v\n%s", err, text)
	}
	if decoder.Decode(new(any)) != io.EOF {
		t.Fatalf("Creator JSON has a trailing value: %s", text)
	}
}

func sha256File(t *testing.T, name string) string {
	t.Helper()
	file, err := os.Open(name)
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
	return "sha256:" + hex.EncodeToString(digest.Sum(nil))
}

func newCreatorBindingRepository(t *testing.T) (string, creatorAcceptanceBinding) {
	t.Helper()
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "tracked.txt"), []byte("original\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	for _, args := range [][]string{
		{"init", "--quiet"},
		{"add", "tracked.txt"},
		{"-c", "user.name=Creator Acceptance", "-c", "user.email=creator@example.invalid", "commit", "--quiet", "-m", "baseline"},
	} {
		if _, err := creatorGitValue(root, args...); err != nil {
			t.Fatal(err)
		}
	}
	commit, err := creatorGitValue(root, "rev-parse", "HEAD^{commit}")
	if err != nil {
		t.Fatal(err)
	}
	tree, err := creatorGitValue(root, "rev-parse", "HEAD^{tree}")
	if err != nil {
		t.Fatal(err)
	}
	return root, creatorAcceptanceBinding{commit: commit, tree: tree, evidenceEligible: true}
}

func creatorGitValue(root string, args ...string) (string, error) {
	command := exec.Command("git", args...)
	command.Dir = root
	output, err := command.CombinedOutput()
	if err != nil {
		return "", fmt.Errorf("git %s: %w: %s", strings.Join(args, " "), err, bytes.TrimSpace(output))
	}
	return strings.TrimSpace(string(output)), nil
}

func isLowerHex40(value string) bool {
	if len(value) != 40 || strings.ToLower(value) != value {
		return false
	}
	_, err := hex.DecodeString(value)
	return err == nil
}
