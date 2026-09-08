// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package archive

import (
	"fmt"
	"io"
	"io/fs"
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/manifest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/packagepath"
)

// Empty directories do not become archive entries, but they still consume
// traversal work. This bound is deliberately separate from the exact 4094
// regular-content-entry limit.
const maxProjectTraversalNodes = MaxArchiveEntries * 2

type projectFileMetadata struct {
	path string
	info os.FileInfo
}

type projectDirectoryMetadata struct {
	path string
	info os.FileInfo
}

// ImportProject loads regular project content through an os.Root confinement
// boundary. Traversal and metadata validation finish before any non-manifest
// content allocation, then the exact canonical envelope is reserved.
func ImportProject(name string, lock dependency.ExactLock, support extension.Support) (*Package, error) {
	return importProjectWithHook(name, lock, support, nil)
}

func importProjectWithHook(name string, lock dependency.ExactLock, support extension.Support, afterRead func(string) error) (*Package, error) {
	root, rootInfo, absoluteName, err := openStableProjectRoot(name)
	if err != nil {
		return nil, err
	}
	defer root.Close()

	walkFS := &boundedProjectFS{
		root: root, remaining: maxProjectTraversalNodes,
		expected: map[string]os.FileInfo{".": rootInfo},
	}
	var tree packagepath.TreeSet
	files := make([]projectFileMetadata, 0)
	directories := make([]projectDirectoryMetadata, 0)
	var declaredBytes uint64
	err = fs.WalkDir(walkFS, ".", func(name string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if name == "." {
			return nil
		}
		if !fs.ValidPath(name) {
			return fmt.Errorf("project walk produced invalid path %q", boundedPath(name))
		}
		if err := packagepath.Validate(name); err != nil {
			return fmt.Errorf("project entry path: %w", err)
		}
		if usesPlatformEnvelope(name) {
			return fmt.Errorf("project entry %q uses platform-owned META-INF", boundedPath(name))
		}
		if err := verifyProjectPrefixes(root, name); err != nil {
			return err
		}
		observed, err := entry.Info()
		if err != nil {
			return fmt.Errorf("inspect project entry %q: %w", boundedPath(name), err)
		}
		current, err := root.Lstat(filepath.FromSlash(name))
		if err != nil {
			return fmt.Errorf("reinspect project entry %q: %w", boundedPath(name), err)
		}
		if err := requireSameProjectMetadata(observed, current); err != nil {
			return fmt.Errorf("project entry %q changed during traversal: %w", boundedPath(name), err)
		}
		mode := current.Mode()
		switch {
		case mode&os.ModeSymlink != 0:
			return fmt.Errorf("project entry %q is a symbolic link", boundedPath(name))
		case mode.IsDir():
			if err := tree.AddDirectory(name); err != nil {
				return fmt.Errorf("project directory path: %w", err)
			}
			directories = append(directories, projectDirectoryMetadata{path: name, info: current})
		case mode.IsRegular():
			if len(files)+1+2 > MaxArchiveEntries {
				return fmt.Errorf("project content plus envelope exceeds %d entries", MaxArchiveEntries)
			}
			if err := tree.AddFile(name); err != nil {
				return fmt.Errorf("project entry path: %w", err)
			}
			if current.Size() < 0 || current.Size() > MaxEntryExpandedBytes {
				return fmt.Errorf("project entry %q exceeds %d bytes", boundedPath(name), MaxEntryExpandedBytes)
			}
			if name == ManifestPath && current.Size() > manifest.MaxManifestBytes {
				return fmt.Errorf("project manifest exceeds %d bytes", manifest.MaxManifestBytes)
			}
			declaredBytes += uint64(current.Size())
			if declaredBytes > MaxArchiveExpandedBytes {
				return fmt.Errorf("project content exceeds %d bytes", MaxArchiveExpandedBytes)
			}
			files = append(files, projectFileMetadata{path: name, info: current})
		default:
			return fmt.Errorf("project entry %q is not a regular file or directory", boundedPath(name))
		}
		return nil
	})
	if err != nil {
		return nil, fmt.Errorf("walk project root: %w", err)
	}
	sort.Slice(files, func(i, j int) bool { return files[i].path < files[j].path })
	manifestIndex := sort.Search(len(files), func(index int) bool { return files[index].path >= ManifestPath })
	if manifestIndex == len(files) || files[manifestIndex].path != ManifestPath {
		return nil, fmt.Errorf("package content is missing %s", ManifestPath)
	}
	manifestRaw, err := readStableProjectFile(root, files[manifestIndex], manifest.MaxManifestBytes)
	if err != nil {
		return nil, err
	}
	if afterRead != nil {
		if err := afterRead(ManifestPath); err != nil {
			return nil, fmt.Errorf("project test hook after %q: %w", ManifestPath, err)
		}
	}
	document, err := manifest.Parse(manifestRaw)
	if err != nil {
		return nil, fmt.Errorf("parse package manifest: %w", err)
	}
	if document.Package == nil {
		return nil, fmt.Errorf("Game Package project requires artifact_type package")
	}
	if err := validateProjectExtensionSizes(files, *document.Package); err != nil {
		return nil, err
	}
	envelopeBytes, err := exactSourceEnvelopeBytes(map[string][]byte{ManifestPath: manifestRaw}, lock)
	if err != nil {
		return nil, err
	}
	if envelopeBytes > MaxArchiveExpandedBytes || declaredBytes > MaxArchiveExpandedBytes-envelopeBytes {
		return nil, fmt.Errorf("project content and canonical envelope exceed %d bytes", MaxArchiveExpandedBytes)
	}

	owned := make(map[string][]byte, len(files))
	owned[ManifestPath] = manifestRaw
	for index, metadata := range files {
		if index == manifestIndex {
			continue
		}
		data, err := readStableProjectFile(root, metadata, MaxEntryExpandedBytes)
		if err != nil {
			return nil, err
		}
		owned[metadata.path] = data
		if afterRead != nil {
			if err := afterRead(metadata.path); err != nil {
				return nil, fmt.Errorf("project test hook after %q: %w", boundedPath(metadata.path), err)
			}
		}
	}
	for _, file := range files {
		if err := verifyStableProjectPath(root, file.path, file.info, false); err != nil {
			return nil, err
		}
	}
	for _, directory := range directories {
		if err := verifyStableProjectPath(root, directory.path, directory.info, true); err != nil {
			return nil, err
		}
	}
	verificationRoot, verificationInfo, err := openProjectRootComponents(absoluteName)
	if err != nil {
		return nil, fmt.Errorf("reopen project root after read: %w", err)
	}
	defer verificationRoot.Close()
	if err := requireStableProjectDirectory(rootInfo, verificationInfo); err != nil {
		return nil, fmt.Errorf("project root changed while reading: %w", err)
	}
	return buildPackage(owned, lock, support, true, "", false, envelopeBytes, MaxArchiveExpandedBytes)
}

