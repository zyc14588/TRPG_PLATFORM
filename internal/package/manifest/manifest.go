// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package manifest parses and validates the version-one TOML package artifact
// contract. It performs no installation, activation, or runtime execution.
package manifest

import (
	"fmt"
	"path"
	"regexp"
	"sort"
	"strings"
	"unicode/utf8"

	"github.com/BurntSushi/toml"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

const (
	SchemaVersion    = 1
	MaxManifestBytes = 1 << 20
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
	Entrypoint    string           `toml:"entrypoint"`
	LuaProfile    string           `toml:"lua_profile"`
	HostAPI       *rawHostAPIRange `toml:"host_api"`
	Build         rawBuild         `toml:"build"`
	Rights        rawRights        `toml:"rights"`
	Capabilities  *rawCapabilities `toml:"capabilities"`
	Dependencies  []rawDependency  `toml:"dependencies"`
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
	Authors           []string `toml:"authors"`
	Source            string   `toml:"source"`
	LicenseExpression string   `toml:"license_expression"`
	Statement         string   `toml:"statement"`
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
	if header.SchemaVersion != SchemaVersion {
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
	rights, err := model.NormalizeRights(model.Rights{
		Authors: raw.Rights.Authors, Source: raw.Rights.Source,
		LicenseExpression: raw.Rights.LicenseExpression, Statement: raw.Rights.Statement,
	})
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
	value := Package{
		SchemaVersion: raw.SchemaVersion, PackageID: id, PackageKind: kind, Version: version,
		DisplayName: strings.TrimSpace(raw.DisplayName), Entrypoint: strings.TrimSpace(raw.Entrypoint),
		LuaProfile: strings.TrimSpace(raw.LuaProfile), Build: build, Rights: rights,
		Capabilities: capabilities, Dependencies: requirements,
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
	if value.SchemaVersion != SchemaVersion {
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
	value.SchemaVersion = SchemaVersion
	value.PackageID = id
	value.PackageKind = kind
	value.Version = version
	value.DisplayName = displayName
	value.Build = build
	value.Rights = rights
	value.Dependencies = requirements
	if err := validateRuntime(&value); err != nil {
		return Package{}, err
	}
	return value, nil
}

func validateRuntime(value *Package) error {
	value.Entrypoint = strings.TrimSpace(value.Entrypoint)
	value.LuaProfile = strings.TrimSpace(value.LuaProfile)
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
	if !validRelativeEntrypoint(value.Entrypoint) || !strings.HasSuffix(value.Entrypoint, ".lua") {
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

func validRelativeEntrypoint(value string) bool {
	return utf8.ValidString(value) && !strings.Contains(value, "\\") && !strings.HasPrefix(value, "/") && path.Clean(value) == value && value != "." && value != ".." && !strings.HasPrefix(value, "../")
}

func bundleFromRaw(raw rawBundle) (Bundle, error) {
	id, err := model.ParsePackageID(raw.BundleID)
	if err != nil {
		return Bundle{}, fmt.Errorf("bundle_id: %w", err)
	}
	version, err := model.ParseVersion(raw.Version)
	if err != nil {
		return Bundle{}, err
	}
	build, err := model.NormalizeBuildProvenance(model.BuildProvenance{Source: raw.Build.Source, Revision: raw.Build.Revision, Builder: raw.Build.Builder})
	if err != nil {
		return Bundle{}, err
	}
	rights, err := model.NormalizeRights(model.Rights{
		Authors: raw.Rights.Authors, Source: raw.Rights.Source,
		LicenseExpression: raw.Rights.LicenseExpression, Statement: raw.Rights.Statement,
	})
	if err != nil {
		return Bundle{}, err
	}
	artifacts := make([]BundleArtifact, 0, len(raw.Artifacts))
	seen := make(map[string]struct{}, len(raw.Artifacts))
	for _, rawArtifact := range raw.Artifacts {
		packageID, parseErr := model.ParsePackageID(rawArtifact.PackageID)
		if parseErr != nil {
			return Bundle{}, parseErr
		}
		artifactVersion, parseErr := model.ParseVersion(rawArtifact.Version)
		if parseErr != nil {
			return Bundle{}, parseErr
		}
		contentHash, parseErr := model.ParseContentHash(rawArtifact.ContentHash)
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
	displayName := strings.TrimSpace(raw.DisplayName)
	if displayName == "" || len(displayName) > 200 || !utf8.ValidString(displayName) {
		return Bundle{}, fmt.Errorf("display_name must be non-empty UTF-8 of at most 200 bytes")
	}
	return Bundle{
		SchemaVersion: SchemaVersion, BundleID: id, Version: version, DisplayName: displayName,
		Build: build, Rights: rights, Artifacts: artifacts,
	}, nil
}
