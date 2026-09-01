// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package archive

import (
	"bytes"
	"fmt"
	"sort"
	"strings"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/manifest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/packagepath"
)

// Entry is one immutable canonical regular content entry. Envelope files are
// exposed separately because they do not participate in the content hash.
type Entry struct {
	path string
	data []byte
}

func (entry Entry) Path() string  { return entry.path }
func (entry Entry) Bytes() []byte { return append([]byte(nil), entry.data...) }

// Package is the canonical Host preservation model shared by archive import,
// project load, deterministic export, and the Creator service.
type Package struct {
	manifestRaw []byte
	document    manifest.Document
	lock        dependency.ExactLock
	lockRaw     []byte
	artifact    manifest.ArtifactIdentity
	entries     []Entry
	extensions  []extension.Document
	support     extension.Support
	contentHash model.ContentHash
	sourceHash  model.ContentHash
	hasSource   bool
	expandedMax uint64
}

// FromFiles constructs a canonical package from caller-owned project files.
// META-INF is platform-owned and therefore rejected at this boundary.
func FromFiles(files map[string][]byte, lock dependency.ExactLock, support extension.Support) (*Package, error) {
	return fromFilesWithLimit(files, lock, support, MaxArchiveExpandedBytes)
}

func fromFilesWithLimit(files map[string][]byte, lock dependency.ExactLock, support extension.Support, expandedLimit uint64) (*Package, error) {
	envelopeBytes, err := exactSourceEnvelopeBytes(files, lock)
	if err != nil {
		return nil, err
	}
	owned, err := cloneValidatedContentFiles(files, envelopeBytes, expandedLimit)
	if err != nil {
		return nil, err
	}
	return buildPackage(owned, lock, support, true, "", false, envelopeBytes, expandedLimit)
}

// exactSourceEnvelopeBytes sizes the canonical outer envelope before any
// caller content is cloned. Content hashes have one fixed canonical width, so
// a zero digest produces the exact lock and artifact byte lengths for the
// eventual source content hash without requiring the content copy first.
func exactSourceEnvelopeBytes(files map[string][]byte, lock dependency.ExactLock) (uint64, error) {
	if _, err := validateContentFiles(files); err != nil {
		return 0, err
	}
	manifestRaw, exists := files[ManifestPath]
	if !exists {
		return 0, fmt.Errorf("package content is missing %s", ManifestPath)
	}
	document, err := manifest.Parse(manifestRaw)
	if err != nil {
		return 0, fmt.Errorf("parse package manifest: %w", err)
	}
	if document.Package == nil {
		return 0, fmt.Errorf("Game Package archive requires artifact_type package")
	}
	placeholder, err := model.ParseContentHash("sha256:" + strings.Repeat("0", 64))
	if err != nil {
		return 0, err
	}
	rebound, err := rebindRootContentHash(lock, *document.Package, placeholder)
	if err != nil {
		return 0, err
	}
	identity, err := manifest.BuildArtifactIdentity(*document.Package, placeholder, rebound)
	if err != nil {
		return 0, fmt.Errorf("build artifact envelope: %w", err)
	}
	lockJSON, err := rebound.CanonicalJSON()
	if err != nil {
		return 0, err
	}
	artifactJSON := identity.CanonicalJSON()
	if err := validateCanonicalEnvelopeEntries(lockJSON, artifactJSON); err != nil {
		return 0, err
	}
	return uint64(len(lockJSON)) + uint64(len(artifactJSON)), nil
}

// Import validates an immutable ZIP snapshot and its exact outer envelope.
func Import(snapshot Snapshot, support extension.Support) (*Package, error) {
	return importWithLimit(snapshot, support, MaxArchiveExpandedBytes)
}

