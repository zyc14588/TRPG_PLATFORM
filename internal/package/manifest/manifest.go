// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package manifest parses and validates the strict versioned TOML package
// artifact contract. It performs no installation or runtime execution.
package manifest

import (
	"encoding/json"
	"fmt"
	"regexp"
	"sort"
	"strings"
	"unicode"
	"unicode/utf8"

	"github.com/BurntSushi/toml"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

const (
	SchemaVersion          = 1
	ExtensionSchemaVersion = 2
	MaxManifestBytes       = 1 << 20
)

var luaProfilePattern = regexp.MustCompile(`^[a-z0-9][a-z0-9.-]{0,127}$`)

// HostAPIRange is a declarative major/minor compatibility range. Negotiation
// and callback execution belong to the later Host API batch.
type HostAPIRange struct {
	Major    uint32 `json:"major"`
	MinMinor uint32 `json:"min_minor"`
	MaxMinor uint32 `json:"max_minor"`
}

// Package is a normalized executable package manifest.
type Package struct {
	SchemaVersion int                      `json:"schema_version"`
	PackageID     model.PackageID          `json:"package_id"`
	PackageKind   model.PackageKind        `json:"package_kind"`
	Version       model.Version            `json:"version"`
	DisplayName   string                   `json:"display_name"`
	Entrypoint    string                   `json:"entrypoint,omitempty"`
	LuaProfile    string                   `json:"lua_profile,omitempty"`
	HostAPI       *HostAPIRange            `json:"host_api,omitempty"`
	Build         model.BuildProvenance    `json:"build"`
	Rights        model.Rights             `json:"rights"`
	Capabilities  capability.Declaration   `json:"capabilities"`
	Dependencies  []dependency.Requirement `json:"dependencies"`
	Extensions    []extension.Descriptor   `json:"extensions,omitempty"`
}

// BundleArtifact is an exact package reference carried by a distribution
// bundle. A Bundle is never a runtime package or a Session root.
type BundleArtifact struct {
	PackageID   model.PackageID   `json:"package_id"`
	Version     model.Version     `json:"version"`
	ContentHash model.ContentHash `json:"content_hash"`
}

// Bundle is the normalized distribution-container form of the manifest.
type Bundle struct {
	SchemaVersion int                   `json:"schema_version"`
	BundleID      model.PackageID       `json:"bundle_id"`
	Version       model.Version         `json:"version"`
	DisplayName   string                `json:"display_name"`
	Build         model.BuildProvenance `json:"build"`
	Rights        model.Rights          `json:"rights"`
	Artifacts     []BundleArtifact      `json:"artifacts"`
}

// Document contains exactly one variant.
type Document struct {
	ArtifactType model.ArtifactType
	Package      *Package
	Bundle       *Bundle
}

// CanStartSession is true only for game-system package documents.
func (document Document) CanStartSession() bool {
	return document.Package != nil && document.Package.PackageKind.CanStartSession()
}

type rawHeader struct {
	SchemaVersion int    `toml:"schema_version"`
	ArtifactType  string `toml:"artifact_type"`
}

type rawPackage struct {
	SchemaVersion int              `toml:"schema_version"`
	ArtifactType  string           `toml:"artifact_type"`
	PackageID     string           `toml:"package_id"`
	PackageKind   string           `toml:"package_kind"`
	Version       string           `toml:"version"`
	DisplayName   string           `toml:"display_name"`
	Entrypoint    optionalText     `toml:"entrypoint"`
	LuaProfile    optionalText     `toml:"lua_profile"`
	HostAPI       *rawHostAPIRange `toml:"host_api"`
	Build         rawBuild         `toml:"build"`
	Rights        rawRights        `toml:"rights"`
	Capabilities  *rawCapabilities `toml:"capabilities"`
	Dependencies  []rawDependency  `toml:"dependencies"`
	Extensions    []rawExtension   `toml:"extensions"`
}

type rawExtension struct {
	Namespace       string `toml:"namespace"`
	Required        *bool  `toml:"required"`
	ContractVersion *int64 `toml:"contract_version"`
	SchemaPath      string `toml:"schema_path"`
	SchemaSHA256    string `toml:"schema_sha256"`
	PayloadPath     string `toml:"payload_path"`
	HostAPIMajor    *int64 `toml:"host_api_major"`
	HostAPIMinMinor *int64 `toml:"host_api_min_minor"`
	HostAPIMaxMinor *int64 `toml:"host_api_max_minor"`
}

type rawBundle struct {
	SchemaVersion int                 `toml:"schema_version"`
	ArtifactType  string              `toml:"artifact_type"`
	BundleID      string              `toml:"bundle_id"`
	Version       string              `toml:"version"`
	DisplayName   string              `toml:"display_name"`
	Build         rawBuild            `toml:"build"`
	Rights        rawRights           `toml:"rights"`
	Artifacts     []rawBundleArtifact `toml:"artifacts"`
}

type rawHostAPIRange struct {
	Major    *int64 `toml:"major"`
	MinMinor *int64 `toml:"min_minor"`
	MaxMinor *int64 `toml:"max_minor"`
}

type rawBuild struct {
	Source   string `toml:"source"`
	Revision string `toml:"revision"`
	Builder  string `toml:"builder"`
}

type rawRights struct {
	Authors           []string     `toml:"authors" json:"authors"`
	Source            string       `toml:"source" json:"source"`
	LicenseExpression optionalText `toml:"license_expression" json:"license_expression"`
	Statement         optionalText `toml:"statement" json:"statement"`
}

// optionalText preserves whether an optional textual field was absent. The
// zero value means absent; explicitly provided empty or whitespace-only input
// is rejected before canonical normalization can collapse it to that value.
type optionalText struct {
	present bool
	value   string
}

func (value *optionalText) UnmarshalText(data []byte) error {
	value.present = true
	value.value = string(data)
	return nil
}

func (value *optionalText) UnmarshalJSON(data []byte) error {
	var decoded *string
	if err := json.Unmarshal(data, &decoded); err != nil {
		return err
	}
	if decoded == nil {
		return fmt.Errorf("optional text cannot be null")
	}
	value.present = true
	value.value = *decoded
	return nil
}

func (value optionalText) canonical(field string) (string, error) {
	if !value.present {
		return "", nil
	}
	if strings.TrimSpace(value.value) == "" {
		return "", fmt.Errorf("%s must be nonblank when provided", field)
	}
	return value.value, nil
}

type rawCapabilities struct {
	Required []string      `toml:"required"`
	Optional []rawOptional `toml:"optional"`
}

type rawOptional struct {
	Name     string `toml:"name"`
	Fallback string `toml:"fallback"`
}

type rawDependency struct {
	PackageID string   `toml:"package_id"`
	Version   string   `toml:"version"`
	Optional  bool     `toml:"optional"`
	Features  []string `toml:"features"`
}

type rawBundleArtifact struct {
	PackageID   string `toml:"package_id"`
	Version     string `toml:"version"`
	ContentHash string `toml:"content_hash"`
}

// Parse strictly decodes a package or bundle TOML manifest. Unknown fields,
// unsupported schema versions, and mixed variants fail closed.
func Parse(data []byte) (Document, error) {
	if len(data) == 0 {
		return Document{}, fmt.Errorf("manifest is empty")
	}
	if len(data) > MaxManifestBytes {
		return Document{}, fmt.Errorf("manifest exceeds %d bytes", MaxManifestBytes)
	}
	if !utf8.Valid(data) {
		return Document{}, fmt.Errorf("manifest is not valid UTF-8")
	}
	var header rawHeader
	if _, err := toml.Decode(string(data), &header); err != nil {
		return Document{}, fmt.Errorf("decode manifest header: %w", err)
	}
	if header.SchemaVersion != SchemaVersion && header.SchemaVersion != ExtensionSchemaVersion {
		return Document{}, fmt.Errorf("unsupported manifest schema_version %d", header.SchemaVersion)
	}
	artifactType, err := model.ParseArtifactType(header.ArtifactType)
	if err != nil {
		return Document{}, err
	}
	switch artifactType {
	case model.ArtifactTypePackage:
		var raw rawPackage
		if err := decodeStrict(data, &raw); err != nil {
			return Document{}, err
		}
		value, err := packageFromRaw(raw)
		if err != nil {
			return Document{}, err
		}
		return Document{ArtifactType: artifactType, Package: &value}, nil
	case model.ArtifactTypeBundle:
		if header.SchemaVersion != SchemaVersion {
			return Document{}, fmt.Errorf("Bundle only supports manifest schema_version %d", SchemaVersion)
		}
		var raw rawBundle
		if err := decodeStrict(data, &raw); err != nil {
			return Document{}, err
		}
		value, err := bundleFromRaw(raw)
		if err != nil {
			return Document{}, err
		}
		return Document{ArtifactType: artifactType, Bundle: &value}, nil
	default:
		return Document{}, fmt.Errorf("unsupported artifact type %q", artifactType)
	}
}

func decodeStrict(data []byte, target any) error {
	metadata, err := toml.Decode(string(data), target)
	if err != nil {
		return fmt.Errorf("decode manifest: %w", err)
	}
	if undecoded := metadata.Undecoded(); len(undecoded) > 0 {
		keys := make([]string, len(undecoded))
		for index, key := range undecoded {
			keys[index] = key.String()
		}
		sort.Strings(keys)
		return fmt.Errorf("manifest contains unknown fields: %s", strings.Join(keys, ", "))
	}
	return nil
}

func packageFromRaw(raw rawPackage) (Package, error) {
	id, err := model.ParsePackageID(raw.PackageID)
	if err != nil {
		return Package{}, err
	}
	kind, err := model.ParsePackageKind(raw.PackageKind)
	if err != nil {
		return Package{}, err
	}
	version, err := model.ParseVersion(raw.Version)
	if err != nil {
		return Package{}, err
	}
	build, err := model.NormalizeBuildProvenance(model.BuildProvenance{Source: raw.Build.Source, Revision: raw.Build.Revision, Builder: raw.Build.Builder})
	if err != nil {
		return Package{}, err
	}
	rights, err := rightsFromRaw(raw.Rights)
	if err != nil {
		return Package{}, err
	}
	entrypoint, err := raw.Entrypoint.canonical("entrypoint")
	if err != nil {
		return Package{}, err
	}
	luaProfile, err := raw.LuaProfile.canonical("lua_profile")
	if err != nil {
		return Package{}, err
	}
	if raw.Capabilities == nil {
		return Package{}, fmt.Errorf("capabilities declaration is required")
	}
	optional := make([]capability.OptionalSpec, len(raw.Capabilities.Optional))
	for index, item := range raw.Capabilities.Optional {
		optional[index] = capability.OptionalSpec{Name: item.Name, Fallback: item.Fallback}
	}
	capabilities, err := capability.NewDeclaration(raw.Capabilities.Required, optional)
	if err != nil {
		return Package{}, err
	}
	requirements := make([]dependency.Requirement, 0, len(raw.Dependencies))
	for _, item := range raw.Dependencies {
		requirement, requirementErr := dependency.NewRequirement(item.PackageID, item.Version, item.Optional, item.Features)
		if requirementErr != nil {
			return Package{}, fmt.Errorf("dependency %q: %w", item.PackageID, requirementErr)
		}
		requirements = append(requirements, requirement)
	}
	requirements, err = dependency.NormalizeRequirements(requirements)
	if err != nil {
		return Package{}, err
	}
	extensions := make([]extension.Descriptor, 0, len(raw.Extensions))
	for _, item := range raw.Extensions {
		if item.Required == nil || item.ContractVersion == nil || item.HostAPIMajor == nil || item.HostAPIMinMinor == nil || item.HostAPIMaxMinor == nil {
			return Package{}, fmt.Errorf("extension %q requires required, contract_version, and complete host API range", item.Namespace)
		}
		if *item.ContractVersion <= 0 || *item.ContractVersion > int64(^uint32(0)) || *item.HostAPIMajor <= 0 || *item.HostAPIMajor > int64(^uint32(0)) || *item.HostAPIMinMinor < 0 || *item.HostAPIMinMinor > int64(^uint32(0)) || *item.HostAPIMaxMinor < 0 || *item.HostAPIMaxMinor > int64(^uint32(0)) {
			return Package{}, fmt.Errorf("extension %q numeric field is outside its supported range", item.Namespace)
		}
		extensions = append(extensions, extension.Descriptor{
			Namespace: item.Namespace, Required: *item.Required, ContractVersion: uint32(*item.ContractVersion),
			SchemaPath: item.SchemaPath, SchemaSHA256: item.SchemaSHA256, PayloadPath: item.PayloadPath,
			HostAPIMajor: uint32(*item.HostAPIMajor), HostAPIMinMinor: uint32(*item.HostAPIMinMinor), HostAPIMaxMinor: uint32(*item.HostAPIMaxMinor),
		})
	}
	if raw.SchemaVersion == SchemaVersion && len(extensions) != 0 {
		return Package{}, fmt.Errorf("manifest schema_version 1 does not support extensions")
	}
	extensions, err = extension.NormalizeDescriptors(extensions)
	if err != nil {
		return Package{}, err
	}
	value := Package{
		SchemaVersion: raw.SchemaVersion, PackageID: id, PackageKind: kind, Version: version,
		DisplayName: strings.TrimSpace(raw.DisplayName), Entrypoint: entrypoint,
		LuaProfile: luaProfile, Build: build, Rights: rights,
		Capabilities: capabilities, Dependencies: requirements, Extensions: extensions,
	}
	if raw.HostAPI != nil {
		hostAPI, hostErr := normalizeHostAPI(*raw.HostAPI)
		if hostErr != nil {
			return Package{}, hostErr
		}
		value.HostAPI = &hostAPI
	}
	return NormalizePackage(value)
}

func rightsFromRaw(raw rawRights) (model.Rights, error) {
	licenseExpression, err := raw.LicenseExpression.canonical("rights.license_expression")
	if err != nil {
		return model.Rights{}, err
	}
	statement, err := raw.Statement.canonical("rights.statement")
	if err != nil {
		return model.Rights{}, err
	}
	return model.NormalizeRights(model.Rights{
		Authors: raw.Authors, Source: raw.Source,
		LicenseExpression: licenseExpression, Statement: statement,
	})
}

func normalizeHostAPI(raw rawHostAPIRange) (HostAPIRange, error) {
	if raw.Major == nil || raw.MinMinor == nil || raw.MaxMinor == nil {
		return HostAPIRange{}, fmt.Errorf("host_api requires major, min_minor, and max_minor")
	}
	if *raw.Major <= 0 || *raw.Major > int64(^uint32(0)) || *raw.MinMinor < 0 || *raw.MinMinor > int64(^uint32(0)) || *raw.MaxMinor < 0 || *raw.MaxMinor > int64(^uint32(0)) {
		return HostAPIRange{}, fmt.Errorf("host_api values are outside the supported unsigned 32-bit range or major is zero")
	}
	if *raw.MinMinor > *raw.MaxMinor {
		return HostAPIRange{}, fmt.Errorf("host_api min_minor cannot exceed max_minor")
	}
	return HostAPIRange{Major: uint32(*raw.Major), MinMinor: uint32(*raw.MinMinor), MaxMinor: uint32(*raw.MaxMinor)}, nil
}

// NormalizePackage validates directly constructed package values and returns
// the canonical representation used in artifact identities.
func NormalizePackage(value Package) (Package, error) {
	if value.SchemaVersion != SchemaVersion && value.SchemaVersion != ExtensionSchemaVersion {
		return Package{}, fmt.Errorf("unsupported manifest schema_version %d", value.SchemaVersion)
	}
	id, err := model.ParsePackageID(value.PackageID.String())
	if err != nil {
		return Package{}, err
	}
	kind, err := model.ParsePackageKind(string(value.PackageKind))
	if err != nil {
		return Package{}, err
	}
	version, err := model.ParseVersion(value.Version.String())
	if err != nil {
		return Package{}, err
	}
	displayName := strings.TrimSpace(value.DisplayName)
	if displayName == "" || len(displayName) > 200 || !utf8.ValidString(displayName) {
		return Package{}, fmt.Errorf("display_name must be non-empty UTF-8 of at most 200 bytes")
	}
	build, err := model.NormalizeBuildProvenance(value.Build)
	if err != nil {
		return Package{}, err
	}
	rights, err := model.NormalizeRights(value.Rights)
	if err != nil {
		return Package{}, err
	}
	if err := value.Capabilities.Validate(); err != nil {
		return Package{}, err
	}
	requirements, err := dependency.NormalizeRequirements(value.Dependencies)
	if err != nil {
		return Package{}, err
	}
	if value.SchemaVersion == SchemaVersion && len(value.Extensions) != 0 {
		return Package{}, fmt.Errorf("manifest schema_version 1 does not support extensions")
	}
	extensions, err := extension.NormalizeDescriptors(value.Extensions)
	if err != nil {
		return Package{}, err
	}
	value.PackageID = id
	value.PackageKind = kind
	value.Version = version
	value.DisplayName = displayName
	value.Build = build
	value.Rights = rights
	value.Dependencies = requirements
	value.Extensions = extensions
	if err := validateRuntime(&value); err != nil {
		return Package{}, err
	}
	return value, nil
}

func validateRuntime(value *Package) error {
	if value.Entrypoint != "" && strings.TrimSpace(value.Entrypoint) == "" {
		return fmt.Errorf("entrypoint must be nonblank when provided")
	}
	if value.LuaProfile != "" && strings.TrimSpace(value.LuaProfile) == "" {
		return fmt.Errorf("lua_profile must be nonblank when provided")
	}
	runtimeDeclared := value.Entrypoint != "" || value.LuaProfile != "" || value.HostAPI != nil
	if value.PackageKind == model.PackageKindGameSystem && !runtimeDeclared {
		return fmt.Errorf("game-system package requires an executable runtime declaration")
	}
	if !runtimeDeclared {
		return nil
	}
	if value.Entrypoint == "" || value.LuaProfile == "" || value.HostAPI == nil {
		return fmt.Errorf("runtime declaration requires entrypoint, lua_profile, and host_api together")
	}
	if !validPackageRelativePath(value.Entrypoint) || !strings.HasSuffix(value.Entrypoint, ".lua") {
		return fmt.Errorf("entrypoint %q must be a canonical relative Lua source path", value.Entrypoint)
	}
	if !luaProfilePattern.MatchString(value.LuaProfile) {
		return fmt.Errorf("lua_profile %q is not canonical", value.LuaProfile)
	}
	if value.HostAPI.Major == 0 || value.HostAPI.MinMinor > value.HostAPI.MaxMinor {
		return fmt.Errorf("host_api requires a nonzero major and min_minor <= max_minor")
	}
	return nil
}

func validPackageRelativePath(value string) bool {
	if value == "" || !utf8.ValidString(value) || strings.TrimSpace(value) != value {
		return false
	}
	for _, segment := range strings.Split(value, "/") {
		if !validPackageRelativePathSegment(segment) {
			return false
		}
	}
	return true
}

func validPackageRelativePathSegment(segment string) bool {
	if segment == "" || segment == "." || segment == ".." || strings.ContainsAny(segment, `/\:`) {
		return false
	}
	for _, character := range segment {
		if unicode.IsControl(character) {
			return false
		}
	}
	return true
}

func bundleFromRaw(raw rawBundle) (Bundle, error) {
	artifacts := make([]BundleArtifact, 0, len(raw.Artifacts))
	for _, rawArtifact := range raw.Artifacts {
		artifacts = append(artifacts, BundleArtifact{
			PackageID: model.PackageID(rawArtifact.PackageID), Version: model.Version(rawArtifact.Version), ContentHash: model.ContentHash(rawArtifact.ContentHash),
		})
	}
	rights, err := rightsFromRaw(raw.Rights)
	if err != nil {
		return Bundle{}, err
	}
	return NormalizeBundle(Bundle{
		SchemaVersion: raw.SchemaVersion, BundleID: model.PackageID(raw.BundleID), Version: model.Version(raw.Version), DisplayName: raw.DisplayName,
		Build:     model.BuildProvenance{Source: raw.Build.Source, Revision: raw.Build.Revision, Builder: raw.Build.Builder},
		Rights:    rights,
		Artifacts: artifacts,
	})
}

// NormalizeBundle validates a directly constructed distribution container and
// returns the same canonical representation used by TOML parsing.
func NormalizeBundle(value Bundle) (Bundle, error) {
	if value.SchemaVersion != SchemaVersion {
		return Bundle{}, fmt.Errorf("unsupported manifest schema_version %d", value.SchemaVersion)
	}
	id, err := model.ParsePackageID(value.BundleID.String())
	if err != nil {
		return Bundle{}, fmt.Errorf("bundle_id: %w", err)
	}
	version, err := model.ParseVersion(value.Version.String())
	if err != nil {
		return Bundle{}, err
	}
	build, err := model.NormalizeBuildProvenance(value.Build)
	if err != nil {
		return Bundle{}, err
	}
	rights, err := model.NormalizeRights(value.Rights)
	if err != nil {
		return Bundle{}, err
	}
	artifacts := make([]BundleArtifact, 0, len(value.Artifacts))
	seen := make(map[string]struct{}, len(value.Artifacts))
	for _, artifact := range value.Artifacts {
		packageID, parseErr := model.ParsePackageID(artifact.PackageID.String())
		if parseErr != nil {
			return Bundle{}, parseErr
		}
		artifactVersion, parseErr := model.ParseVersion(artifact.Version.String())
		if parseErr != nil {
			return Bundle{}, parseErr
		}
		contentHash, parseErr := model.ParseContentHash(artifact.ContentHash.String())
		if parseErr != nil {
			return Bundle{}, parseErr
		}
		key := packageID.String() + "@" + artifactVersion.String() + "#" + contentHash.String()
		if _, exists := seen[key]; exists {
			return Bundle{}, fmt.Errorf("bundle artifact %q is repeated", key)
		}
		seen[key] = struct{}{}
		artifacts = append(artifacts, BundleArtifact{PackageID: packageID, Version: artifactVersion, ContentHash: contentHash})
	}
	if len(artifacts) == 0 {
		return Bundle{}, fmt.Errorf("bundle requires at least one exact package artifact")
	}
	sort.Slice(artifacts, func(i, j int) bool {
		if artifacts[i].PackageID != artifacts[j].PackageID {
			return artifacts[i].PackageID < artifacts[j].PackageID
		}
		if artifacts[i].Version != artifacts[j].Version {
			return artifacts[i].Version < artifacts[j].Version
		}
		return artifacts[i].ContentHash < artifacts[j].ContentHash
	})
	displayName := strings.TrimSpace(value.DisplayName)
	if displayName == "" || len(displayName) > 200 || !utf8.ValidString(displayName) {
		return Bundle{}, fmt.Errorf("display_name must be non-empty UTF-8 of at most 200 bytes")
	}
	return Bundle{
		SchemaVersion: SchemaVersion, BundleID: id, Version: version, DisplayName: displayName,
		Build: build, Rights: rights, Artifacts: artifacts,
	}, nil
}
