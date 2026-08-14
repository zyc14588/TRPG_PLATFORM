// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package archive

import (
	"fmt"
	"io"
	"os"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
)

// ImportFile reads one stable regular file into one owned immutable snapshot.
// The archive hash/conflict token and package model are therefore derived from
// exactly the same byte slice, with no path reopen between hashing and parse.
func ImportFile(name string, support extension.Support) (*Package, error) {
	return importFileWithHook(name, support, nil)
}

// importFileWithHook is a deterministic race-test seam. Production always
// passes nil; tests may mutate the opened inode after the exact read and must
// observe the stability checks reject it.
func importFileWithHook(name string, support extension.Support, afterRead func() error) (*Package, error) {
	pathBefore, err := os.Lstat(name)
	if err != nil {
		return nil, fmt.Errorf("inspect archive file %q: %w", boundedPath(name), err)
	}
	if err := validateArchiveFileInfo(pathBefore); err != nil {
		return nil, fmt.Errorf("archive file %q: %w", boundedPath(name), err)
	}
	file, err := os.Open(name)
	if err != nil {
		return nil, fmt.Errorf("open archive file %q: %w", boundedPath(name), err)
	}
	defer file.Close()
	fdBefore, err := file.Stat()
	if err != nil {
		return nil, fmt.Errorf("stat opened archive file: %w", err)
	}
	if err := validateArchiveFileInfo(fdBefore); err != nil {
		return nil, err
	}
	pathOpened, err := os.Lstat(name)
	if err != nil {
		return nil, fmt.Errorf("reinspect archive file: %w", err)
	}
	if err := requireStableRegularFile(pathBefore, fdBefore, pathOpened); err != nil {
		return nil, fmt.Errorf("archive file changed while opening: %w", err)
	}

	data := make([]byte, int(fdBefore.Size()))
	if _, err := io.ReadFull(file, data); err != nil {
		return nil, fmt.Errorf("read exact archive file: %w", err)
	}
	var extra [1]byte
	count, tailErr := file.Read(extra[:])
	if count != 0 || tailErr != io.EOF {
		if tailErr == nil {
			tailErr = fmt.Errorf("file grew past its declared size")
		}
		return nil, fmt.Errorf("read exact archive file trailer: %w", tailErr)
	}
	if afterRead != nil {
		if err := afterRead(); err != nil {
			return nil, fmt.Errorf("archive file test hook: %w", err)
		}
	}
	fdAfter, err := file.Stat()
	if err != nil {
		return nil, fmt.Errorf("restat opened archive file: %w", err)
	}
	pathAfter, err := os.Lstat(name)
	if err != nil {
		return nil, fmt.Errorf("reinspect archive file after read: %w", err)
	}
	if err := requireStableRegularFile(fdBefore, fdAfter, pathAfter); err != nil {
		return nil, fmt.Errorf("archive file changed while reading: %w", err)
	}
	snapshot, err := newOwnedSnapshot(data)
	if err != nil {
		return nil, err
	}
	return Import(snapshot, support)
}

func validateArchiveFileInfo(info os.FileInfo) error {
	if info.Mode()&os.ModeSymlink != 0 {
		return fmt.Errorf("symbolic links are forbidden")
	}
	if !info.Mode().IsRegular() {
		return fmt.Errorf("not a regular file")
	}
	if info.Size() < 0 || info.Size() > MaxSnapshotBytes {
		return fmt.Errorf("size exceeds %d bytes", MaxSnapshotBytes)
	}
	return nil
}

func requireStableRegularFile(expected, opened, path os.FileInfo) error {
	if err := validateArchiveFileInfo(opened); err != nil {
		return err
	}
	if err := validateArchiveFileInfo(path); err != nil {
		return err
	}
	if !os.SameFile(expected, opened) || !os.SameFile(opened, path) {
		return fmt.Errorf("path and opened descriptor identify different files")
	}
	if expected.Size() != opened.Size() || opened.Size() != path.Size() ||
		!expected.ModTime().Equal(opened.ModTime()) || !opened.ModTime().Equal(path.ModTime()) {
		return fmt.Errorf("file size or modification time changed")
	}
	return nil
}
