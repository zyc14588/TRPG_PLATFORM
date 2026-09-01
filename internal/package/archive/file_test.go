// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package archive

import (
	"crypto/sha256"
	"encoding/hex"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
)

func TestImportFileBindsOneStableSnapshotAndCallerCopy(t *testing.T) {
	pkg, err := FromFiles(v1Files(t), fixtureLock(t), extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err := pkg.Export()
	if err != nil {
		t.Fatal(err)
	}
	data := snapshot.Bytes()
	name := filepath.Join(t.TempDir(), "package.trpgpkg")
	if err := os.WriteFile(name, data, 0600); err != nil {
		t.Fatal(err)
	}
	loaded, err := ImportFile(name, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(data)
	want := "sha256:" + hex.EncodeToString(digest[:])
	if got, exists := loaded.SourceArchiveHash(); !exists || got.String() != want {
		t.Fatalf("source archive hash = %s, %v; want %s", got, exists, want)
	}

	fromBytes, err := ImportBytes(data, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	data[0] ^= 0xff
	if got, exists := fromBytes.SourceArchiveHash(); !exists || got.String() != want {
		t.Fatalf("caller mutation changed ImportBytes token = %s, %v", got, exists)
	}
}

func TestImportFileRejectsSymlinkSpecialOversizeAndInPlaceChange(t *testing.T) {
	directory := t.TempDir()
	regular := filepath.Join(directory, "regular")
	if err := os.WriteFile(regular, []byte("not a zip"), 0600); err != nil {
		t.Fatal(err)
	}
	symlink := filepath.Join(directory, "link")
	if err := os.Symlink(regular, symlink); err == nil {
		if _, err := ImportFile(symlink, extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "symbolic links") {
			t.Fatalf("symlink result = %v", err)
		}
	}
	if _, err := ImportFile(directory, extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "regular file") {
		t.Fatalf("directory result = %v", err)
	}

	oversized := filepath.Join(directory, "oversized")
	file, err := os.Create(oversized)
	if err != nil {
		t.Fatal(err)
	}
	if err := file.Truncate(MaxSnapshotBytes + 1); err != nil {
		file.Close()
		t.Fatal(err)
	}
	if err := file.Close(); err != nil {
		t.Fatal(err)
	}
	if _, err := ImportFile(oversized, extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "size exceeds") {
		t.Fatalf("oversized result = %v", err)
	}

	before, err := os.Lstat(regular)
	if err != nil {
		t.Fatal(err)
	}
	_, err = importFileWithHook(regular, extension.DefaultSupport, func() error {
		writer, openErr := os.OpenFile(regular, os.O_WRONLY, 0)
		if openErr != nil {
			return openErr
		}
		if _, writeErr := writer.WriteAt([]byte("N"), 0); writeErr != nil {
			writer.Close()
			return writeErr
		}
		if syncErr := writer.Sync(); syncErr != nil {
			writer.Close()
			return syncErr
		}
		if closeErr := writer.Close(); closeErr != nil {
			return closeErr
		}
		changed := time.Now().Add(2 * time.Second)
		return os.Chtimes(regular, changed, changed)
	})
	if err == nil || !strings.Contains(err.Error(), "changed while reading") {
		t.Fatalf("in-place change result = %v", err)
	}
	after, err := os.Lstat(regular)
	if err != nil {
		t.Fatal(err)
	}
	if !os.SameFile(before, after) {
		t.Fatal("test did not mutate the same inode")
	}
}
