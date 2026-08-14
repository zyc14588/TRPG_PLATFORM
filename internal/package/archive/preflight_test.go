// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package archive

import (
	"archive/zip"
	"bytes"
	"encoding/binary"
	"fmt"
	"io/fs"
	"strings"
	"testing"
)

func TestSnapshotIsBoundedAndImmutable(t *testing.T) {
	source := makeTestZIP(t, []testZIPEntry{{name: "package.toml", data: []byte("manifest")}})
	snapshot, err := NewSnapshot(source)
	if err != nil {
		t.Fatal(err)
	}
	wantHash := snapshot.Hash()
	source[0] ^= 0xff
	first := snapshot.Bytes()
	first[0] ^= 0xff
	if got := snapshot.Bytes(); got[0] == first[0] || snapshot.Hash() != wantHash {
		t.Fatal("snapshot changed through caller-owned or returned bytes")
	}
	if _, err := NewSnapshot(make([]byte, MaxSnapshotBytes+1)); err == nil {
		t.Fatal("oversized snapshot succeeded")
	}
}

func TestPreflightAcceptsStoreAndDeflate(t *testing.T) {
	t.Parallel()
	for _, method := range []uint16{zip.Store, zip.Deflate} {
		data := makeTestZIP(t, []testZIPEntry{{name: "package.toml", data: []byte("manifest"), method: method}})
		result, err := preflightZIP32(data)
		if err != nil {
			t.Fatalf("method %d: %v", method, err)
		}
		if len(result.entries) != 1 || result.entries[0].name != "package.toml" || result.expandedBytes != 8 {
			t.Fatalf("method %d metadata = %#v", method, result)
		}
	}
}

func TestPreflightAcceptsDeflateOptionsOnlyForDeflate(t *testing.T) {
	t.Parallel()
	for _, option := range []uint16{0x0002, 0x0004, 0x0006} {
		data := makeTestZIP(t, []testZIPEntry{{name: "value", data: []byte("payload"), method: zip.Deflate}})
		patchZIPFlags(t, data, option)
		snapshot, err := NewSnapshot(data)
		if err != nil {
			t.Fatal(err)
		}
		files, err := readSnapshot(snapshot)
		if err != nil || string(files["value"]) != "payload" {
			t.Fatalf("Deflate option 0x%04x = %q, %v", option, files["value"], err)
		}
	}

	for _, test := range []struct {
		name   string
		method uint16
		flag   uint16
	}{
		{name: "Store compression option", method: zip.Store, flag: 0x0002},
		{name: "encryption", method: zip.Deflate, flag: 0x0001},
		{name: "unsupported dangerous flag", method: zip.Deflate, flag: 0x0010},
	} {
		t.Run(test.name, func(t *testing.T) {
			data := makeTestZIP(t, []testZIPEntry{{name: "value", data: []byte("payload"), method: test.method}})
			patchZIPFlags(t, data, test.flag)
			if _, err := preflightZIP32(data); err == nil || !strings.Contains(err.Error(), "unsupported ZIP flags") {
				t.Fatalf("flag 0x%04x result = %v", test.flag, err)
			}
		})
	}
}