func importWithLimit(snapshot Snapshot, support extension.Support, expandedLimit uint64) (*Package, error) {
	files, err := readSnapshot(snapshot)
	if err != nil {
		return nil, err
	}
	lockRaw, lockExists := files[PlatformLockPath]
	artifactRaw, artifactExists := files[ArtifactPath]
	if !lockExists || !artifactExists {
		return nil, fmt.Errorf("archive requires %s and %s", PlatformLockPath, ArtifactPath)
	}
	delete(files, PlatformLockPath)
	delete(files, ArtifactPath)
	lock, err := dependency.ParseExactLock(lockRaw)
	if err != nil {
		return nil, fmt.Errorf("parse platform lock envelope: %w", err)
	}
	canonicalLock, err := lock.CanonicalJSON()
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(lockRaw, canonicalLock) {
		return nil, fmt.Errorf("platform lock envelope is not exact canonical JSON")
	}
	expectedArtifact, err := expectedImportedArtifact(files, lock)
	if err != nil {
		return nil, err
	}
	if err := validateCanonicalEnvelopeEntries(canonicalLock, expectedArtifact); err != nil {
		return nil, err
	}
	if !bytes.Equal(artifactRaw, expectedArtifact) {
		return nil, fmt.Errorf("artifact identity envelope does not match the exact lock and package manifest")
	}
	envelopeBytes := uint64(len(canonicalLock)) + uint64(len(expectedArtifact))
	result, err := buildPackage(files, lock, support, false, snapshot.Hash(), true, envelopeBytes, expandedLimit)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(artifactRaw, result.artifact.CanonicalJSON()) {
		return nil, fmt.Errorf("artifact identity envelope does not match canonical package content")
	}
	return result, nil
}

func expectedImportedArtifact(files map[string][]byte, lock dependency.ExactLock) ([]byte, error) {
	manifestRaw, exists := files[ManifestPath]
	if !exists {
		return nil, fmt.Errorf("package content is missing %s", ManifestPath)
	}
	document, err := manifest.Parse(manifestRaw)
	if err != nil {
		return nil, fmt.Errorf("parse package manifest: %w", err)
	}
	if document.Package == nil {
		return nil, fmt.Errorf("Game Package archive requires artifact_type package")
	}
	root, exists := lock.Package(document.Package.PackageID)
	if !exists {
		return nil, fmt.Errorf("exact lock does not contain manifest package %q", document.Package.PackageID)
	}
	identity, err := manifest.BuildArtifactIdentity(*document.Package, root.ContentHash, lock)
	if err != nil {
		return nil, fmt.Errorf("build expected artifact envelope: %w", err)
	}
	return identity.CanonicalJSON(), nil
}

func validateCanonicalEnvelopeEntries(lockJSON, artifactJSON []byte) error {
	if len(lockJSON) > MaxEntryExpandedBytes {
		return fmt.Errorf("canonical platform lock envelope exceeds %d bytes", MaxEntryExpandedBytes)
	}
	if len(artifactJSON) > MaxEntryExpandedBytes {
		return fmt.Errorf("canonical artifact identity envelope exceeds %d bytes", MaxEntryExpandedBytes)
	}
	return nil
}

// ImportBytes first establishes the 80 MiB immutable snapshot boundary.
func ImportBytes(data []byte, support extension.Support) (*Package, error) {
	snapshot, err := NewSnapshot(data)
	if err != nil {
		return nil, err
	}
	return Import(snapshot, support)
}

