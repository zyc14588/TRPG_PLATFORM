// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package archive

import (
	"fmt"
	"net"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
)

func TestImportProjectPreservesV1AndUsesCanonicalArchiveModel(t *testing.T) {
	files := v1Files(t)
	files[ManifestPath] = append(files[ManifestPath], []byte("\n# project-raw-preserved\n")...)
	root := writeProjectFiles(t, files)
	project, err := ImportProject(root, fixtureLock(t), extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	expected, err := FromFiles(files, fixtureLock(t), extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if string(project.ManifestBytes()) != string(files[ManifestPath]) || project.ContentHash() != expected.ContentHash() {
		t.Fatal("project load changed v1 raw manifest or canonical content identity")
	}
	projectArchive, err := project.Export()
	if err != nil {
		t.Fatal(err)
	}
	expectedArchive, err := expected.Export()
	if err != nil {
		t.Fatal(err)
	}
	if string(projectArchive.Bytes()) != string(expectedArchive.Bytes()) {
		t.Fatal("project and in-memory source exports differ")
	}
	if _, err := Import(projectArchive, extension.DefaultSupport); err != nil {
		t.Fatalf("project export re-import: %v", err)
	}
}

func TestImportProjectRejectsSymlinksSpecialFilesAndPortableCollisions(t *testing.T) {
	t.Run("file symlink", func(t *testing.T) {
		root := writeProjectFiles(t, v1Files(t))
		if err := os.Symlink(filepath.Join(root, "content", "raw.bin"), filepath.Join(root, "link.bin")); err != nil {
			t.Skipf("symlink unavailable: %v", err)
		}
		if _, err := ImportProject(root, fixtureLock(t), extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "symbolic link") {
			t.Fatalf("file symlink result = %v", err)
		}
	})

	t.Run("parent escape symlink", func(t *testing.T) {
		root := writeProjectFiles(t, v1Files(t))
		outside := t.TempDir()
		if err := os.WriteFile(filepath.Join(outside, "escape.bin"), []byte("escape"), 0600); err != nil {
			t.Fatal(err)
		}
		if err := os.Symlink(outside, filepath.Join(root, "escape")); err != nil {
			t.Skipf("symlink unavailable: %v", err)
		}
		if _, err := ImportProject(root, fixtureLock(t), extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "symbolic link") {
			t.Fatalf("parent escape result = %v", err)
		}
	})

	t.Run("root path parent symlink", func(t *testing.T) {
		base := t.TempDir()
		outside := writeProjectFiles(t, v1Files(t))
		link := filepath.Join(base, "linkparent")
		if err := os.Symlink(filepath.Dir(outside), link); err != nil {
			t.Skipf("symlink unavailable: %v", err)
		}
		throughLink := filepath.Join(link, filepath.Base(outside))
		if _, err := ImportProject(throughLink, fixtureLock(t), extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "symbolic links") {
			t.Fatalf("root parent symlink result = %v", err)
		}
	})

	t.Run("root itself symlink", func(t *testing.T) {
		target := writeProjectFiles(t, v1Files(t))
		link := filepath.Join(t.TempDir(), "project-link")
		if err := os.Symlink(target, link); err != nil {
			t.Skipf("symlink unavailable: %v", err)
		}
		if _, err := ImportProject(link, fixtureLock(t), extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "symbolic links") {
			t.Fatalf("root symlink result = %v", err)
		}
	})

	t.Run("special socket", func(t *testing.T) {
		root := writeProjectFiles(t, v1Files(t))
		listener, err := net.Listen("unix", filepath.Join(root, "special.sock"))
		if err != nil {
			t.Skipf("Unix-domain socket unavailable: %v", err)
		}
		defer listener.Close()
		if _, err := ImportProject(root, fixtureLock(t), extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "not a regular file or directory") {
			t.Fatalf("special file result = %v", err)
		}
	})

	t.Run("case-folded parent", func(t *testing.T) {
		root := writeProjectFiles(t, v1Files(t))
		upper := filepath.Join(root, "A")
		lower := filepath.Join(root, "a")
		if err := os.MkdirAll(upper, 0700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(upper, "x"), nil, 0600); err != nil {
			t.Fatal(err)
		}
		if err := os.MkdirAll(lower, 0700); err != nil {
			t.Fatal(err)
		}
		upperInfo, _ := os.Lstat(upper)
		lowerInfo, _ := os.Lstat(lower)
		if os.SameFile(upperInfo, lowerInfo) {
			t.Skip("filesystem is case-insensitive")
		}
		if err := os.WriteFile(filepath.Join(lower, "y"), nil, 0600); err != nil {
			t.Fatal(err)
		}
		if _, err := ImportProject(root, fixtureLock(t), extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "portable spelling") {
			t.Fatalf("case-folded parent result = %v", err)
		}
	})
}

func TestImportProjectEnforcesMetadataLimitsBeforeContentReads(t *testing.T) {
	t.Run("path", func(t *testing.T) {
		root := writeProjectFiles(t, v1Files(t))
		longPath := filepath.Join(root, strings.Repeat("a", 120), strings.Repeat("b", 120))
		if err := os.MkdirAll(filepath.Dir(longPath), 0700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(longPath, nil, 0600); err != nil {
			t.Fatal(err)
		}
		if _, err := ImportProject(root, fixtureLock(t), extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "240 bytes") {
			t.Fatalf("long path result = %v", err)
		}
	})

	t.Run("per entry", func(t *testing.T) {
		root := writeProjectFiles(t, v1Files(t))
		name := filepath.Join(root, "content", "oversized.bin")
		file, err := os.Create(name)
		if err != nil {
			t.Fatal(err)
		}
		if err := file.Truncate(MaxEntryExpandedBytes + 1); err != nil {
			file.Close()
			t.Fatal(err)
		}
		file.Close()
		if _, err := ImportProject(root, fixtureLock(t), extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "exceeds") {
			t.Fatalf("per-entry result = %v", err)
		}
	})

	t.Run("aggregate with exact envelope", func(t *testing.T) {
		files := v1Files(t)
		root := writeProjectFiles(t, files)
		var present uint64
		for _, data := range files {
			present += uint64(len(data))
		}
		remaining := uint64(MaxArchiveExpandedBytes) - present
		for index := 0; remaining != 0; index++ {
			length := remaining
			if length > MaxEntryExpandedBytes {
				length = MaxEntryExpandedBytes
			}
			name := filepath.Join(root, "content", fmt.Sprintf("sparse-%02d.bin", index))
			file, err := os.Create(name)
			if err != nil {
				t.Fatal(err)
			}
			if err := file.Truncate(int64(length)); err != nil {
				file.Close()
				t.Fatal(err)
			}
			file.Close()
			remaining -= length
		}
		if _, err := ImportProject(root, fixtureLock(t), extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "content and canonical envelope") {
			t.Fatalf("aggregate result = %v", err)
		}
	})

	t.Run("entry boundary", func(t *testing.T) {
		root := writeProjectFiles(t, v1Files(t))
		directory := filepath.Join(root, "many")
		if err := os.MkdirAll(directory, 0700); err != nil {
			t.Fatal(err)
		}
		for index := 0; index < MaxArchiveEntries-2-3; index++ {
			if err := os.WriteFile(filepath.Join(directory, fmt.Sprintf("entry-%04d", index)), nil, 0600); err != nil {
				t.Fatal(err)
			}
		}
		if _, err := ImportProject(root, fixtureLock(t), extension.DefaultSupport); err != nil {
			t.Fatalf("4094 content entries: %v", err)
		}
		if err := os.WriteFile(filepath.Join(directory, "one-too-many"), nil, 0600); err != nil {
			t.Fatal(err)
		}
		if _, err := ImportProject(root, fixtureLock(t), extension.DefaultSupport); err == nil || !strings.Contains(err.Error(), "entries") {
			t.Fatalf("4095 content entries result = %v", err)
		}
	})
}

func TestImportProjectFinalPassRejectsEarlierFileMutation(t *testing.T) {
	root := writeProjectFiles(t, v1Files(t))
	mutated := false
	_, err := importProjectWithHook(root, fixtureLock(t), extension.DefaultSupport, func(name string) error {
		if mutated || name != ManifestPath {
			return nil
		}
		mutated = true
		manifestPath := filepath.Join(root, filepath.FromSlash(ManifestPath))
		file, err := os.OpenFile(manifestPath, os.O_WRONLY, 0)
		if err != nil {
			return err
		}
		if _, err := file.WriteAt([]byte("#"), 0); err != nil {
			file.Close()
			return err
		}
		if err := file.Sync(); err != nil {
			file.Close()
			return err
		}
		if err := file.Close(); err != nil {
			return err
		}
		changed := time.Now().Add(2 * time.Second)
		return os.Chtimes(manifestPath, changed, changed)
	})
	if err == nil || !strings.Contains(err.Error(), "changed") {
		t.Fatalf("earlier-file mutation result = %v", err)
	}
}

func TestBoundedProjectFSStopsAtRemainingPlusOne(t *testing.T) {
	rootName := t.TempDir()
	if err := os.WriteFile(filepath.Join(rootName, "one"), nil, 0600); err != nil {
		t.Fatal(err)
	}
	root, info, _, err := openStableProjectRoot(rootName)
	if err != nil {
		t.Fatal(err)
	}
	defer root.Close()
	filesystem := &boundedProjectFS{root: root, remaining: 0, expected: map[string]os.FileInfo{".": info}}
	if _, err := filesystem.ReadDir("."); err == nil || !strings.Contains(err.Error(), "traversal") {
		t.Fatalf("zero-remaining ReadDir result = %v", err)
	}
}

func writeProjectFiles(t *testing.T, files map[string][]byte) string {
	t.Helper()
	root := t.TempDir()
	for name, data := range files {
		fullName := filepath.Join(root, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(fullName), 0700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(fullName, data, 0600); err != nil {
			t.Fatal(err)
		}
	}
	return root
}
