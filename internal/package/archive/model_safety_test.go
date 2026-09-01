// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package archive

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"runtime"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/manifest"
)

func TestFromFilesRejectsLimitsBeforeCloningCallerBytes(t *testing.T) {
	large := make([]byte, MaxEntryExpandedBytes+1)
	allocations := testing.AllocsPerRun(5, func() {
		if _, err := FromFiles(map[string][]byte{"large": large}, dependency.ExactLock{}, extension.DefaultSupport); err == nil {
			panic("oversized entry succeeded")
		}
	})
	if allocations > 32 {
		t.Fatalf("oversized pre-clone allocations = %.0f", allocations)
	}

	tooMany := make(map[string][]byte, MaxArchiveEntries-1)
	for index := 0; index < MaxArchiveEntries-1; index++ {
		tooMany[fmt.Sprintf("entry-%04d", index)] = nil
	}
	allocations = testing.AllocsPerRun(5, func() {
		if _, err := FromFiles(tooMany, dependency.ExactLock{}, extension.DefaultSupport); err == nil {
			panic("excess entry count succeeded")
		}
	})
	if allocations > 32 {
		t.Fatalf("entry-count pre-clone allocations = %.0f", allocations)
	}
}

func TestFromFilesReservesExactEnvelopeBeforeCloningCombinedLimit(t *testing.T) {
	files := v1Files(t)
	var baseBytes uint64
	for _, data := range files {
		baseBytes += uint64(len(data))
	}
	remaining := uint64(MaxArchiveExpandedBytes) - baseBytes
	backing := make([]byte, int(remaining))
	offset := 0
	for index := 0; remaining != 0; index++ {
		length := remaining
		if length > MaxEntryExpandedBytes {
			length = MaxEntryExpandedBytes
		}
		files[fmt.Sprintf("content/fill-%02d.bin", index)] = backing[offset : offset+int(length)]
		offset += int(length)
		remaining -= length
	}

	runtime.GC()
	var before runtime.MemStats
	var after runtime.MemStats
	runtime.ReadMemStats(&before)
	_, err := FromFiles(files, fixtureLock(t), extension.DefaultSupport)
	runtime.ReadMemStats(&after)
	if err == nil || !strings.Contains(err.Error(), "content and canonical envelope") {
		t.Fatalf("combined raw/envelope limit result = %v", err)
	}
	if allocated := after.TotalAlloc - before.TotalAlloc; allocated > 16<<20 {
		t.Fatalf("combined limit allocated %d bytes before rejection; caller content was cloned", allocated)
	}
}

func TestCanonicalEnvelopeEntriesHaveIndependentPrecloneLimits(t *testing.T) {
	boundary := make([]byte, MaxEntryExpandedBytes)
	if err := validateCanonicalEnvelopeEntries(boundary, boundary); err != nil {
		t.Fatalf("boundary envelope entries: %v", err)
	}
	oversized := append(boundary, 0)
	if err := validateCanonicalEnvelopeEntries(oversized, nil); err == nil || !strings.Contains(err.Error(), "platform lock") {
		t.Fatalf("oversized lock result = %v", err)
	}
	if err := validateCanonicalEnvelopeEntries(nil, oversized); err == nil || !strings.Contains(err.Error(), "artifact identity") {
		t.Fatalf("oversized artifact result = %v", err)
	}
}

func TestCanonicalPayloadAggregateUsesRemainingPackageBudget(t *testing.T) {
	const (
		extensionCount = 64
		numberCount    = 12000
		testLimit      = 8 << 20
	)
	canonicalPayloadBytes := 2 + numberCount*128 + numberCount - 1
	if projected := uint64(canonicalPayloadBytes * extensionCount); projected <= MaxArchiveExpandedBytes {
		t.Fatalf("test projection = %d, want over production aggregate limit", projected)
	}

	files := exponentExtensionFiles(t, extensionCount, numberCount)
	var rawBytes uint64
	for _, data := range files {
		rawBytes += uint64(len(data))
	}
	if rawBytes >= testLimit {
		t.Fatalf("raw test package = %d, want below bounded seam %d", rawBytes, testLimit)
	}
	_, err := fromFilesWithLimit(files, fixtureLock(t), extension.DefaultSupport, testLimit)
	if !extension.IsCode(err, extension.ErrInvalid) || !strings.Contains(err.Error(), "canonical payload") {
		t.Fatalf("canonical aggregate result = %v", err)
	}
}

