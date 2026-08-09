// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/manifest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

const maxLockBytes = 8 << 20

func runPackage(args []string, stdout, stderr io.Writer) int {
	if len(args) == 0 {
		fmt.Fprintln(stderr, "usage: creator-cli package <validate|build> [options]")
		return 2
	}
	switch args[0] {
	case "validate":
		return runPackageValidate(args[1:], stdout, stderr)
	case "build":
		return runPackageBuild(args[1:], stdout, stderr)
	default:
		fmt.Fprintf(stderr, "unknown package command %q\n", args[0])
		return 2
	}
}

func runPackageValidate(args []string, stdout, stderr io.Writer) int {
	flags := flag.NewFlagSet("package validate", flag.ContinueOnError)
	flags.SetOutput(stderr)
	manifestPath := flags.String("manifest", "", "path to the versioned TOML manifest")
	lockPath := flags.String("lock", "", "path to the exact dependency lock (required for package artifacts)")
	resolveCapabilities := flags.Bool("resolve-capabilities", false, "validate required capabilities against explicit trust and execution grants")
	var trustGrants stringListFlag
	var contextGrants stringListFlag
	flags.Var(&trustGrants, "trust-grant", "capability allowed by the selected trust policy (repeatable)")
	flags.Var(&contextGrants, "context-grant", "capability allowed by the execution context (repeatable)")
	if err := flags.Parse(args); err != nil {
		return 2
	}
	if flags.NArg() != 0 || *manifestPath == "" {
		fmt.Fprintln(stderr, "package validate requires --manifest and no positional arguments")
		return 2
	}
	document, err := loadManifest(*manifestPath)
	if err != nil {
		return reportPackageError(stderr, err)
	}
	if !*resolveCapabilities && (len(trustGrants) > 0 || len(contextGrants) > 0) {
		return reportPackageError(stderr, errors.New("capability grants require --resolve-capabilities"))
	}
	result := validationResult{SchemaVersion: 1, Valid: true, ArtifactType: document.ArtifactType}
	switch document.ArtifactType {
	case model.ArtifactTypePackage:
		if *lockPath == "" {
			return reportPackageError(stderr, errors.New("package artifact validation requires --lock"))
		}
		if *resolveCapabilities {
			if resolveErr := validateCapabilityResolution(document.Package.Capabilities, trustGrants, contextGrants); resolveErr != nil {
				return reportPackageError(stderr, resolveErr)
			}
		}
		lock, loadErr := loadLock(*lockPath)
		if loadErr != nil {
			return reportPackageError(stderr, loadErr)
		}
		root, exists := lock.Package(lock.Root())
		if !exists {
			return reportPackageError(stderr, errors.New("exact lock has no root package"))
		}
		identity, buildErr := manifest.BuildArtifactIdentity(*document.Package, root.ContentHash, lock)
		if buildErr != nil {
			return reportPackageError(stderr, buildErr)
		}
		lockDigest, digestErr := lock.Digest()
		if digestErr != nil {
			return reportPackageError(stderr, digestErr)
		}
		result.PackageID = document.Package.PackageID
		result.Version = document.Package.Version
		result.LockDigest = lockDigest
		result.ArtifactDigest = identity.Digest()
	case model.ArtifactTypeBundle:
		if *resolveCapabilities {
			return reportPackageError(stderr, errors.New("Bundle validation does not resolve runtime capabilities"))
		}
		if *lockPath != "" {
			return reportPackageError(stderr, errors.New("Bundle validation does not accept a runtime dependency lock"))
		}
		result.BundleID = document.Bundle.BundleID
		result.Version = document.Bundle.Version
	default:
		return reportPackageError(stderr, fmt.Errorf("unsupported artifact type %q", document.ArtifactType))
	}
	if err := writeJSON(stdout, result); err != nil {
		return reportPackageError(stderr, err)
	}
	return 0
}

type stringListFlag []string

func (values *stringListFlag) String() string {
	return strings.Join(*values, ",")
}

func (values *stringListFlag) Set(value string) error {
	*values = append(*values, value)
	return nil
}

func validateCapabilityResolution(declaration capability.Declaration, trustValues, contextValues []string) error {
	policy, err := capability.NewTrustPolicy(map[capability.TrustLevel][]string{
		capability.TrustOfficial:          trustValues,
		capability.TrustSigned:            {},
		capability.TrustPrivateUnverified: {},
		capability.TrustDevelopment:       {},
	})
	if err != nil {
		return fmt.Errorf("validate trust grants: %w", err)
	}
	context, err := capability.NewGrantSet(contextValues)
	if err != nil {
		return fmt.Errorf("validate execution-context grants: %w", err)
	}
	if _, err := capability.Resolve(declaration, capability.TrustOfficial, policy, context); err != nil {
		return fmt.Errorf("resolve manifest capabilities: %w", err)
	}
	return nil
}

