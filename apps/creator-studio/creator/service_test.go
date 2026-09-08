// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package creator

import (
	"bytes"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"sync"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
)

const (
	testNamespace   = "third.party.probe"
	testSchemaPath  = "extensions/third.party.probe/value.schema.json"
	testPayloadPath = "extensions/third.party.probe/value.json"
)

func TestImportInspectAndValidatedEdit(t *testing.T) {
	source := writeTestArchive(t, testV2Files(t, false, 1))
	service := NewService()
	inspection, err := service.ImportArchive(source)
	if err != nil {
		t.Fatal(err)
	}
	if inspection.SourcePath != source || inspection.ConflictToken == "" || inspection.ContentHash == "" {
		t.Fatalf("incomplete inspection: %#v", inspection)
	}
	if len(inspection.Extensions) != 1 || !inspection.Extensions[0].Editable ||
		inspection.Extensions[0].Status != extension.Supported || inspection.Extensions[0].CanonicalJSON != `{"count":1}` {
		t.Fatalf("supported inspection: %#v", inspection.Extensions)
	}

	// Returned values cannot mutate the retained immutable session.
	inspection.Extensions[0].Descriptor.Namespace = "mutated"
	inspection.Extensions[0].CanonicalJSON = "mutated"
	inspection.Extensions = append(inspection.Extensions, ExtensionInspection{})
	again, err := service.Inspect()
	if err != nil {
		t.Fatal(err)
	}
	if len(again.Extensions) != 1 || again.Extensions[0].Descriptor.Namespace != testNamespace ||
		again.Extensions[0].CanonicalJSON != `{"count":1}` {
		t.Fatalf("caller mutation reached service: %#v", again.Extensions)
	}

	if _, err := service.Edit("sha256:"+strings.Repeat("0", 64), testNamespace, `{"count":2}`); !IsCode(err, ErrConflict) {
		t.Fatalf("wrong-token edit error = %v", err)
	}
	if _, err := service.Edit(again.ConflictToken, testNamespace, `{"count":"wrong"}`); !extension.IsCode(err, extension.ErrSchemaValidation) {
		t.Fatalf("invalid edit error = %v", err)
	}
	edited, err := service.Edit(again.ConflictToken, testNamespace, ` { "count" : 2 } `)
	if err != nil {
		t.Fatal(err)
	}
	if edited.ConflictToken != again.ConflictToken || edited.CanonicalJSON != `{"count":2}` || edited.ContentHash == again.ContentHash {
		t.Fatalf("edit result = %#v", edited)
	}
	updated, err := service.Inspect()
	if err != nil {
		t.Fatal(err)
	}
	if updated.Extensions[0].CanonicalJSON != `{"count":2}` || updated.ConflictToken != again.ConflictToken {
		t.Fatalf("updated inspection = %#v", updated)
	}
}

func TestEditRejectsChangedSourceWithStableConflict(t *testing.T) {
	source := writeTestArchive(t, testV2Files(t, false, 1))
	service := NewService()
	inspection, err := service.ImportArchive(source)
	if err != nil {
		t.Fatal(err)
	}
	file, err := os.OpenFile(source, os.O_APPEND|os.O_WRONLY, 0)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := file.Write([]byte("changed")); err != nil {
		_ = file.Close()
		t.Fatal(err)
	}
	if err := file.Close(); err != nil {
		t.Fatal(err)
	}
	if _, err := service.Edit(inspection.ConflictToken, testNamespace, `{"count":2}`); !IsCode(err, ErrConflict) {
		t.Fatalf("changed-source edit error = %v", err)
	}
}

func TestEditRejectsOversizedStringBeforeByteCopy(t *testing.T) {
	source := writeTestArchive(t, testV2Files(t, false, 1))
	service := NewService()
	inspection, err := service.ImportArchive(source)
	if err != nil {
		t.Fatal(err)
	}
	oversized := strings.Repeat("x", extension.MaxPayloadBytes+1)
	runtime.GC()
	var before runtime.MemStats
	runtime.ReadMemStats(&before)
	for range 8 {
		_, err = service.Edit(inspection.ConflictToken, testNamespace, oversized)
	}
	var after runtime.MemStats
	runtime.ReadMemStats(&after)
	if allocated := after.TotalAlloc - before.TotalAlloc; allocated > 1<<20 {
		t.Fatalf("oversized edits allocated %d bytes before rejection", allocated)
	}
	if !extension.IsCode(err, extension.ErrInvalid) {
		t.Fatalf("oversized edit error = %v", err)
	}
}