type boundedProjectFS struct {
	root      *os.Root
	remaining int
	expected  map[string]os.FileInfo
}

func (filesystem *boundedProjectFS) Open(name string) (fs.File, error) {
	return filesystem.root.Open(filepath.FromSlash(name))
}

// ReadDir reads at most remaining+1 entries from any directory before
// rejecting the traversal work budget, instead of allowing fs.WalkDir's
// default ReadDir(-1) to allocate an attacker-sized directory listing.
func (filesystem *boundedProjectFS) ReadDir(name string) ([]fs.DirEntry, error) {
	if err := verifyProjectPrefixes(filesystem.root, name); err != nil {
		return nil, err
	}
	localName := filepath.FromSlash(name)
	pathBefore, err := filesystem.root.Lstat(localName)
	if err != nil {
		return nil, err
	}
	if err := requireProjectDirectory(pathBefore); err != nil {
		return nil, err
	}
	if expected, exists := filesystem.expected[name]; exists {
		if err := requireSameProjectMetadata(expected, pathBefore); err != nil {
			return nil, fmt.Errorf("project directory %q changed before enumeration: %w", boundedPath(name), err)
		}
	}
	directory, err := filesystem.root.Open(localName)
	if err != nil {
		return nil, err
	}
	fdBefore, err := directory.Stat()
	if err != nil {
		directory.Close()
		return nil, err
	}
	pathOpened, err := filesystem.root.Lstat(localName)
	if err != nil {
		directory.Close()
		return nil, err
	}
	if err := requireStableProjectDirectory(pathBefore, fdBefore, pathOpened); err != nil {
		directory.Close()
		return nil, err
	}
	entries := make([]fs.DirEntry, 0)
	for {
		request := 128
		if filesystem.remaining < request {
			request = filesystem.remaining + 1
		}
		batch, readErr := directory.ReadDir(request)
		if len(batch) > filesystem.remaining {
			directory.Close()
			return nil, fmt.Errorf("project traversal exceeds %d nodes", maxProjectTraversalNodes)
		}
		filesystem.remaining -= len(batch)
		for _, raw := range batch {
			child := raw.Name()
			if name != "." {
				child = path.Join(name, child)
			}
			info, statErr := filesystem.root.Lstat(filepath.FromSlash(child))
			if statErr != nil {
				directory.Close()
				return nil, statErr
			}
			filesystem.expected[child] = info
			entries = append(entries, fs.FileInfoToDirEntry(info))
		}
		if readErr == io.EOF {
			break
		}
		if readErr != nil {
			directory.Close()
			return nil, readErr
		}
	}
	fdAfter, err := directory.Stat()
	if err != nil {
		directory.Close()
		return nil, err
	}
	pathAfter, err := filesystem.root.Lstat(localName)
	if err != nil {
		directory.Close()
		return nil, err
	}
	if err := requireStableProjectDirectory(fdBefore, fdAfter, pathAfter); err != nil {
		directory.Close()
		return nil, err
	}
	if err := directory.Close(); err != nil {
		return nil, err
	}
	sort.Slice(entries, func(i, j int) bool { return entries[i].Name() < entries[j].Name() })
	return entries, nil
}

