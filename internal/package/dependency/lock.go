// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package dependency defines deterministic, exact package dependency locks.
// It resolves no registries and performs no installation.
package dependency

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"regexp"
	"sort"
	"strings"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

const LockSchemaVersion = 1

var featurePattern = regexp.MustCompile(`^[a-z][a-z0-9_-]{0,63}$`)

// LockedPackage is one exact node in the fully transitive dependency graph.
type LockedPackage struct {
	PackageID    model.PackageID
	Version      model.Version
	ContentHash  model.ContentHash
	Features     []string
	Dependencies []model.PackageID
}

// NewLockedPackage validates and canonicalizes one exact graph node.
func NewLockedPackage(packageID, version, contentHash string, features, dependencies []string) (LockedPackage, error) {
	id, err := model.ParsePackageID(packageID)
	if err != nil {
		return LockedPackage{}, err
	}
	exactVersion, err := model.ParseVersion(version)
	if err != nil {
		return LockedPackage{}, err
	}
	hash, err := model.ParseContentHash(contentHash)
	if err != nil {
		return LockedPackage{}, err
	}
	normalizedFeatures, err := normalizeFeatures(features)
	if err != nil {
		return LockedPackage{}, err
	}
	dependencyIDs := make([]model.PackageID, 0, len(dependencies))
	seen := make(map[model.PackageID]struct{}, len(dependencies))
	for _, raw := range dependencies {
		dependencyID, parseErr := model.ParsePackageID(raw)
		if parseErr != nil {
			return LockedPackage{}, parseErr
		}
		if _, exists := seen[dependencyID]; exists {
			return LockedPackage{}, fmt.Errorf("package %q repeats dependency %q", id, dependencyID)
		}
		seen[dependencyID] = struct{}{}
		dependencyIDs = append(dependencyIDs, dependencyID)
	}
	sort.Slice(dependencyIDs, func(i, j int) bool { return dependencyIDs[i] < dependencyIDs[j] })
	return LockedPackage{
		PackageID: id, Version: exactVersion, ContentHash: hash,
		Features: normalizedFeatures, Dependencies: dependencyIDs,
	}, nil
}

func normalizeFeatures(values []string) ([]string, error) {
	result := make([]string, 0, len(values))
	seen := make(map[string]struct{}, len(values))
	for _, value := range values {
		if !featurePattern.MatchString(value) {
			return nil, fmt.Errorf("feature %q is not canonical", value)
		}
		if _, exists := seen[value]; exists {
			return nil, fmt.Errorf("feature %q is repeated", value)
		}
		seen[value] = struct{}{}
		result = append(result, value)
	}
	sort.Strings(result)
	return result, nil
}

func cloneLockedPackage(value LockedPackage) LockedPackage {
	value.Features = append(make([]string, 0, len(value.Features)), value.Features...)
	value.Dependencies = append(make([]model.PackageID, 0, len(value.Dependencies)), value.Dependencies...)
	return value
}

// ExactLock is a validated, single-version, cycle-free transitive graph.
type ExactLock struct {
	root     model.PackageID
	packages []LockedPackage
}

// BuildExactLock validates the graph and returns a canonical lock. Every node
// must be reachable from root; no extra or partially locked nodes are allowed.
func BuildExactLock(root string, packages []LockedPackage) (ExactLock, error) {
	rootID, err := model.ParsePackageID(root)
	if err != nil {
		return ExactLock{}, err
	}
	byID := make(map[model.PackageID]LockedPackage, len(packages))
	for _, input := range packages {
		node, normalizeErr := NewLockedPackage(
			input.PackageID.String(), input.Version.String(), input.ContentHash.String(), input.Features, packageIDsToStrings(input.Dependencies),
		)
		if normalizeErr != nil {
			return ExactLock{}, fmt.Errorf("invalid locked package: %w", normalizeErr)
		}
		if existing, exists := byID[node.PackageID]; exists {
			if existing.Version != node.Version || existing.ContentHash != node.ContentHash {
				return ExactLock{}, fmt.Errorf("single-version violation for %q: %s/%s and %s/%s", node.PackageID, existing.Version, existing.ContentHash, node.Version, node.ContentHash)
			}
			return ExactLock{}, fmt.Errorf("locked package %q is duplicated", node.PackageID)
		}
		byID[node.PackageID] = node
	}
	if len(byID) == 0 {
		return ExactLock{}, errors.New("exact lock requires at least the root package")
	}
	if _, exists := byID[rootID]; !exists {
		return ExactLock{}, fmt.Errorf("lock root %q is not present", rootID)
	}
	for _, node := range byID {
		for _, dependencyID := range node.Dependencies {
			if _, exists := byID[dependencyID]; !exists {
				return ExactLock{}, fmt.Errorf("package %q depends on unlocked package %q", node.PackageID, dependencyID)
			}
		}
	}
	visited, err := validateAcyclic(rootID, byID)
	if err != nil {
		return ExactLock{}, err
	}
	if len(visited) != len(byID) {
		unreachable := make([]string, 0, len(byID)-len(visited))
		for packageID := range byID {
			if !visited[packageID] {
				unreachable = append(unreachable, packageID.String())
			}
		}
		sort.Strings(unreachable)
		return ExactLock{}, fmt.Errorf("lock contains packages unreachable from %q: %s", rootID, strings.Join(unreachable, ", "))
	}
	canonical := make([]LockedPackage, 0, len(byID))
	for _, node := range byID {
		canonical = append(canonical, cloneLockedPackage(node))
	}
	sort.Slice(canonical, func(i, j int) bool { return canonical[i].PackageID < canonical[j].PackageID })
	return ExactLock{root: rootID, packages: canonical}, nil
}