func TestEditRejectsOversizedNamespaceBeforeSourceReload(t *testing.T) {
	source := writeTestArchive(t, testV2Files(t, false, 1))
	service := NewService()
	inspection, err := service.ImportArchive(source)
	if err != nil {
		t.Fatal(err)
	}
	oversized := strings.Repeat("n", 8<<20)
	runtime.GC()
	var before runtime.MemStats
	runtime.ReadMemStats(&before)
	for range 8 {
		_, err = service.Edit(inspection.ConflictToken, oversized, `{}`)
	}
	var after runtime.MemStats
	runtime.ReadMemStats(&after)
	if allocated := after.TotalAlloc - before.TotalAlloc; allocated > 1<<20 {
		t.Fatalf("oversized namespace edits allocated %d bytes before rejection", allocated)
	}
	var contract *extension.ContractError
	if !errors.As(err, &contract) || contract.Code != extension.ErrInvalid || contract.Namespace != "" ||
		strings.Contains(err.Error(), oversized[:1024]) || len(err.Error()) > 160 {
		t.Fatalf("oversized namespace error = %#v / %v", contract, err)
	}
}

func TestOptionalUnsupportedIsReadOnlyAndRequiredUnsupportedPropagates(t *testing.T) {
	optionalFiles := testV2Files(t, false, 2)
	raw := []byte("{ \"duplicate\" : 1, \"duplicate\" : 2 }\n")
	optionalFiles[testPayloadPath] = raw
	source := writeTestArchive(t, optionalFiles)
	service := NewService()
	inspection, err := service.ImportArchive(source)
	if err != nil {
		t.Fatal(err)
	}
	if len(inspection.Extensions) != 1 || inspection.Extensions[0].Editable ||
		inspection.Extensions[0].Status != extension.ReadOnly || inspection.Extensions[0].CanonicalJSON != "" {
		t.Fatalf("read-only inspection = %#v", inspection.Extensions)
	}
	decoded, err := base64.StdEncoding.DecodeString(inspection.Extensions[0].RawPayloadBase64)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(decoded, raw) || inspection.Extensions[0].RawPayloadBytes != len(raw) {
		t.Fatalf("read-only raw inspection changed: %#v", inspection.Extensions[0])
	}
	if _, err := service.Edit(inspection.ConflictToken, testNamespace, `{}`); !extension.IsCode(err, extension.ErrReadOnly) {
		t.Fatalf("read-only edit error = %v", err)
	}
	target := filepath.Join(t.TempDir(), "readonly.trpgpkg")
	result, err := service.Export(inspection.ConflictToken, target)
	if err != nil {
		t.Fatal(err)
	}
	reloaded, err := archive.ImportFile(target, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	entry, exists := reloaded.Entry(testPayloadPath)
	if !exists || !bytes.Equal(entry.Bytes(), raw) || result.ContentHash != reloaded.ContentHash().String() {
		t.Fatal("read-only export did not preserve exact raw payload")
	}

	requiredSupport := extension.Support{ContractVersion: 2, HostAPIMajor: 1, HostAPIMinor: 0}
	requiredSource := writeTestArchiveWithSupport(t, testV2Files(t, true, 2), requiredSupport)
	if _, err := NewService().ImportArchive(requiredSource); !extension.IsCode(err, extension.ErrRequiredUnsupported) {
		t.Fatalf("required unsupported import error = %v", err)
	}
}

func TestExportNewPathAndDeterministicReimport(t *testing.T) {
	source := writeTestArchive(t, testV2Files(t, false, 1))
	service := NewService()
	inspection, err := service.ImportArchive(source)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := service.Edit(inspection.ConflictToken, testNamespace, `{"count":2}`); err != nil {
		t.Fatal(err)
	}
	directory := t.TempDir()
	firstName := filepath.Join(directory, "first.trpgpkg")
	first, err := service.Export(inspection.ConflictToken, firstName)
	if err != nil {
		t.Fatal(err)
	}
	if first.Path != firstName || first.ConflictToken == "" || first.ConflictToken == inspection.ConflictToken ||
		first.ArchiveHash != first.ConflictToken {
		t.Fatalf("first export = %#v", first)
	}
	loaded, err := archive.ImportFile(firstName, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if loaded.ContentHash().String() != first.ContentHash {
		t.Fatalf("re-import content hash = %s, want %s", loaded.ContentHash(), first.ContentHash)
	}
	secondName := filepath.Join(directory, "second.trpgpkg")
	second, err := service.Export(first.ConflictToken, secondName)
	if err != nil {
		t.Fatal(err)
	}
	firstBytes, err := os.ReadFile(firstName)
	if err != nil {
		t.Fatal(err)
	}
	secondBytes, err := os.ReadFile(secondName)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(firstBytes, secondBytes) || first.ConflictToken != second.ConflictToken {
		t.Fatal("identical session exports are not byte deterministic")
	}
}

func TestExportSamePathAndExistingTargetBinding(t *testing.T) {
	t.Run("same path", func(t *testing.T) {
		source := writeTestArchive(t, testV2Files(t, false, 1))
		service := NewService()
		inspection, err := service.ImportArchive(source)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := service.Edit(inspection.ConflictToken, testNamespace, `{"count":3}`); err != nil {
			t.Fatal(err)
		}
		result, err := service.Export(inspection.ConflictToken, source)
		if err != nil {
			t.Fatal(err)
		}
		if result.Path != source || result.ConflictToken == inspection.ConflictToken {
			t.Fatalf("same-path result = %#v", result)
		}
		if _, err := archive.ImportFile(source, extension.DefaultSupport); err != nil {
			t.Fatal(err)
		}
	})

	t.Run("unrelated existing target", func(t *testing.T) {
		source := writeTestArchive(t, testV2Files(t, false, 1))
		service := NewService()
		inspection, err := service.ImportArchive(source)
		if err != nil {
			t.Fatal(err)
		}
		target := filepath.Join(t.TempDir(), "unrelated.bin")
		original := []byte("must remain untouched")
		if err := os.WriteFile(target, original, 0o600); err != nil {
			t.Fatal(err)
		}
		if _, err := service.Export(inspection.ConflictToken, target); !IsCode(err, ErrTarget) {
			t.Fatalf("unrelated target error = %v", err)
		}
		got, err := os.ReadFile(target)
		if err != nil {
			t.Fatal(err)
		}
		if !bytes.Equal(got, original) {
			t.Fatal("unrelated target was changed")
		}
	})
}

func TestExportFailuresPreservePreexistingTargetAndCleanTemporary(t *testing.T) {
	tests := []struct {
		name string
		ops  exportOperations
	}{
		{
			name: "before replace",
			ops:  exportOperations{beforeReplace: func(string, string) error { return errors.New("injected") }},
		},
		{
			name: "atomic replace",
			ops:  exportOperations{replace: func(string, string) error { return errors.New("injected") }},
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			source := writeTestArchive(t, testV2Files(t, false, 1))
			service := newServiceWithOperations(extension.DefaultSupport, test.ops)
			inspection, err := service.ImportArchive(source)
			if err != nil {
				t.Fatal(err)
			}
			if _, err := service.Edit(inspection.ConflictToken, testNamespace, `{"count":2}`); err != nil {
				t.Fatal(err)
			}
			// A hard link exercises the only authorized existing-target case.
			target := filepath.Join(t.TempDir(), "target.trpgpkg")
			if err := os.Link(source, target); err != nil {
				t.Skipf("hard links unavailable: %v", err)
			}
			before, err := os.ReadFile(target)
			if err != nil {
				t.Fatal(err)
			}
			_, err = service.Export(inspection.ConflictToken, target)
			if err == nil {
				t.Fatal("export unexpectedly succeeded")
			}
			after, readErr := os.ReadFile(target)
			if readErr != nil {
				t.Fatal(readErr)
			}
			if !bytes.Equal(after, before) {
				t.Fatal("failed export changed preexisting target")
			}
			assertNoCreatorTemporaries(t, filepath.Dir(target))
		})
	}
}

func TestExportRevalidatesSourceImmediatelyBeforePublish(t *testing.T) {
	source := writeTestArchive(t, testV2Files(t, false, 1))
	service := newServiceWithOperations(extension.DefaultSupport, exportOperations{
		beforeReplace: func(string, string) error {
			file, err := os.OpenFile(source, os.O_APPEND|os.O_WRONLY, 0)
			if err != nil {
				return err
			}
			if _, err := file.Write([]byte("changed")); err != nil {
				_ = file.Close()
				return err
			}
			return file.Close()
		},
	})
	inspection, err := service.ImportArchive(source)
	if err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(t.TempDir(), "new.trpgpkg")
	if _, err := service.Export(inspection.ConflictToken, target); !IsCode(err, ErrConflict) {
		t.Fatalf("source-conflict export error = %v", err)
	}
	if _, err := os.Lstat(target); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("conflicted export published target: %v", err)
	}
	assertNoCreatorTemporaries(t, filepath.Dir(target))
}

