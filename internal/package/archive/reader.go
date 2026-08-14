// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package archive

import (
	"archive/zip"
	"bytes"
	"fmt"
	"hash/crc32"
	"io"
)

// readSnapshot is the only archive/zip import entry point. Keep the metadata
// preflight visibly before zip.NewReader: callers cannot accidentally allocate
// or decompress through the standard library before package limits apply.
func readSnapshot(snapshot Snapshot) (map[string][]byte, error) {
	metadata, err := preflightZIP32(snapshot.data)
	if err != nil {
		return nil, fmt.Errorf("archive preflight: %w", err)
	}
	reader, err := zip.NewReader(bytes.NewReader(snapshot.data), int64(len(snapshot.data)))
	if err != nil {
		return nil, fmt.Errorf("open preflighted ZIP: %w", err)
	}
	if len(reader.File) != len(metadata.entries) {
		return nil, fmt.Errorf("archive reader entry count differs from preflight")
	}
	files := make(map[string][]byte, len(reader.File))
	for index, file := range reader.File {
		expected := metadata.entries[index]
		if file.Name != expected.name || file.Flags != expected.flags || file.Method != expected.method ||
			file.CRC32 != expected.crc32 || file.CompressedSize64 != expected.compressedSize ||
			file.UncompressedSize64 != expected.uncompressedSize {
			return nil, fmt.Errorf("archive reader metadata differs from preflight for entry %d", index)
		}
		if !file.Mode().IsRegular() {
			return nil, fmt.Errorf("archive entry %q is not a regular file", boundedPath(file.Name))
		}
		data, err := readExpandedFile(file, expected.uncompressedSize)
		if err != nil {
			return nil, fmt.Errorf("read archive entry %q: %w", boundedPath(file.Name), err)
		}
		files[file.Name] = data
	}
	return files, nil
}

func readExpandedFile(file *zip.File, size uint64) ([]byte, error) {
	if size > MaxEntryExpandedBytes {
		return nil, fmt.Errorf("expanded size exceeds %d bytes", MaxEntryExpandedBytes)
	}
	reader, err := file.Open()
	if err != nil {
		return nil, err
	}
	data := make([]byte, int(size))
	if _, err := io.ReadFull(reader, data); err != nil {
		_ = reader.Close()
		return nil, fmt.Errorf("expanded data is truncated: %w", err)
	}
	var tail [1]byte
	count, tailErr := reader.Read(tail[:])
	if count != 0 || tailErr != io.EOF {
		_ = reader.Close()
		if tailErr == nil {
			tailErr = fmt.Errorf("expanded data exceeds declared size")
		}
		return nil, tailErr
	}
	if err := reader.Close(); err != nil {
		return nil, err
	}
	if crc32.ChecksumIEEE(data) != file.CRC32 {
		return nil, fmt.Errorf("expanded data checksum differs from central metadata")
	}
	return data, nil
}