func buildPackage(files map[string][]byte, lock dependency.ExactLock, support extension.Support, rebindRoot bool, sourceHash model.ContentHash, hasSource bool, envelopeBytes, expandedLimit uint64) (*Package, error) {
	rawBytes, err := validateContentFiles(files)
	if err != nil {
		return nil, err
	}
	budget, err := newCanonicalContentBudget(rawBytes, envelopeBytes, expandedLimit)
	if err != nil {
		return nil, err
	}
	manifestRaw, exists := files[ManifestPath]
	if !exists {
		return nil, fmt.Errorf("package content is missing %s", ManifestPath)
	}
	document, err := manifest.Parse(manifestRaw)
	if err != nil {
		return nil, fmt.Errorf("parse package manifest: %w", err)
	}
	if document.Package == nil {
		return nil, fmt.Errorf("Game Package archive requires artifact_type package")
	}
	if err := validateExtensionOwnership(files, document.Package.Extensions); err != nil {
		return nil, err
	}

	extensions := make([]extension.Document, 0, len(document.Package.Extensions))
	if document.Package.SchemaVersion == manifest.ExtensionSchemaVersion {
		canonicalManifest, err := manifest.CanonicalTOML(document)
		if err != nil {
			return nil, fmt.Errorf("canonicalize package manifest: %w", err)
		}
		if len(canonicalManifest) > manifest.MaxManifestBytes {
			return nil, fmt.Errorf("canonical manifest exceeds %d bytes", manifest.MaxManifestBytes)
		}
		if err := budget.replace(len(manifestRaw), len(canonicalManifest)); err != nil {
			return nil, fmt.Errorf("canonical manifest: %w", err)
		}
		files[ManifestPath] = canonicalManifest
		manifestRaw = canonicalManifest
		for _, descriptor := range document.Package.Extensions {
			payload, exists := files[descriptor.PayloadPath]
			if !exists {
				return nil, fmt.Errorf("extension payload %q is absent", boundedPath(descriptor.PayloadPath))
			}
			canonicalLimit, err := budget.replacementLimit(len(payload), extension.MaxPayloadBytes)
			if err != nil {
				return nil, err
			}
			item, err := extension.LoadWithCanonicalLimit(descriptor, files, support, canonicalLimit)
			if err != nil {
				return nil, err
			}
			extensions = append(extensions, item)
			if item.Status == extension.Supported {
				canonical := item.CanonicalPayload()
				if err := budget.replace(len(payload), len(canonical)); err != nil {
					return nil, err
				}
				files[descriptor.PayloadPath] = canonical
			}
		}
	}
	entries, err := canonicalEntries(files)
	if err != nil {
		return nil, err
	}
	contentHash, err := computeContentHash(entries)
	if err != nil {
		return nil, err
	}
	effectiveLock := lock
	if rebindRoot {
		effectiveLock, err = rebindRootContentHash(lock, *document.Package, contentHash)
		if err != nil {
			return nil, err
		}
	}
	identity, err := manifest.BuildArtifactIdentity(*document.Package, contentHash, effectiveLock)
	if err != nil {
		return nil, fmt.Errorf("build artifact envelope: %w", err)
	}
	lockJSON, err := effectiveLock.CanonicalJSON()
	if err != nil {
		return nil, err
	}
	if err := validateExportLimits(entries, lockJSON, identity.CanonicalJSON()); err != nil {
		return nil, err
	}
	return &Package{
		manifestRaw: append([]byte(nil), manifestRaw...), document: document,
		lock: effectiveLock, lockRaw: append([]byte(nil), lockJSON...), artifact: identity,
		entries: entries, extensions: extensions, support: support, contentHash: contentHash,
		sourceHash: sourceHash, hasSource: hasSource, expandedMax: expandedLimit,
	}, nil
}

func rebindRootContentHash(lock dependency.ExactLock, pkg manifest.Package, contentHash model.ContentHash) (dependency.ExactLock, error) {
	if lock.Root() != pkg.PackageID {
		return dependency.ExactLock{}, fmt.Errorf("source exact lock root %q does not match package_id %q", lock.Root(), pkg.PackageID)
	}
	nodes := lock.Packages()
	found := false
	for index := range nodes {
		if nodes[index].PackageID != pkg.PackageID {
			continue
		}
		if nodes[index].Version != pkg.Version {
			return dependency.ExactLock{}, fmt.Errorf("source exact lock root version %q does not match manifest version %q", nodes[index].Version, pkg.Version)
		}
		nodes[index].ContentHash = contentHash
		found = true
		break
	}
	if !found {
		return dependency.ExactLock{}, fmt.Errorf("source exact lock does not contain package root %q", pkg.PackageID)
	}
	result, err := dependency.BuildExactLock(lock.Root().String(), nodes)
	if err != nil {
		return dependency.ExactLock{}, fmt.Errorf("rebind source exact lock: %w", err)
	}
	return result, nil
}

