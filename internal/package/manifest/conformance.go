// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package manifest

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"unicode/utf8"

	"github.com/santhosh-tekuri/jsonschema/v6"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

const packageSchemaBaseID = "https://github.com/zyc14588/TRPG_PLATFORM/schemas/package/"

// SchemaDocument identifies one public package JSON Schema document.
type SchemaDocument string

const (
	ManifestSchemaDocument         SchemaDocument = "manifest-v1.schema.json"
	LockSchemaDocument             SchemaDocument = "lock-v1.schema.json"
	ArtifactIdentitySchemaDocument SchemaDocument = "artifact-identity-v1.schema.json"
)

// SchemaResources supplies the tracked public schemas without copying them
// into the Go package. Callers may read them from disk, an embed.FS, or another
// immutable source.
type SchemaResources struct {
	Manifest         []byte
	Lock             []byte
	ArtifactIdentity []byte
}

// SchemaConformance validates the public package contract in two explicit
// layers: Draft 2020-12 structure and the existing canonical Go rules that
// JSON Schema cannot reliably express.
type SchemaConformance struct {
	schemas map[SchemaDocument]*jsonschema.Schema
}

// NewSchemaConformance compiles all package schemas with the pinned Draft
// 2020-12 validator. Relative references resolve through their tracked IDs.
func NewSchemaConformance(resources SchemaResources) (*SchemaConformance, error) {
	compiler := jsonschema.NewCompiler()
	compiler.DefaultDraft(jsonschema.Draft2020)
	inputs := []struct {
		document SchemaDocument
		data     []byte
	}{
		{document: ManifestSchemaDocument, data: resources.Manifest},
		{document: LockSchemaDocument, data: resources.Lock},
		{document: ArtifactIdentitySchemaDocument, data: resources.ArtifactIdentity},
	}
	for _, input := range inputs {
		if len(input.data) == 0 {
			return nil, fmt.Errorf("package schema %s is empty", input.document)
		}
		document, err := jsonschema.UnmarshalJSON(bytes.NewReader(input.data))
		if err != nil {
			return nil, fmt.Errorf("decode package schema %s: %w", input.document, err)
		}
		if err := compiler.AddResource(packageSchemaBaseID+string(input.document), document); err != nil {
			return nil, fmt.Errorf("add package schema %s: %w", input.document, err)
		}
	}
	compiled := make(map[SchemaDocument]*jsonschema.Schema, len(inputs))
	for _, input := range inputs {
		schema, err := compiler.Compile(packageSchemaBaseID + string(input.document))
		if err != nil {
			return nil, fmt.Errorf("compile package schema %s as Draft 2020-12: %w", input.document, err)
		}
		compiled[input.document] = schema
	}
	return &SchemaConformance{schemas: compiled}, nil
}

// ValidateStructure applies only the Draft 2020-12 structural layer.
func (validator *SchemaConformance) ValidateStructure(document SchemaDocument, data []byte) error {
	schema, ok := validator.schema(document)
	if !ok {
		return fmt.Errorf("unknown package schema document %q", document)
	}
	if !utf8.Valid(data) {
		return fmt.Errorf("%s instance is not valid UTF-8", document)
	}
	instance, err := jsonschema.UnmarshalJSON(bytes.NewReader(data))
	if err != nil {
		return fmt.Errorf("decode %s instance: %w", document, err)
	}
	if err := schema.Validate(instance); err != nil {
		return fmt.Errorf("%s structural validation: %w", document, err)
	}
	return nil
}

// ValidateCanonical applies only the supplemental canonical Go layer. Public
// conformance callers normally use Validate, which always runs structure first.
func (validator *SchemaConformance) ValidateCanonical(document SchemaDocument, data []byte) error {
	if _, ok := validator.schema(document); !ok {
		return fmt.Errorf("unknown package schema document %q", document)
	}
	if !utf8.Valid(data) {
		return fmt.Errorf("%s instance is not valid UTF-8", document)
	}
	switch document {
	case ManifestSchemaDocument:
		if len(data) > MaxManifestBytes {
			return fmt.Errorf("manifest exceeds %d bytes", MaxManifestBytes)
		}
		return validateCanonicalManifestJSON(data)
	case LockSchemaDocument:
		if _, err := dependency.ParseExactLock(data); err != nil {
			return fmt.Errorf("canonical exact lock: %w", err)
		}
		return nil
	case ArtifactIdentitySchemaDocument:
		return validateCanonicalArtifactIdentityJSON(data)
	default:
		return fmt.Errorf("unknown package schema document %q", document)
	}
}