func TestPreflightRejectsPathsSpecialFilesAndMetadataAttacks(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name string
		make func(*testing.T) []byte
	}{
		{name: "traversal", make: func(t *testing.T) []byte {
			return makeTestZIP(t, []testZIPEntry{{name: "../escape", data: []byte("x")}})
		}},
		{name: "absolute", make: func(t *testing.T) []byte { return makeTestZIP(t, []testZIPEntry{{name: "/escape", data: []byte("x")}}) }},
		{name: "duplicate", make: func(t *testing.T) []byte {
			return makeTestZIP(t, []testZIPEntry{{name: "a", data: []byte("x")}, {name: "a", data: []byte("y")}})
		}},
		{name: "folded parent", make: func(t *testing.T) []byte {
			return makeTestZIP(t, []testZIPEntry{{name: "A/x", data: []byte("x")}, {name: "a/y", data: []byte("y")}})
		}},
		{name: "directory", make: func(t *testing.T) []byte {
			return makeTestZIP(t, []testZIPEntry{{name: "dir/", mode: fs.ModeDir | 0755}})
		}},
		{name: "symlink", make: func(t *testing.T) []byte {
			return makeTestZIP(t, []testZIPEntry{{name: "link", data: []byte("target"), mode: fs.ModeSymlink | 0777}})
		}},
		{name: "trailing data", make: func(t *testing.T) []byte {
			return append(makeTestZIP(t, []testZIPEntry{{name: "a", data: []byte("x")}}), 0)
		}},
		{name: "preamble", make: func(t *testing.T) []byte {
			data := append([]byte("prefix"), makeTestZIP(t, []testZIPEntry{{name: "a", data: []byte("x")}})...)
			shiftZIPOffsets(data, 6)
			return data
		}},
		{name: "overlapping local offset", make: func(t *testing.T) []byte {
			data := makeTestZIP(t, []testZIPEntry{{name: "a", data: []byte("x")}, {name: "b", data: []byte("y")}})
			central := centralOffset(t, data)
			firstOffset := binary.LittleEndian.Uint32(data[central+42 : central+46])
			second := nextCentral(t, data, central)
			binary.LittleEndian.PutUint32(data[second+42:second+46], firstOffset)
			return data
		}},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			if _, err := preflightZIP32(test.make(t)); err == nil {
				t.Fatal("malicious ZIP passed preflight")
			}
		})
	}
}

func TestPreflightRejectsLimitsBeforeZIPReader(t *testing.T) {
	t.Parallel()
	ratio := makeTestZIP(t, []testZIPEntry{{name: "bomb", data: bytes.Repeat([]byte("A"), 1000), method: zip.Deflate}})
	central := centralOffset(t, ratio)
	binary.LittleEndian.PutUint32(ratio[central+20:central+24], 1)
	if _, err := preflightZIP32(ratio); err == nil || !strings.Contains(err.Error(), "compression ratio") {
		t.Fatalf("ratio error = %v", err)
	}

	tooMany := makeTestZIP(t, nil)
	end := len(tooMany) - endHeaderBytes
	binary.LittleEndian.PutUint16(tooMany[end+8:end+10], MaxArchiveEntries+1)
	binary.LittleEndian.PutUint16(tooMany[end+10:end+12], MaxArchiveEntries+1)
	if _, err := preflightZIP32(tooMany); err == nil || !strings.Contains(err.Error(), "maximum") {
		t.Fatalf("entry count error = %v", err)
	}

	tooLarge := makeTestZIP(t, []testZIPEntry{{name: "large", data: []byte("x"), method: zip.Deflate}})
	central = centralOffset(t, tooLarge)
	binary.LittleEndian.PutUint32(tooLarge[central+24:central+28], MaxEntryExpandedBytes+1)
	if _, err := preflightZIP32(tooLarge); err == nil || !strings.Contains(err.Error(), "expands past") {
		t.Fatalf("expanded size error = %v", err)
	}
}

func TestPreflightRequiresExactDataDescriptorAndLocalLayout(t *testing.T) {
	t.Parallel()
	base := makeTestZIP(t, []testZIPEntry{{name: "payload.json", data: []byte(`{"value":1}`), method: zip.Deflate}})
	metadata, err := preflightZIP32(base)
	if err != nil {
		t.Fatal(err)
	}
	if metadata.entries[0].flags&0x0008 == 0 || metadata.entries[0].recordEnd != metadata.centralOffset {
		t.Fatalf("test ZIP does not use an exact descriptor: %#v", metadata.entries[0])
	}

	tampered := append([]byte(nil), base...)
	descriptorStart := metadata.entries[0].dataEnd
	if binary.LittleEndian.Uint32(tampered[descriptorStart:descriptorStart+4]) == dataDescriptorSignature {
		tampered[descriptorStart+4] ^= 0xff
	} else {
		tampered[descriptorStart] ^= 0xff
	}
	if _, err := preflightZIP32(tampered); err == nil || !strings.Contains(err.Error(), "data descriptor") {
		t.Fatalf("descriptor tamper error = %v", err)
	}

	central := centralOffset(t, base)
	withGap := make([]byte, 0, len(base)+1)
	withGap = append(withGap, base[:central]...)
	withGap = append(withGap, 0)
	withGap = append(withGap, base[central:]...)
	end, err := findEndHeader(withGap)
	if err != nil {
		t.Fatal(err)
	}
	binary.LittleEndian.PutUint32(withGap[end+16:end+20], uint32(central+1))
	if _, err := preflightZIP32(withGap); err == nil || !strings.Contains(err.Error(), "exactly reach") {
		t.Fatalf("hidden gap error = %v", err)
	}
}