func TestAtomicPublishNeverClobbersConcurrentTarget(t *testing.T) {
	directory := t.TempDir()
	temporary := filepath.Join(directory, "temporary")
	target := filepath.Join(directory, "target")
	if err := os.WriteFile(temporary, []byte("new"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(target, []byte("concurrent"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := atomicPublish(temporary, target); err == nil {
		t.Fatal("no-clobber publish unexpectedly replaced target")
	}
	got, err := os.ReadFile(target)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "concurrent" {
		t.Fatalf("concurrent target = %q", got)
	}
	if _, err := os.Stat(temporary); err != nil {
		t.Fatalf("failed no-clobber publish removed temporary: %v", err)
	}
}

func TestAtomicPublishSuccessConsumesTemporary(t *testing.T) {
	directory := t.TempDir()
	temporary := filepath.Join(directory, "temporary")
	target := filepath.Join(directory, "target")
	if err := os.WriteFile(temporary, []byte("new"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := atomicPublish(temporary, target); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Lstat(temporary); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("published temporary still exists: %v", err)
	}
	got, err := os.ReadFile(target)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "new" {
		t.Fatalf("published target = %q", got)
	}
}

func TestExportRejectsSymlinkAndSpecialTargets(t *testing.T) {
	source := writeTestArchive(t, testV2Files(t, false, 1))
	service := NewService()
	inspection, err := service.ImportArchive(source)
	if err != nil {
		t.Fatal(err)
	}
	directory := t.TempDir()
	referent := filepath.Join(directory, "referent")
	if err := os.WriteFile(referent, []byte("safe"), 0o600); err != nil {
		t.Fatal(err)
	}
	t.Run("symlink target", func(t *testing.T) {
		symlink := filepath.Join(directory, "target-link")
		if err := os.Symlink(referent, symlink); err != nil {
			t.Skipf("symlink unavailable: %v", err)
		}
		if _, err := service.Export(inspection.ConflictToken, symlink); !IsCode(err, ErrTarget) {
			t.Fatalf("symlink target error = %v", err)
		}
		got, err := os.ReadFile(referent)
		if err != nil {
			t.Fatal(err)
		}
		if string(got) != "safe" {
			t.Fatalf("symlink referent changed to %q", got)
		}
	})
	directoryTarget := filepath.Join(directory, "target-directory")
	if err := os.Mkdir(directoryTarget, 0o700); err != nil {
		t.Fatal(err)
	}
	if _, err := service.Export(inspection.ConflictToken, directoryTarget); !IsCode(err, ErrTarget) {
		t.Fatalf("directory target error = %v", err)
	}
	t.Run("symlink parent", func(t *testing.T) {
		symlinkParent := filepath.Join(t.TempDir(), "linked-parent")
		if err := os.Symlink(directory, symlinkParent); err != nil {
			t.Skipf("symlink unavailable: %v", err)
		}
		if _, err := service.Export(inspection.ConflictToken, filepath.Join(symlinkParent, "new")); !IsCode(err, ErrTarget) {
			t.Fatalf("symlink parent error = %v", err)
		}
	})
}

func TestServiceConcurrentInspectionAndEdits(t *testing.T) {
	source := writeTestArchive(t, testV2Files(t, false, 1))
	service := NewService()
	inspection, err := service.ImportArchive(source)
	if err != nil {
		t.Fatal(err)
	}
	var group sync.WaitGroup
	errorsChannel := make(chan error, 32)
	for index := 0; index < 16; index++ {
		group.Add(2)
		go func() {
			defer group.Done()
			value, inspectErr := service.Inspect()
			if inspectErr != nil || len(value.Extensions) != 1 {
				if inspectErr == nil {
					inspectErr = errors.New("inspection lost extension")
				}
				errorsChannel <- inspectErr
			}
		}()
		go func(count int) {
			defer group.Done()
			_, editErr := service.Edit(inspection.ConflictToken, testNamespace, `{"count":`+strconv.Itoa(count)+`}`)
			if editErr != nil {
				errorsChannel <- editErr
			}
		}(index)
	}
	group.Wait()
	close(errorsChannel)
	for err := range errorsChannel {
		t.Error(err)
	}
}

func TestExportDurabilityFailureDoesNotReportSuccess(t *testing.T) {
	source := writeTestArchive(t, testV2Files(t, false, 1))
	service := newServiceWithOperations(extension.DefaultSupport, exportOperations{
		syncDirectory: func(string) error { return errors.New("injected") },
	})
	inspection, err := service.ImportArchive(source)
	if err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(t.TempDir(), "published.trpgpkg")
	if _, err := service.Export(inspection.ConflictToken, target); !IsCode(err, ErrDurability) {
		t.Fatalf("durability error = %v", err)
	}
	if _, err := archive.ImportFile(target, extension.DefaultSupport); err != nil {
		t.Fatalf("committed archive after durability error: %v", err)
	}
}

func assertNoCreatorTemporaries(t *testing.T, directory string) {
	t.Helper()
	names, err := filepath.Glob(filepath.Join(directory, ".creator-studio-*.tmp"))
	if err != nil {
		t.Fatal(err)
	}
	if len(names) != 0 {
		t.Fatalf("temporary files remain: %v", names)
	}
}

func TestNoSessionErrorsAreTyped(t *testing.T) {
	service := NewService()
	if _, err := service.Inspect(); !IsCode(err, ErrNoSession) {
		t.Fatalf("inspect error = %v", err)
	}
	if _, err := service.Edit("token", testNamespace, `{}`); !IsCode(err, ErrNoSession) {
		t.Fatalf("edit error = %v", err)
	}
	if _, err := service.Export("token", filepath.Join(t.TempDir(), "out.trpgpkg")); !IsCode(err, ErrNoSession) {
		t.Fatalf("export error = %v", err)
	}
}

func writeTestArchive(t *testing.T, files map[string][]byte) string {
	t.Helper()
	return writeTestArchiveWithSupport(t, files, extension.DefaultSupport)
}

func writeTestArchiveWithSupport(t *testing.T, files map[string][]byte, support extension.Support) string {
	t.Helper()
	pkg, err := archive.FromFiles(files, testFixtureLock(t), support)
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
	absolute, err := filepath.Abs(name)
	if err != nil {
		t.Fatal(err)
	}
	return absolute
}

func testV2Files(t *testing.T, required bool, contractVersion int) map[string][]byte {
	t.Helper()
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["count"],"properties":{"count":{"type":"integer"}}}`)
	digest := sha256.Sum256(schema)
	manifestText := strings.Replace(string(testFixtureBytes(t, "package.toml")), "schema_version = 1", "schema_version = 2", 1)
	manifestText += "\n[[extensions]]\n" +
		"namespace = \"" + testNamespace + "\"\n" +
		"required = " + map[bool]string{true: "true", false: "false"}[required] + "\n" +
		"contract_version = " + strconv.Itoa(contractVersion) + "\n" +
		"schema_path = \"" + testSchemaPath + "\"\n" +
		"schema_sha256 = \"sha256:" + hex.EncodeToString(digest[:]) + "\"\n" +
		"payload_path = \"" + testPayloadPath + "\"\n" +
		"host_api_major = 1\n" +
		"host_api_min_minor = 0\n" +
		"host_api_max_minor = 0\n"
	return map[string][]byte{
		archive.ManifestPath: []byte(manifestText),
		"package.lock.json":  testFixtureBytes(t, "package.lock.json"),
		testSchemaPath:       schema,
		testPayloadPath:      []byte(" { \"count\" : 1 } \n"),
	}
}

func testFixtureLock(t *testing.T) dependency.ExactLock {
	t.Helper()
	lock, err := dependency.ParseExactLock(testFixtureBytes(t, "package.lock.json"))
	if err != nil {
		t.Fatal(err)
	}
	return lock
}

func testFixtureBytes(t *testing.T, name string) []byte {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "internal", "package", "testdata", name))
	if err != nil {
		t.Fatal(err)
	}
	return data
}