// Validate applies the complete public schema contract.
func (validator *SchemaConformance) Validate(document SchemaDocument, data []byte) error {
	if err := validator.ValidateStructure(document, data); err != nil {
		return err
	}
	if err := validator.ValidateCanonical(document, data); err != nil {
		return fmt.Errorf("%s canonical validation: %w", document, err)
	}
	return nil
}

func (validator *SchemaConformance) schema(document SchemaDocument) (*jsonschema.Schema, bool) {
	if validator == nil {
		return nil, false
	}
	schema, ok := validator.schemas[document]
	return schema, ok
}

type conformanceManifestHeader struct {
	ArtifactType string `json:"artifact_type"`
}

type conformancePackageManifest struct {
	SchemaVersion int                     `json:"schema_version"`
	ArtifactType  string                  `json:"artifact_type"`
	PackageID     string                  `json:"package_id"`
	PackageKind   string                  `json:"package_kind"`
	Version       string                  `json:"version"`
	DisplayName   string                  `json:"display_name"`
	Entrypoint    optionalText            `json:"entrypoint"`
	LuaProfile    optionalText            `json:"lua_profile"`
	HostAPI       *HostAPIRange           `json:"host_api"`
	Build         model.BuildProvenance   `json:"build"`
	Rights        rawRights               `json:"rights"`
	Capabilities  conformanceCapabilities `json:"capabilities"`
	Dependencies  []conformanceDependency `json:"dependencies"`
}

type conformanceCapabilities struct {
	Required []string                  `json:"required"`
	Optional []capability.OptionalSpec `json:"optional"`
}

type conformanceDependency struct {
	PackageID string   `json:"package_id"`
	Version   string   `json:"version"`
	Optional  bool     `json:"optional"`
	Features  []string `json:"features"`
}

type conformanceBundleManifest struct {
	SchemaVersion int                   `json:"schema_version"`
	ArtifactType  string                `json:"artifact_type"`
	BundleID      string                `json:"bundle_id"`
	Version       string                `json:"version"`
	DisplayName   string                `json:"display_name"`
	Build         model.BuildProvenance `json:"build"`
	Rights        rawRights             `json:"rights"`
	Artifacts     []BundleArtifact      `json:"artifacts"`
}

func validateCanonicalManifestJSON(data []byte) error {
	var header conformanceManifestHeader
	if err := json.Unmarshal(data, &header); err != nil {
		return fmt.Errorf("decode manifest header: %w", err)
	}
	artifactType, err := model.ParseArtifactType(header.ArtifactType)
	if err != nil {
		return err
	}
	switch artifactType {
	case model.ArtifactTypePackage:
		var raw conformancePackageManifest
		if err := decodeConformanceJSON(data, &raw); err != nil {
			return fmt.Errorf("decode package manifest: %w", err)
		}
		entrypoint, err := raw.Entrypoint.canonical("entrypoint")
		if err != nil {
			return err
		}
		luaProfile, err := raw.LuaProfile.canonical("lua_profile")
		if err != nil {
			return err
		}
		rights, err := rightsFromRaw(raw.Rights)
		if err != nil {
			return err
		}
		declaration, err := capability.NewDeclaration(raw.Capabilities.Required, raw.Capabilities.Optional)
		if err != nil {
			return err
		}
		requirements := make([]dependency.Requirement, 0, len(raw.Dependencies))
		for _, item := range raw.Dependencies {
			requirement, err := dependency.NewRequirement(item.PackageID, item.Version, item.Optional, item.Features)
			if err != nil {
				return fmt.Errorf("dependency %q: %w", item.PackageID, err)
			}
			requirements = append(requirements, requirement)
		}
		_, err = NormalizePackage(Package{
			SchemaVersion: raw.SchemaVersion,
			PackageID:     model.PackageID(raw.PackageID), PackageKind: model.PackageKind(raw.PackageKind), Version: model.Version(raw.Version),
			DisplayName: raw.DisplayName, Entrypoint: entrypoint, LuaProfile: luaProfile, HostAPI: raw.HostAPI,
			Build: raw.Build, Rights: rights, Capabilities: declaration, Dependencies: requirements,
		})
		return err
	case model.ArtifactTypeBundle:
		var raw conformanceBundleManifest
		if err := decodeConformanceJSON(data, &raw); err != nil {
			return fmt.Errorf("decode bundle manifest: %w", err)
		}
		rights, err := rightsFromRaw(raw.Rights)
		if err != nil {
			return err
		}
		_, err = NormalizeBundle(Bundle{
			SchemaVersion: raw.SchemaVersion, BundleID: model.PackageID(raw.BundleID), Version: model.Version(raw.Version),
			DisplayName: raw.DisplayName, Build: raw.Build, Rights: rights, Artifacts: raw.Artifacts,
		})
		return err
	default:
		return fmt.Errorf("unsupported artifact type %q", artifactType)
	}
}

