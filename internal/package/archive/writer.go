// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package archive

import (
	"archive/zip"
	"bytes"
	"fmt"
	"io/fs"
	"sort"
)

// Export serializes the canonical model with fixed ZIP32 Store metadata.
func (pkg *Package) Export() (Snapshot, error) {
	entries := make([]Entry, 0, len(pkg.entries)+2)
	entries = append(entries, pkg.entries...)
	entries = append(entries,
		Entry{path: PlatformLockPath, data: pkg.lockRaw},
		Entry{path: ArtifactPath, data: pkg.artifact.CanonicalJSON()},
	)
	sort.Slice(entries, func(i, j int) bool { return entries[i].path < entries[j].path })
	var output bytes.Buffer
	writer := zip.NewWriter(&output)
	for _, entry := range entries {
		header := &zip.FileHeader{
			Name: entry.path, Method: zip.Store,
			ModifiedDate: 1<<5 | 1, // 1980-01-01, the DOS epoch.
			ModifiedTime: 0,
		}
		header.SetMode(fs.FileMode(0644))
		header.Extra = nil
		header.Comment = ""
		file, err := writer.CreateHeader(header)
		if err != nil {
			_ = writer.Close()
			return Snapshot{}, fmt.Errorf("create archive entry %q: %w", boundedPath(entry.path), err)
		}
		if _, err := file.Write(entry.data); err != nil {
			_ = writer.Close()
			return Snapshot{}, fmt.Errorf("write archive entry %q: %w", boundedPath(entry.path), err)
		}
	}
	if err := writer.Close(); err != nil {
		return Snapshot{}, fmt.Errorf("close archive: %w", err)
	}
	snapshot, err := NewSnapshot(output.Bytes())
	if err != nil {
		return Snapshot{}, err
	}
	if _, err := preflightZIP32(snapshot.data); err != nil {
		return Snapshot{}, fmt.Errorf("generated archive failed its own preflight: %w", err)
	}
	return snapshot, nil
}