func validateAcyclic(root model.PackageID, nodes map[model.PackageID]LockedPackage) (map[model.PackageID]bool, error) {
	const (
		unvisited = 0
		visiting  = 1
		visited   = 2
	)
	state := make(map[model.PackageID]int, len(nodes))
	reachable := make(map[model.PackageID]bool, len(nodes))
	stack := make([]model.PackageID, 0, len(nodes))
	var visit func(model.PackageID) error
	visit = func(packageID model.PackageID) error {
		switch state[packageID] {
		case visiting:
			start := 0
			for index, item := range stack {
				if item == packageID {
					start = index
					break
				}
			}
			cycle := append(append([]model.PackageID(nil), stack[start:]...), packageID)
			return fmt.Errorf("dependency cycle: %s", strings.Join(packageIDsToStrings(cycle), " -> "))
		case visited:
			return nil
		}
		state[packageID] = visiting
		reachable[packageID] = true
		stack = append(stack, packageID)
		for _, dependencyID := range nodes[packageID].Dependencies {
			if err := visit(dependencyID); err != nil {
				return err
			}
		}
		stack = stack[:len(stack)-1]
		state[packageID] = visited
		return nil
	}
	if err := visit(root); err != nil {
		return nil, err
	}
	return reachable, nil
}

func packageIDsToStrings(values []model.PackageID) []string {
	result := make([]string, len(values))
	for index, value := range values {
		result[index] = value.String()
	}
	return result
}

// Root returns the package graph root.
func (lock ExactLock) Root() model.PackageID { return lock.root }

// Packages returns a deep copy in canonical package_id order.
func (lock ExactLock) Packages() []LockedPackage {
	result := make([]LockedPackage, len(lock.packages))
	for index, node := range lock.packages {
		result[index] = cloneLockedPackage(node)
	}
	return result
}

// Package returns a copy of the exact node for packageID.
func (lock ExactLock) Package(packageID model.PackageID) (LockedPackage, bool) {
	index := sort.Search(len(lock.packages), func(index int) bool { return lock.packages[index].PackageID >= packageID })
	if index >= len(lock.packages) || lock.packages[index].PackageID != packageID {
		return LockedPackage{}, false
	}
	return cloneLockedPackage(lock.packages[index]), true
}

type lockDocument struct {
	SchemaVersion int                 `json:"schema_version"`
	Root          string              `json:"root"`
	Packages      []lockedPackageJSON `json:"packages"`
}

type lockedPackageJSON struct {
	PackageID    string   `json:"package_id"`
	Version      string   `json:"version"`
	ContentHash  string   `json:"content_hash"`
	Features     []string `json:"features"`
	Dependencies []string `json:"dependencies"`
}

func (lock ExactLock) document() lockDocument {
	document := lockDocument{SchemaVersion: LockSchemaVersion, Root: lock.root.String(), Packages: make([]lockedPackageJSON, len(lock.packages))}
	for index, node := range lock.packages {
		document.Packages[index] = lockedPackageJSON{
			PackageID: node.PackageID.String(), Version: node.Version.String(), ContentHash: node.ContentHash.String(),
			Features: append(make([]string, 0, len(node.Features)), node.Features...), Dependencies: packageIDsToStrings(node.Dependencies),
		}
	}
	return document
}

// CanonicalJSON returns stable compact JSON suitable for hashing and storage.
func (lock ExactLock) CanonicalJSON() ([]byte, error) {
	if lock.root == "" || len(lock.packages) == 0 {
		return nil, errors.New("exact lock is uninitialized")
	}
	return json.Marshal(lock.document())
}

// Digest hashes the complete exact lock, including enabled Features and every
// transitive edge.
func (lock ExactLock) Digest() (model.ContentHash, error) {
	data, err := lock.CanonicalJSON()
	if err != nil {
		return "", err
	}
	digest := sha256.Sum256(data)
	return model.ParseContentHash("sha256:" + hex.EncodeToString(digest[:]))
}

// ParseExactLock decodes strict JSON and then applies all graph invariants.
func ParseExactLock(data []byte) (ExactLock, error) {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var document lockDocument
	if err := decoder.Decode(&document); err != nil {
		return ExactLock{}, fmt.Errorf("decode exact lock: %w", err)
	}
	if err := ensureJSONEOF(decoder); err != nil {
		return ExactLock{}, err
	}
	if document.SchemaVersion != LockSchemaVersion {
		return ExactLock{}, fmt.Errorf("unsupported exact lock schema_version %d", document.SchemaVersion)
	}
	packages := make([]LockedPackage, 0, len(document.Packages))
	for _, raw := range document.Packages {
		node, err := NewLockedPackage(raw.PackageID, raw.Version, raw.ContentHash, raw.Features, raw.Dependencies)
		if err != nil {
			return ExactLock{}, fmt.Errorf("package %q: %w", raw.PackageID, err)
		}
		packages = append(packages, node)
	}
	return BuildExactLock(document.Root, packages)
}

func ensureJSONEOF(decoder *json.Decoder) error {
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		if err == nil {
			return errors.New("exact lock contains multiple JSON values")
		}
		return fmt.Errorf("decode exact lock trailer: %w", err)
	}
	return nil
}
