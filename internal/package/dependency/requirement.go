// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package dependency

import (
	"fmt"
	"sort"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

// Requirement is an exact source-manifest dependency declaration. Version
// ranges may be introduced by a later versioned contract; version one accepts
// exact semantic versions and locks them without runtime re-resolution.
type Requirement struct {
	PackageID model.PackageID
	Version   model.Version
	Optional  bool
	Features  []string
}

func NewRequirement(packageID, version string, optional bool, features []string) (Requirement, error) {
	id, err := model.ParsePackageID(packageID)
	if err != nil {
		return Requirement{}, err
	}
	exactVersion, err := model.ParseVersion(version)
	if err != nil {
		return Requirement{}, err
	}
	normalizedFeatures, err := normalizeFeatures(features)
	if err != nil {
		return Requirement{}, err
	}
	return Requirement{PackageID: id, Version: exactVersion, Optional: optional, Features: normalizedFeatures}, nil
}

// NormalizeRequirements validates, sorts, and rejects repeated declarations.
func NormalizeRequirements(values []Requirement) ([]Requirement, error) {
	result := make([]Requirement, 0, len(values))
	seen := make(map[model.PackageID]struct{}, len(values))
	for _, value := range values {
		normalized, err := NewRequirement(value.PackageID.String(), value.Version.String(), value.Optional, value.Features)
		if err != nil {
			return nil, err
		}
		if _, exists := seen[normalized.PackageID]; exists {
			return nil, fmt.Errorf("dependency %q is declared more than once", normalized.PackageID)
		}
		seen[normalized.PackageID] = struct{}{}
		result = append(result, normalized)
	}
	sort.Slice(result, func(i, j int) bool { return result[i].PackageID < result[j].PackageID })
	return result, nil
}

// ValidateRequirements proves that the root's exact edges match the manifest:
// all required dependencies are present, optional edges are explicit, no edge
// is undeclared, and exact versions/Features match.
func ValidateRequirements(lock ExactLock, root model.PackageID, requirements []Requirement) error {
	if lock.Root() != root {
		return fmt.Errorf("lock root %q does not match manifest package_id %q", lock.Root(), root)
	}
	normalized, err := NormalizeRequirements(requirements)
	if err != nil {
		return err
	}
	rootNode, exists := lock.Package(root)
	if !exists {
		return fmt.Errorf("lock has no root node %q", root)
	}
	declared := make(map[model.PackageID]Requirement, len(normalized))
	for _, requirement := range normalized {
		declared[requirement.PackageID] = requirement
	}
	lockedEdges := make(map[model.PackageID]struct{}, len(rootNode.Dependencies))
	for _, dependencyID := range rootNode.Dependencies {
		lockedEdges[dependencyID] = struct{}{}
		requirement, declaredEdge := declared[dependencyID]
		if !declaredEdge {
			return fmt.Errorf("lock contains undeclared direct dependency %q", dependencyID)
		}
		node, locked := lock.Package(dependencyID)
		if !locked {
			return fmt.Errorf("lock has no exact node for dependency %q", dependencyID)
		}
		if node.Version != requirement.Version {
			return fmt.Errorf("dependency %q locks version %q, manifest requires %q", dependencyID, node.Version, requirement.Version)
		}
		if err := requireFeatures(dependencyID, node.Features, requirement.Features); err != nil {
			return err
		}
	}
	for _, requirement := range normalized {
		if _, locked := lockedEdges[requirement.PackageID]; !locked && !requirement.Optional {
			return fmt.Errorf("required dependency %q is absent from the exact lock", requirement.PackageID)
		}
	}
	return nil
}

func requireFeatures(packageID model.PackageID, locked, required []string) error {
	available := make(map[string]struct{}, len(locked))
	for _, feature := range locked {
		available[feature] = struct{}{}
	}
	for _, feature := range required {
		if _, exists := available[feature]; !exists {
			return fmt.Errorf("dependency %q does not enable required feature %q", packageID, feature)
		}
	}
	return nil
}
