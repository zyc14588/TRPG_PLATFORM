// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package archive

import (
	"archive/zip"
	"bytes"
	"encoding/binary"
	"strings"
	"testing"
)

func TestReadSnapshotAcceptsPreflightedStoreAndDeflate(t *testing.T) {
	t.Parallel()
	data := makeTestZIP(t, []testZIPEntry{
		{name: "package.toml", data: []byte("manifest"), method: zip.Store},
		{name: "data/value.json", data: []byte(`{"value":1}`), method: zip.Deflate},
	})
	snapshot, err := NewSnapshot(data)
	if err != nil {
		t.Fatal(err)
	}
	files, err := readSnapshot(snapshot)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(files["package.toml"], []byte("manifest")) || !bytes.Equal(files["data/value.json"], []byte(`{"value":1}`)) {
		t.Fatalf("expanded files = %#v", files)
	}
}

func TestReadSnapshotNeverReachesZIPReaderOnPreflightFailure(t *testing.T) {
	t.Parallel()
	data := makeTestZIP(t, []testZIPEntry{{name: "value", data: []byte("x")}})
	central := centralOffset(t, data)
	binary.LittleEndian.PutUint32(data[central+24:central+28], MaxEntryExpandedBytes+1)
	snapshot, err := NewSnapshot(data)
	if err != nil {
		t.Fatal(err)
	}
	_, err = readSnapshot(snapshot)
	if err == nil || !strings.Contains(err.Error(), "archive preflight") || strings.Contains(err.Error(), "open preflighted ZIP") {
		t.Fatalf("bounded import error = %v", err)
	}
}

func TestReadSnapshotChecksCRCWhenDescriptorIsAbsentAndDeclaredCRCIsZero(t *testing.T) {
	t.Parallel()
	var output bytes.Buffer
	writer := zip.NewWriter(&output)
	header := &zip.FileHeader{
		Name: "value", Method: zip.Store, CRC32: 0,
		CompressedSize: 7, UncompressedSize: 7,
		CompressedSize64: 7, UncompressedSize64: 7,
	}
	file, err := writer.CreateRaw(header)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := file.Write([]byte("payload")); err != nil {
		t.Fatal(err)
	}
	if err := writer.Close(); err != nil {
		t.Fatal(err)
	}
	snapshot, err := NewSnapshot(output.Bytes())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := readSnapshot(snapshot); err == nil || !strings.Contains(err.Error(), "checksum") {
		t.Fatalf("forged zero CRC error = %v", err)
	}
}