func runPackageBuild(args []string, stdout, stderr io.Writer) int {
	flags := flag.NewFlagSet("package build", flag.ContinueOnError)
	flags.SetOutput(stderr)
	manifestPath := flags.String("manifest", "", "path to the versioned TOML manifest")
	lockPath := flags.String("lock", "", "path to the exact dependency lock")
	contentHash := flags.String("content-hash", "", "canonical sha256 content hash of the immutable package bytes")
	if err := flags.Parse(args); err != nil {
		return 2
	}
	if flags.NArg() != 0 || *manifestPath == "" || *lockPath == "" || *contentHash == "" {
		fmt.Fprintln(stderr, "package build requires --manifest, --lock, --content-hash, and no positional arguments")
		return 2
	}
	document, err := loadManifest(*manifestPath)
	if err != nil {
		return reportPackageError(stderr, err)
	}
	if document.ArtifactType != model.ArtifactTypePackage {
		return reportPackageError(stderr, errors.New("package build accepts executable package manifests, not Bundle containers"))
	}
	lock, err := loadLock(*lockPath)
	if err != nil {
		return reportPackageError(stderr, err)
	}
	hash, err := model.ParseContentHash(*contentHash)
	if err != nil {
		return reportPackageError(stderr, err)
	}
	identity, err := manifest.BuildArtifactIdentity(*document.Package, hash, lock)
	if err != nil {
		return reportPackageError(stderr, err)
	}
	result := buildResult{
		SchemaVersion: 1, ArtifactIdentity: json.RawMessage(identity.CanonicalJSON()), ArtifactDigest: identity.Digest(),
	}
	if err := writeJSON(stdout, result); err != nil {
		return reportPackageError(stderr, err)
	}
	return 0
}

type validationResult struct {
	SchemaVersion  int                `json:"schema_version"`
	Valid          bool               `json:"valid"`
	ArtifactType   model.ArtifactType `json:"artifact_type"`
	PackageID      model.PackageID    `json:"package_id,omitempty"`
	BundleID       model.PackageID    `json:"bundle_id,omitempty"`
	Version        model.Version      `json:"version"`
	LockDigest     model.ContentHash  `json:"lock_digest,omitempty"`
	ArtifactDigest model.ContentHash  `json:"artifact_digest,omitempty"`
}

type buildResult struct {
	SchemaVersion    int               `json:"schema_version"`
	ArtifactIdentity json.RawMessage   `json:"artifact_identity"`
	ArtifactDigest   model.ContentHash `json:"artifact_digest"`
}

func loadManifest(path string) (manifest.Document, error) {
	data, err := readFileLimited(path, manifest.MaxManifestBytes)
	if err != nil {
		return manifest.Document{}, fmt.Errorf("read manifest: %w", err)
	}
	document, err := manifest.Parse(data)
	if err != nil {
		return manifest.Document{}, fmt.Errorf("validate manifest: %w", err)
	}
	return document, nil
}

func loadLock(path string) (dependency.ExactLock, error) {
	data, err := readFileLimited(path, maxLockBytes)
	if err != nil {
		return dependency.ExactLock{}, fmt.Errorf("read exact lock: %w", err)
	}
	lock, err := dependency.ParseExactLock(data)
	if err != nil {
		return dependency.ExactLock{}, fmt.Errorf("validate exact lock: %w", err)
	}
	return lock, nil
}

func readFileLimited(path string, maximum int64) ([]byte, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer file.Close()
	data, err := io.ReadAll(io.LimitReader(file, maximum+1))
	if err != nil {
		return nil, err
	}
	if int64(len(data)) > maximum {
		return nil, fmt.Errorf("file exceeds %d bytes", maximum)
	}
	return data, nil
}

func writeJSON(writer io.Writer, value any) error {
	data, err := json.Marshal(value)
	if err != nil {
		return fmt.Errorf("encode result: %w", err)
	}
	if _, err := fmt.Fprintln(writer, string(data)); err != nil {
		return fmt.Errorf("write result: %w", err)
	}
	return nil
}

func reportPackageError(stderr io.Writer, err error) int {
	fmt.Fprintf(stderr, "package contract error: %v\n", err)
	return 1
}
