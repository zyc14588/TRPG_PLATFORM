// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package model defines the value objects shared by the executable package
// contract. It deliberately contains no installation or runtime behavior.
package model

import "fmt"

// PackageKind is a runtime package kind. Bundle is intentionally not a
// PackageKind: it is a distribution container and is never loaded by a
// Session.
type PackageKind string

const (
	PackageKindGameSystem  PackageKind = "game-system"
	PackageKindContent     PackageKind = "content"
	PackageKindAssets      PackageKind = "assets"
	PackageKindUIExtension PackageKind = "ui-extension"
	PackageKindLibrary     PackageKind = "library"
)

var packageKinds = []PackageKind{
	PackageKindGameSystem,
	PackageKindContent,
	PackageKindAssets,
	PackageKindUIExtension,
	PackageKindLibrary,
}

// ParsePackageKind rejects distribution-container kinds and unknown values.
func ParsePackageKind(value string) (PackageKind, error) {
	kind := PackageKind(value)
	for _, candidate := range packageKinds {
		if kind == candidate {
			return kind, nil
		}
	}
	return "", fmt.Errorf("package kind %q is not a runtime package kind", value)
}

// PackageKinds returns the canonical runtime package kinds in stable order.
func PackageKinds() []PackageKind {
	return append([]PackageKind(nil), packageKinds...)
}

// CanStartSession reports whether this kind can be the root game system for a
// Session. In particular, library packages can never start Sessions.
func (kind PackageKind) CanStartSession() bool {
	return kind == PackageKindGameSystem
}

// ArtifactType distinguishes executable packages from distribution bundles.
type ArtifactType string

const (
	ArtifactTypePackage ArtifactType = "package"
	ArtifactTypeBundle  ArtifactType = "bundle"
)

// ParseArtifactType returns one of the two version-one artifact forms.
func ParseArtifactType(value string) (ArtifactType, error) {
	switch ArtifactType(value) {
	case ArtifactTypePackage:
		return ArtifactTypePackage, nil
	case ArtifactTypeBundle:
		return ArtifactTypeBundle, nil
	default:
		return "", fmt.Errorf("artifact type %q is not supported", value)
	}
}