// ReplaceExtension validates a JSON edit through the retained schema, then
// rebuilds the same canonical model and rebinds only the exact lock root hash.
// Unsupported optional documents remain read-only through ValidateReplacement.
func (pkg *Package) ReplaceExtension(namespace string, data []byte) (*Package, error) {
	var selected *extension.Document
	for index := range pkg.extensions {
		if pkg.extensions[index].Descriptor.Namespace == namespace {
			selected = &pkg.extensions[index]
			break
		}
	}
	if selected == nil {
		return nil, fmt.Errorf("extension namespace %q is not present", namespace)
	}
	files := make(map[string][]byte, len(pkg.entries))
	var contentBytes uint64
	for _, entry := range pkg.entries {
		files[entry.path] = entry.data
		contentBytes += uint64(len(entry.data))
	}
	payload := files[selected.Descriptor.PayloadPath]
	envelopeBytes := uint64(len(pkg.lockRaw)) + uint64(len(pkg.artifact.CanonicalJSON()))
	budget, err := newCanonicalContentBudget(contentBytes, envelopeBytes, pkg.expandedMax)
	if err != nil {
		return nil, err
	}
	canonicalLimit, err := budget.replacementLimit(len(payload), extension.MaxPayloadBytes)
	if err != nil {
		return nil, err
	}
	canonical, err := selected.ValidateReplacementWithCanonicalLimit(data, canonicalLimit)
	if err != nil {
		return nil, err
	}
	files[selected.Descriptor.PayloadPath] = canonical
	return buildPackage(files, pkg.lock, pkg.support, true, pkg.sourceHash, pkg.hasSource, envelopeBytes, pkg.expandedMax)
}

type canonicalContentBudget struct {
	content  uint64
	envelope uint64
	limit    uint64
}

func newCanonicalContentBudget(content, envelope, limit uint64) (canonicalContentBudget, error) {
	budget := canonicalContentBudget{content: content, envelope: envelope, limit: limit}
	if envelope > limit || content > limit-envelope {
		return canonicalContentBudget{}, fmt.Errorf("package content and canonical envelope exceed %d bytes", limit)
	}
	return budget, nil
}

func (budget canonicalContentBudget) replacementLimit(oldBytes, perEntryLimit int) (int, error) {
	if oldBytes < 0 || perEntryLimit < 0 || uint64(oldBytes) > budget.content {
		return 0, fmt.Errorf("invalid canonical content budget accounting")
	}
	contentWithoutOld := budget.content - uint64(oldBytes)
	if budget.envelope > budget.limit || contentWithoutOld > budget.limit-budget.envelope {
		return 0, fmt.Errorf("package content and canonical envelope exceed %d bytes", budget.limit)
	}
	remaining := budget.limit - budget.envelope - contentWithoutOld
	if remaining > uint64(perEntryLimit) {
		remaining = uint64(perEntryLimit)
	}
	return int(remaining), nil
}

func (budget *canonicalContentBudget) replace(oldBytes, newBytes int) error {
	allowed, err := budget.replacementLimit(oldBytes, newBytes)
	if err != nil {
		return err
	}
	if newBytes > allowed {
		return fmt.Errorf("package content and canonical envelope exceed %d bytes", budget.limit)
	}
	budget.content = budget.content - uint64(oldBytes) + uint64(newBytes)
	return nil
}

func validateContentFiles(files map[string][]byte) (uint64, error) {
	if len(files)+2 > MaxArchiveEntries {
		return 0, fmt.Errorf("package content plus envelope has %d entries, maximum is %d", len(files)+2, MaxArchiveEntries)
	}
	var paths packagepath.TreeSet
	var total uint64
	for name, data := range files {
		if err := paths.AddFile(name); err != nil {
			return 0, fmt.Errorf("project entry path: %w", err)
		}
		first := name
		if slash := strings.IndexByte(first, '/'); slash >= 0 {
			first = first[:slash]
		}
		if packagepath.CollisionKey(first) == packagepath.CollisionKey("META-INF") {
			return 0, fmt.Errorf("project entry %q uses platform-owned META-INF", boundedPath(name))
		}
		if len(data) > MaxEntryExpandedBytes {
			return 0, fmt.Errorf("project entry %q exceeds %d bytes", boundedPath(name), MaxEntryExpandedBytes)
		}
		total += uint64(len(data))
		if total > MaxArchiveExpandedBytes {
			return 0, fmt.Errorf("project content exceeds %d bytes", MaxArchiveExpandedBytes)
		}
	}
	return total, nil
}