func TestPreflightDoesNotTreatDescriptorSignatureAsUnsignedCRC(t *testing.T) {
	t.Parallel()
	data := makeTestZIP(t, []testZIPEntry{{name: "value", data: []byte("payload"), method: zip.Deflate}})
	metadata, err := preflightZIP32(data)
	if err != nil {
		t.Fatal(err)
	}
	entry := metadata.entries[0]
	if binary.LittleEndian.Uint32(data[entry.dataEnd:entry.dataEnd+4]) != dataDescriptorSignature {
		t.Fatal("test ZIP lacks signed descriptor")
	}
	// Remove the original CRC word so the 12 remaining bytes look like an
	// unsigned descriptor whose CRC equals the descriptor signature.
	data = append(data[:entry.dataEnd+4], data[entry.dataEnd+8:]...)
	end, err := findEndHeader(data)
	if err != nil {
		t.Fatal(err)
	}
	newCentral := int(metadata.centralOffset) - 4
	binary.LittleEndian.PutUint32(data[end+16:end+20], uint32(newCentral))
	binary.LittleEndian.PutUint32(data[newCentral+16:newCentral+20], dataDescriptorSignature)
	if _, err := preflightZIP32(data); err == nil || !strings.Contains(err.Error(), "signed ZIP32 data descriptor") {
		t.Fatalf("signature/CRC differential error = %v", err)
	}
}

func TestPreflightRejectsEOCDParserDifferential(t *testing.T) {
	t.Parallel()
	data := makeTestZIP(t, []testZIPEntry{{name: "a", data: []byte("x")}})
	end := len(data) - endHeaderBytes
	comment := append([]byte("prefix"), make([]byte, endHeaderBytes)...)
	binary.LittleEndian.PutUint32(comment[len("prefix"):len("prefix")+4], endHeaderSignature)
	comment = append(comment, []byte("tail")...)
	binary.LittleEndian.PutUint16(data[end+20:end+22], uint16(len(comment)))
	data = append(data, comment...)
	if _, err := preflightZIP32(data); err == nil || !strings.Contains(err.Error(), "unclaimed bytes") {
		t.Fatalf("EOCD differential error = %v", err)
	}
}

func TestPreflightBoundsCentralMetadataAndStoredSizes(t *testing.T) {
	t.Parallel()
	entries := make([]testZIPEntry, 129)
	comment := strings.Repeat("m", 65535)
	for index := range entries {
		entries[index] = testZIPEntry{name: "entry/" + strings.Repeat("0", 3-len(fmt.Sprint(index))) + fmt.Sprint(index), comment: comment}
	}
	metadataHeavy := makeTestZIP(t, entries)
	if _, err := preflightZIP32(metadataHeavy); err == nil || !strings.Contains(err.Error(), "central directory exceeds") {
		t.Fatalf("central metadata error = %v", err)
	}

	stored := makeTestZIP(t, []testZIPEntry{{name: "stored", data: []byte("x"), method: zip.Store}})
	central := centralOffset(t, stored)
	binary.LittleEndian.PutUint32(stored[central+24:central+28], 2)
	if _, err := preflightZIP32(stored); err == nil || !strings.Contains(err.Error(), "stored") {
		t.Fatalf("stored size mismatch error = %v", err)
	}
}

func TestPreflightBoundsNamesBeforeConversionOrComparison(t *testing.T) {
	t.Parallel()
	centralLong := makeTestZIP(t, []testZIPEntry{{name: strings.Repeat("n", 241), data: []byte("x")}})
	if _, err := preflightZIP32(centralLong); err == nil || !strings.Contains(err.Error(), "portable path bound") {
		t.Fatalf("central long-name error = %v", err)
	}

	localLong := makeTestZIP(t, []testZIPEntry{{name: "name", data: []byte("x")}})
	binary.LittleEndian.PutUint16(localLong[26:28], 241)
	if _, err := preflightZIP32(localLong); err == nil || !strings.Contains(err.Error(), "name length") {
		t.Fatalf("local long-name error = %v", err)
	}
}