func openStableProjectRoot(name string) (*os.Root, os.FileInfo, string, error) {
	absoluteName, err := filepath.Abs(name)
	if err != nil {
		return nil, nil, "", fmt.Errorf("resolve project root %q: %w", boundedPath(name), err)
	}
	root, info, err := openProjectRootComponents(absoluteName)
	if err != nil {
		return nil, nil, "", fmt.Errorf("open project root %q: %w", boundedPath(name), err)
	}
	return root, info, absoluteName, nil
}

func openProjectRootComponents(absoluteName string) (*os.Root, os.FileInfo, error) {
	absoluteName = filepath.Clean(absoluteName)
	volume := filepath.VolumeName(absoluteName)
	anchor := volume + string(filepath.Separator)
	if anchor == "" {
		anchor = string(filepath.Separator)
	}
	root, err := os.OpenRoot(anchor)
	if err != nil {
		return nil, nil, err
	}
	remainder := strings.TrimPrefix(absoluteName, volume)
	remainder = strings.TrimLeft(remainder, `/\`)
	components := []string(nil)
	if remainder != "" {
		components = strings.Split(remainder, string(filepath.Separator))
	}
	for _, component := range components {
		before, err := root.Lstat(component)
		if err != nil {
			root.Close()
			return nil, nil, err
		}
		if err := requireProjectDirectory(before); err != nil {
			root.Close()
			return nil, nil, fmt.Errorf("project root component %q: %w", boundedPath(component), err)
		}
		next, err := root.OpenRoot(component)
		if err != nil {
			root.Close()
			return nil, nil, err
		}
		opened, err := next.Stat(".")
		if err != nil {
			next.Close()
			root.Close()
			return nil, nil, err
		}
		after, err := root.Lstat(component)
		if err != nil {
			next.Close()
			root.Close()
			return nil, nil, err
		}
		if err := requireStableProjectDirectory(before, opened, after); err != nil {
			next.Close()
			root.Close()
			return nil, nil, fmt.Errorf("project root component %q changed: %w", boundedPath(component), err)
		}
		if err := root.Close(); err != nil {
			next.Close()
			return nil, nil, err
		}
		root = next
	}
	info, err := root.Stat(".")
	if err != nil {
		root.Close()
		return nil, nil, err
	}
	return root, info, nil
}

func readStableProjectFile(root *os.Root, metadata projectFileMetadata, maximum int64) ([]byte, error) {
	if err := verifyProjectPrefixes(root, metadata.path); err != nil {
		return nil, err
	}
	localName := filepath.FromSlash(metadata.path)
	pathBefore, err := root.Lstat(localName)
	if err != nil {
		return nil, fmt.Errorf("inspect project entry %q: %w", boundedPath(metadata.path), err)
	}
	if err := validateProjectRegularFile(pathBefore, maximum); err != nil {
		return nil, fmt.Errorf("project entry %q: %w", boundedPath(metadata.path), err)
	}
	if err := requireSameProjectMetadata(metadata.info, pathBefore); err != nil {
		return nil, fmt.Errorf("project entry %q changed before read: %w", boundedPath(metadata.path), err)
	}
	file, err := root.Open(localName)
	if err != nil {
		return nil, fmt.Errorf("open project entry %q: %w", boundedPath(metadata.path), err)
	}
	fdBefore, err := file.Stat()
	if err != nil {
		file.Close()
		return nil, err
	}
	pathOpened, err := root.Lstat(localName)
	if err != nil {
		file.Close()
		return nil, err
	}
	if err := validateProjectRegularFile(fdBefore, maximum); err != nil {
		file.Close()
		return nil, err
	}
	if err := requireSameProjectMetadata(pathBefore, fdBefore, pathOpened); err != nil {
		file.Close()
		return nil, fmt.Errorf("project entry %q changed while opening: %w", boundedPath(metadata.path), err)
	}
	data := make([]byte, int(fdBefore.Size()))
	if _, err := io.ReadFull(file, data); err != nil {
		file.Close()
		return nil, fmt.Errorf("read project entry %q: %w", boundedPath(metadata.path), err)
	}
	var extra [1]byte
	count, tailErr := file.Read(extra[:])
	if count != 0 || tailErr != io.EOF {
		file.Close()
		if tailErr == nil {
			tailErr = fmt.Errorf("file grew past its declared size")
		}
		return nil, fmt.Errorf("read project entry %q trailer: %w", boundedPath(metadata.path), tailErr)
	}
	fdAfter, err := file.Stat()
	if err != nil {
		file.Close()
		return nil, err
	}
	pathAfter, err := root.Lstat(localName)
	if err != nil {
		file.Close()
		return nil, err
	}
	if err := requireSameProjectMetadata(fdBefore, fdAfter, pathAfter); err != nil {
		file.Close()
		return nil, fmt.Errorf("project entry %q changed while reading: %w", boundedPath(metadata.path), err)
	}
	if err := verifyProjectPrefixes(root, metadata.path); err != nil {
		file.Close()
		return nil, err
	}
	if err := file.Close(); err != nil {
		return nil, err
	}
	return data, nil
}

func verifyProjectPrefixes(root *os.Root, name string) error {
	if name == "." {
		return nil
	}
	components := strings.Split(name, "/")
	for index := 1; index < len(components); index++ {
		prefix := strings.Join(components[:index], "/")
		info, err := root.Lstat(filepath.FromSlash(prefix))
		if err != nil {
			return fmt.Errorf("inspect project path prefix %q: %w", boundedPath(prefix), err)
		}
		if err := requireProjectDirectory(info); err != nil {
			return fmt.Errorf("project path prefix %q: %w", boundedPath(prefix), err)
		}
	}
	return nil
}

func verifyStableProjectPath(root *os.Root, name string, expected os.FileInfo, directory bool) error {
	if err := verifyProjectPrefixes(root, name); err != nil {
		return err
	}
	current, err := root.Lstat(filepath.FromSlash(name))
	if err != nil {
		return fmt.Errorf("reinspect project path %q: %w", boundedPath(name), err)
	}
	if directory {
		if err := requireProjectDirectory(current); err != nil {
			return err
		}
	} else {
		if err := validateProjectRegularFile(current, MaxEntryExpandedBytes); err != nil {
			return err
		}
	}
	if err := requireSameProjectMetadata(expected, current); err != nil {
		return fmt.Errorf("project path %q changed: %w", boundedPath(name), err)
	}
	return nil
}

func requireProjectDirectory(info os.FileInfo) error {
	if info.Mode()&os.ModeSymlink != 0 {
		return fmt.Errorf("symbolic links are forbidden")
	}
	if !info.IsDir() {
		return fmt.Errorf("not a directory")
	}
	return nil
}

func validateProjectRegularFile(info os.FileInfo, maximum int64) error {
	if info.Mode()&os.ModeSymlink != 0 {
		return fmt.Errorf("symbolic links are forbidden")
	}
	if !info.Mode().IsRegular() {
		return fmt.Errorf("not a regular file")
	}
	if info.Size() < 0 || info.Size() > maximum {
		return fmt.Errorf("size exceeds %d bytes", maximum)
	}
	return nil
}

func requireStableProjectDirectory(values ...os.FileInfo) error {
	for _, value := range values {
		if err := requireProjectDirectory(value); err != nil {
			return err
		}
	}
	return requireSameProjectMetadata(values...)
}

func requireSameProjectMetadata(values ...os.FileInfo) error {
	if len(values) < 2 {
		return nil
	}
	first := values[0]
	for _, value := range values[1:] {
		if !os.SameFile(first, value) {
			return fmt.Errorf("path and opened descriptor identify different filesystem objects")
		}
		if first.Mode().Type() != value.Mode().Type() || first.Size() != value.Size() || !first.ModTime().Equal(value.ModTime()) {
			return fmt.Errorf("filesystem object type, size, or modification time changed")
		}
	}
	return nil
}

func usesPlatformEnvelope(name string) bool {
	first := name
	if slash := strings.IndexByte(first, '/'); slash >= 0 {
		first = first[:slash]
	}
	return packagepath.CollisionKey(first) == packagepath.CollisionKey("META-INF")
}

func validateProjectExtensionSizes(files []projectFileMetadata, pkg manifest.Package) error {
	byPath := make(map[string]os.FileInfo, len(files))
	for _, file := range files {
		byPath[file.path] = file.info
	}
	for _, descriptor := range pkg.Extensions {
		if info, exists := byPath[descriptor.SchemaPath]; exists && info.Size() > extension.MaxSchemaBytes {
			return fmt.Errorf("extension schema %q exceeds %d bytes", boundedPath(descriptor.SchemaPath), extension.MaxSchemaBytes)
		}
		if info, exists := byPath[descriptor.PayloadPath]; exists && info.Size() > extension.MaxPayloadBytes {
			return fmt.Errorf("extension payload %q exceeds %d bytes", boundedPath(descriptor.PayloadPath), extension.MaxPayloadBytes)
		}
	}
	return nil
}