type conformanceArtifactIdentity struct {
	SchemaVersion         int                   `json:"schema_version"`
	ManifestSchemaVersion int                   `json:"manifest_schema_version"`
	ArtifactType          string                `json:"artifact_type"`
	PackageID             string                `json:"package_id"`
	PackageKind           string                `json:"package_kind"`
	Version               string                `json:"version"`
	ContentHash           string                `json:"content_hash"`
	Build                 model.BuildProvenance `json:"build_provenance"`
	Rights                rawRights             `json:"rights"`
	DependencyLock        json.RawMessage       `json:"dependency_lock"`
}

func validateCanonicalArtifactIdentityJSON(data []byte) error {
	var raw conformanceArtifactIdentity
	if err := decodeConformanceJSON(data, &raw); err != nil {
		return fmt.Errorf("decode artifact identity: %w", err)
	}
	if raw.SchemaVersion != ArtifactIdentitySchemaVersion {
		return fmt.Errorf("unsupported artifact identity schema_version %d", raw.SchemaVersion)
	}
	if raw.ManifestSchemaVersion != SchemaVersion {
		return fmt.Errorf("unsupported manifest schema_version %d", raw.ManifestSchemaVersion)
	}
	artifactType, err := model.ParseArtifactType(raw.ArtifactType)
	if err != nil {
		return err
	}
	if artifactType != model.ArtifactTypePackage {
		return fmt.Errorf("artifact identity requires artifact_type %q", model.ArtifactTypePackage)
	}
	packageID, err := model.ParsePackageID(raw.PackageID)
	if err != nil {
		return err
	}
	if _, err := model.ParsePackageKind(raw.PackageKind); err != nil {
		return err
	}
	version, err := model.ParseVersion(raw.Version)
	if err != nil {
		return err
	}
	contentHash, err := model.ParseContentHash(raw.ContentHash)
	if err != nil {
		return err
	}
	if _, err := model.NormalizeBuildProvenance(raw.Build); err != nil {
		return err
	}
	if _, err := rightsFromRaw(raw.Rights); err != nil {
		return err
	}
	lock, err := dependency.ParseExactLock(raw.DependencyLock)
	if err != nil {
		return err
	}
	if lock.Root() != packageID {
		return fmt.Errorf("artifact package_id %q does not match exact lock root %q", packageID, lock.Root())
	}
	root, exists := lock.Package(packageID)
	if !exists {
		return fmt.Errorf("exact lock does not contain artifact package %q", packageID)
	}
	if root.Version != version {
		return fmt.Errorf("artifact version %q does not match exact lock root version %q", version, root.Version)
	}
	if root.ContentHash != contentHash {
		return fmt.Errorf("artifact content hash %q does not match exact lock root content hash %q", contentHash, root.ContentHash)
	}
	return nil
}

func decodeConformanceJSON(data []byte, target any) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(target); err != nil {
		return err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return fmt.Errorf("multiple JSON values")
		}
		return fmt.Errorf("decode JSON trailer: %w", err)
	}
	return nil
}