func TestPreflightRejectsDOSVolumeLabelsAndMacUnixSpecialModes(t *testing.T) {
	t.Parallel()
	volume := makeTestZIP(t, []testZIPEntry{{name: "label", data: []byte("x")}})
	central := centralOffset(t, volume)
	volume[central+5] = 0 // FAT creator.
	binary.LittleEndian.PutUint32(volume[central+38:central+42], 0x08)
	if _, err := preflightZIP32(volume); err == nil || !strings.Contains(err.Error(), "special file") {
		t.Fatalf("DOS volume-label error = %v", err)
	}

	macSpecial := makeTestZIP(t, []testZIPEntry{{name: "link", data: []byte("target"), mode: fs.ModeSymlink | 0777}})
	central = centralOffset(t, macSpecial)
	macSpecial[central+5] = 19
	if _, err := preflightZIP32(macSpecial); err == nil || !strings.Contains(err.Error(), "special file") {
		t.Fatalf("macOS special-mode error = %v", err)
	}
}

type testZIPEntry struct {
	name    string
	data    []byte
	method  uint16
	mode    fs.FileMode
	comment string
}

func makeTestZIP(t *testing.T, entries []testZIPEntry) []byte {
	t.Helper()
	var output bytes.Buffer
	writer := zip.NewWriter(&output)
	for _, entry := range entries {
		header := &zip.FileHeader{Name: entry.name, Method: entry.method, Comment: entry.comment}
		if entry.method == 0 {
			header.Method = zip.Store
		}
		if entry.mode != 0 {
			header.SetMode(entry.mode)
		}
		file, err := writer.CreateHeader(header)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := file.Write(entry.data); err != nil {
			t.Fatal(err)
		}
	}
	if err := writer.Close(); err != nil {
		t.Fatal(err)
	}
	return output.Bytes()
}

func centralOffset(t *testing.T, data []byte) int {
	t.Helper()
	end, err := findEndHeader(data)
	if err != nil {
		t.Fatal(err)
	}
	return int(binary.LittleEndian.Uint32(data[end+16 : end+20]))
}

func nextCentral(t *testing.T, data []byte, offset int) int {
	t.Helper()
	if binary.LittleEndian.Uint32(data[offset:offset+4]) != centralHeaderSignature {
		t.Fatal("not a central header")
	}
	return offset + centralHeaderBytes + int(binary.LittleEndian.Uint16(data[offset+28:offset+30])) + int(binary.LittleEndian.Uint16(data[offset+30:offset+32])) + int(binary.LittleEndian.Uint16(data[offset+32:offset+34]))
}

func patchZIPFlags(t *testing.T, data []byte, additional uint16) {
	t.Helper()
	central := centralOffset(t, data)
	local := int(binary.LittleEndian.Uint32(data[central+42 : central+46]))
	centralFlags := binary.LittleEndian.Uint16(data[central+8 : central+10])
	localFlags := binary.LittleEndian.Uint16(data[local+6 : local+8])
	binary.LittleEndian.PutUint16(data[central+8:central+10], centralFlags|additional)
	binary.LittleEndian.PutUint16(data[local+6:local+8], localFlags|additional)
}

func shiftZIPOffsets(data []byte, delta uint32) {
	end := len(data) - endHeaderBytes
	central := int(binary.LittleEndian.Uint32(data[end+16 : end+20]))
	binary.LittleEndian.PutUint32(data[end+16:end+20], uint32(central)+delta)
	central += int(delta)
	for position := central; position < end; {
		binary.LittleEndian.PutUint32(data[position+42:position+46], binary.LittleEndian.Uint32(data[position+42:position+46])+delta)
		position += centralHeaderBytes + int(binary.LittleEndian.Uint16(data[position+28:position+30])) + int(binary.LittleEndian.Uint16(data[position+30:position+32])) + int(binary.LittleEndian.Uint16(data[position+32:position+34]))
	}
}