func validateExtensionOwnership(files map[string][]byte, descriptors []extension.Descriptor) error {
	owners := make(map[string]string, len(descriptors)*2)
	for _, descriptor := range descriptors {
		owners[packagepath.CollisionKey(descriptor.SchemaPath)] = descriptor.Namespace
		owners[packagepath.CollisionKey(descriptor.PayloadPath)] = descriptor.Namespace
	}
	for name := range files {
		first := name
		if slash := strings.IndexByte(first, '/'); slash >= 0 {
			first = first[:slash]
		}
		if packagepath.CollisionKey(first) != packagepath.CollisionKey("extensions") {
			continue
		}
		if _, exists := owners[packagepath.CollisionKey(name)]; !exists {
			return fmt.Errorf("extension entry %q is not declared by exactly one descriptor", boundedPath(name))
		}
	}
	return nil
}

func canonicalEntries(files map[string][]byte) ([]Entry, error) {
	entries := make([]Entry, 0, len(files))
	var total uint64
	for name, data := range files {
		if len(data) > MaxEntryExpandedBytes {
			return nil, fmt.Errorf("canonical entry %q exceeds %d bytes", boundedPath(name), MaxEntryExpandedBytes)
		}
		total += uint64(len(data))
		if total > MaxArchiveExpandedBytes {
			return nil, fmt.Errorf("canonical content exceeds %d bytes", MaxArchiveExpandedBytes)
		}
		entries = append(entries, Entry{path: name, data: data})
	}
	sort.Slice(entries, func(i, j int) bool { return entries[i].path < entries[j].path })
	return entries, nil
}

func cloneValidatedContentFiles(files map[string][]byte, envelopeBytes, expandedLimit uint64) (map[string][]byte, error) {
	total, err := validateContentFiles(files)
	if err != nil {
		return nil, err
	}
	if _, err := newCanonicalContentBudget(total, envelopeBytes, expandedLimit); err != nil {
		return nil, err
	}
	result := make(map[string][]byte, len(files))
	for name, data := range files {
		result[name] = append([]byte(nil), data...)
	}
	return result, nil
}

func validateExportLimits(entries []Entry, lockJSON, artifactJSON []byte) error {
	if len(lockJSON) > MaxEntryExpandedBytes || len(artifactJSON) > MaxEntryExpandedBytes {
		return fmt.Errorf("generated envelope entry exceeds %d bytes", MaxEntryExpandedBytes)
	}
	total := uint64(len(lockJSON) + len(artifactJSON))
	for _, entry := range entries {
		total += uint64(len(entry.data))
	}
	if total > MaxArchiveExpandedBytes {
		return fmt.Errorf("canonical content and envelope exceed %d bytes", MaxArchiveExpandedBytes)
	}
	return nil
}

// ManifestBytes returns the raw v1 manifest or canonical v2 manifest.
func (pkg *Package) ManifestBytes() []byte { return append([]byte(nil), pkg.manifestRaw...) }

// Manifest reparses an isolated copy so public slices and pointers cannot
// mutate the model retained for export.
func (pkg *Package) Manifest() (manifest.Document, error) { return manifest.Parse(pkg.manifestRaw) }

func (pkg *Package) ExactLock() dependency.ExactLock { return pkg.lock }
func (pkg *Package) LockBytes() []byte               { return append([]byte(nil), pkg.lockRaw...) }
func (pkg *Package) ArtifactIdentity() manifest.ArtifactIdentity {
	return pkg.artifact
}
func (pkg *Package) ArtifactBytes() []byte          { return pkg.artifact.CanonicalJSON() }
func (pkg *Package) ContentHash() model.ContentHash { return pkg.contentHash }
func (pkg *Package) Support() extension.Support     { return pkg.support }
func (pkg *Package) Extensions() []extension.Document {
	return append([]extension.Document(nil), pkg.extensions...)
}
func (pkg *Package) Entries() []Entry { return append([]Entry(nil), pkg.entries...) }
func (pkg *Package) SourceArchiveHash() (model.ContentHash, bool) {
	return pkg.sourceHash, pkg.hasSource
}

// Entry returns an immutable canonical content entry by exact path.
func (pkg *Package) Entry(name string) (Entry, bool) {
	index := sort.Search(len(pkg.entries), func(index int) bool { return pkg.entries[index].path >= name })
	if index >= len(pkg.entries) || pkg.entries[index].path != name {
		return Entry{}, false
	}
	return pkg.entries[index], true
}
