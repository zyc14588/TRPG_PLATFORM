// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package archive implements the bounded, deterministic Game Package archive
// envelope. It deliberately performs ZIP metadata preflight before handing an
// immutable snapshot to archive/zip.
package archive

const (
	MaxArchiveEntries        = 4096
	MaxEntryExpandedBytes    = 8 << 20
	MaxArchiveExpandedBytes  = 64 << 20
	MaxCompressionRatio      = 100
	MaxSnapshotBytes         = 80 << 20
	MaxCentralDirectoryBytes = 8 << 20
	MaxArchiveMetadataBytes  = 8 << 20
)

const (
	ManifestPath     = "package.toml"
	PlatformLockPath = "META-INF/platform.lock.json"
	ArtifactPath     = "META-INF/artifact.json"
)
