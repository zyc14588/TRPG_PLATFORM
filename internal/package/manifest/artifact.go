// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package manifest

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

const (
	ArtifactIdentitySchemaVersion   = 1
	ArtifactIdentityV2SchemaVersion = 2
)

// ArtifactIdentity is an opaque canonical identity document and its digest.
// Callers receive copies so the identity cannot be changed after construction.
type ArtifactIdentity struct {
	canonical []byte
	digest    model.ContentHash
}

type artifactIdentityDocument struct {
	SchemaVersion         int                   `json:"schema_version"`
	ManifestSchemaVersion int                   `json:"manifest_schema_version"`
	ArtifactType          model.ArtifactType    `json:"artifact_type"`
	PackageID             model.PackageID       `json:"package_id"`
	PackageKind           model.PackageKind     `json:"package_kind"`
	Version               model.Version         `json:"version"`
	ContentHash           model.ContentHash     `json:"content_hash"`
	Build                 model.BuildProvenance `json:"build_provenance"`
	Rights                model.Rights          `json:"rights"`
	DependencyLock        json.RawMessage       `json:"dependency_lock"`
}

// BuildArtifactIdentity validates the complete package/lock relationship and
// hashes a canonical document containing provenance, rights, and the complete
// exact transitive lock.
func BuildArtifactIdentity(pkg Package, contentHash model.ContentHash, lock dependency.ExactLock) (ArtifactIdentity, error) {
	normalized, err := NormalizePackage(pkg)
	if err != nil {
		return ArtifactIdentity{}, err
	}
	hash, err := model.ParseContentHash(contentHash.String())
	if err != nil {
		return ArtifactIdentity{}, err
	}
	root, exists := lock.Package(normalized.PackageID)
	if !exists {
		return ArtifactIdentity{}, fmt.Errorf("exact lock does not contain manifest package %q", normalized.PackageID)
	}
	if lock.Root() != normalized.PackageID {
		return ArtifactIdentity{}, fmt.Errorf("exact lock root %q does not match package_id %q", lock.Root(), normalized.PackageID)
	}
	if root.Version != normalized.Version {
		return ArtifactIdentity{}, fmt.Errorf("exact lock root version %q does not match manifest version %q", root.Version, normalized.Version)
	}
	if root.ContentHash != hash {
		return ArtifactIdentity{}, fmt.Errorf("exact lock root content hash %q does not match artifact content hash %q", root.ContentHash, hash)
	}
	if err := dependency.ValidateRequirements(lock, normalized.PackageID, normalized.Dependencies); err != nil {
		return ArtifactIdentity{}, err
	}
	lockJSON, err := lock.CanonicalJSON()
	if err != nil {
		return ArtifactIdentity{}, err
	}
	identityVersion := ArtifactIdentitySchemaVersion
	if normalized.SchemaVersion == ExtensionSchemaVersion {
		identityVersion = ArtifactIdentityV2SchemaVersion
	}
	document := artifactIdentityDocument{
		SchemaVersion: identityVersion, ManifestSchemaVersion: normalized.SchemaVersion,
		ArtifactType: model.ArtifactTypePackage, PackageID: normalized.PackageID, PackageKind: normalized.PackageKind,
		Version: normalized.Version, ContentHash: hash, Build: normalized.Build, Rights: normalized.Rights,
		DependencyLock: json.RawMessage(lockJSON),
	}
	canonical, err := json.Marshal(document)
	if err != nil {
		return ArtifactIdentity{}, fmt.Errorf("encode artifact identity: %w", err)
	}
	digestBytes := sha256.Sum256(canonical)
	digest, err := model.ParseContentHash("sha256:" + hex.EncodeToString(digestBytes[:]))
	if err != nil {
		return ArtifactIdentity{}, err
	}
	return ArtifactIdentity{canonical: canonical, digest: digest}, nil
}

func (identity ArtifactIdentity) CanonicalJSON() []byte {
	return append([]byte(nil), identity.canonical...)
}

func (identity ArtifactIdentity) Digest() model.ContentHash { return identity.digest }