func TestImportRejectsShortArtifactBeforePayloadCanonicalExpansion(t *testing.T) {
	const numberCount = 12000
	files := exponentExtensionFiles(t, 1, numberCount)
	pkg, err := FromFiles(files, fixtureLock(t), extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err := pkg.Export()
	if err != nil {
		t.Fatal(err)
	}
	archived, err := readSnapshot(snapshot)
	if err != nil {
		t.Fatal(err)
	}
	archived["extensions/third.party.p00/value.json"] = files["extensions/third.party.p00/value.json"]
	archived[ArtifactPath] = []byte(`{}`)
	forged := zipFileMap(t, archived)

	runtime.GC()
	var before runtime.MemStats
	var after runtime.MemStats
	runtime.ReadMemStats(&before)
	_, err = ImportBytes(forged, extension.DefaultSupport)
	runtime.ReadMemStats(&after)
	if err == nil || !strings.Contains(err.Error(), "exact lock and package manifest") {
		t.Fatalf("short artifact result = %v", err)
	}
	if allocated := after.TotalAlloc - before.TotalAlloc; allocated > 8<<20 {
		t.Fatalf("short artifact allocated %d bytes; payload was canonicalized before envelope rejection", allocated)
	}
}

func TestImportCanonicalPayloadUsesExactEnvelopeRemainingBudget(t *testing.T) {
	const numberCount = 12000
	files := exponentExtensionFiles(t, 1, numberCount)
	pkg, err := FromFiles(files, fixtureLock(t), extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err := pkg.Export()
	if err != nil {
		t.Fatal(err)
	}
	archived, err := readSnapshot(snapshot)
	if err != nil {
		t.Fatal(err)
	}
	payloadPath := "extensions/third.party.p00/value.json"
	canonicalLength := len(archived[payloadPath])
	archived[payloadPath] = files[payloadPath]
	var rawExpanded uint64
	for _, data := range archived {
		rawExpanded += uint64(len(data))
	}
	canonicalGrowth := uint64(canonicalLength - len(archived[payloadPath]))
	if canonicalGrowth < 2 {
		t.Fatal("test payload does not canonically expand")
	}
	limit := rawExpanded + canonicalGrowth/2
	forged, err := NewSnapshot(zipFileMap(t, archived))
	if err != nil {
		t.Fatal(err)
	}
	_, err = importWithLimit(forged, extension.DefaultSupport, limit)
	if !extension.IsCode(err, extension.ErrInvalid) || !strings.Contains(err.Error(), "canonical payload") {
		t.Fatalf("bounded import canonical aggregate result = %v", err)
	}
}

func TestReplacementCanonicalizationUsesRemainingPackageBudget(t *testing.T) {
	const (
		testLimit = 1 << 20
		spare     = 32
	)
	files := v2Files(t, false, 1)
	document, err := manifest.Parse(files[ManifestPath])
	if err != nil {
		t.Fatal(err)
	}
	files[ManifestPath], err = manifest.CanonicalTOML(document)
	if err != nil {
		t.Fatal(err)
	}
	files[v2PayloadPath] = []byte(`{"count":1}`)
	lock := fixtureLock(t)
	envelopeBytes, err := exactSourceEnvelopeBytes(files, lock)
	if err != nil {
		t.Fatal(err)
	}
	var contentBytes uint64
	for _, data := range files {
		contentBytes += uint64(len(data))
	}
	if contentBytes+envelopeBytes+spare >= testLimit {
		t.Fatal("test fixture leaves no room for bounded filler")
	}
	files["content/fill.bin"] = make([]byte, int(testLimit-contentBytes-envelopeBytes-spare))
	pkg, err := fromFilesWithLimit(files, lock, extension.DefaultSupport, testLimit)
	if err != nil {
		t.Fatal(err)
	}
	_, err = pkg.ReplaceExtension("third.party.probe", []byte(`{"count":1e127}`))
	if !extension.IsCode(err, extension.ErrInvalid) || !strings.Contains(err.Error(), "canonical payload") {
		t.Fatalf("bounded replacement aggregate result = %v", err)
	}
}

func exponentExtensionFiles(t *testing.T, extensionCount, numberCount int) map[string][]byte {
	t.Helper()
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"array","items":{"type":"number"}}`)
	digest := sha256.Sum256(schema)
	payload := []byte(`[` + strings.Repeat(`1e127,`, numberCount-1) + `1e127]`)
	manifestText := strings.Replace(string(fixtureBytes(t, "package.toml")), "schema_version = 1", "schema_version = 2", 1)
	files := map[string][]byte{"package.lock.json": fixtureBytes(t, "package.lock.json")}
	for index := 0; index < extensionCount; index++ {
		namespace := fmt.Sprintf("third.party.p%02d", index)
		schemaPath := "extensions/" + namespace + "/value.schema.json"
		payloadPath := "extensions/" + namespace + "/value.json"
		manifestText += "\n[[extensions]]\n" +
			"namespace = \"" + namespace + "\"\n" +
			"required = true\n" +
			"contract_version = 1\n" +
			"schema_path = \"" + schemaPath + "\"\n" +
			"schema_sha256 = \"sha256:" + hex.EncodeToString(digest[:]) + "\"\n" +
			"payload_path = \"" + payloadPath + "\"\n" +
			"host_api_major = 1\n" +
			"host_api_min_minor = 0\n" +
			"host_api_max_minor = 0\n"
		files[schemaPath] = schema
		files[payloadPath] = payload
	}
	files[ManifestPath] = []byte(manifestText)
	return files
}
