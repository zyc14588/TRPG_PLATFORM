// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package packagedata

import (
	"bytes"
	"encoding/json"
	"sort"
	"strings"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

// SessionLock supplements the canonical transitive lock with the authenticated
// artifact identities, per-package runtime contract and complete Schema set.
// It is an internal immutable fact, never a dependency resolver request.
type SessionLock struct {
	Format         uint64           `json:"format"`
	RootIdentity   string           `json:"root_identity"`
	GraphHash      string           `json:"graph_hash"`
	DependencyLock json.RawMessage  `json:"dependency_lock"`
	ArtifactSet    json.RawMessage  `json:"artifact_set"`
	Packages       []LockedArtifact `json:"packages"`
	Hash           string           `json:"hash"`
}
type LockedArtifact struct {
	Identity, PackageID, Version, Kind, ContentHash, PolicyDigest, ValidationDigest string
	LuaProfile                                                                      string
	HostAPI                                                                         *[3]uint32
	Schemas                                                                         map[string]string
}
type lockArtifactEvidence struct {
	Identity, PackageID, ContentHash, PolicyDigest, ValidationDigest string
}

func LockHash(l SessionLock) string {
	l.Hash = ""
	raw, _ := json.Marshal(l)
	return checkpoint.Hash(raw)
}
func ValidateSessionLock(l SessionLock) error {
	raw, err := json.Marshal(l)
	if err != nil || len(raw) > 256<<10 || l.Format != 1 || len(l.Packages) == 0 || len(l.Packages) > 128 || !checkpoint.IsDigest(l.RootIdentity) || l.Hash != LockHash(l) {
		return ErrDenied
	}
	lock, err := dependency.ParseExactLock(l.DependencyLock)
	if err != nil {
		return ErrDenied
	}
	canonical, err := lock.CanonicalJSON()
	hash, e := lock.Digest()
	if err != nil || e != nil || !bytes.Equal(canonical, l.DependencyLock) || string(hash) != l.GraphHash || len(lock.Packages()) != len(l.Packages) {
		return ErrDenied
	}
	var artifacts []lockArtifactEvidence
	if checkpoint.StrictDecode(l.ArtifactSet, &artifacts, 256<<10) != nil || len(artifacts) != len(l.Packages) {
		return ErrDenied
	}
	last := ""
	rootFound := false
	for n, p := range l.Packages {
		id, e := model.ParsePackageID(p.PackageID)
		v, ok := lock.Package(id)
		if e != nil || !ok || p.PackageID <= last || p.Version != string(v.Version) || p.ContentHash != string(v.ContentHash) || !checkpoint.IsDigest(p.Identity) || !checkpoint.IsDigest(p.PolicyDigest) || !checkpoint.IsDigest(p.ValidationDigest) {
			return ErrDenied
		}
		if p.Kind != "game-system" && p.Kind != "content" && p.Kind != "assets" && p.Kind != "ui-extension" && p.Kind != "library" {
			return ErrDenied
		}
		if p.HostAPI != nil && (p.HostAPI[0] == 0 || p.HostAPI[1] > p.HostAPI[2]) {
			return ErrDenied
		}
		if len(p.Schemas) > 128 {
			return ErrDenied
		}
		for path, h := range p.Schemas {
			if len(path) > 512 || !strings.HasSuffix(path, ".schema.json") || strings.HasPrefix(path, "/") || strings.Contains(path, "\\") || strings.Contains(path, "..") || !checkpoint.IsDigest(h) {
				return ErrDenied
			}
		}
		a := artifacts[n]
		if a.Identity != p.Identity || a.PackageID != p.PackageID || a.ContentHash != p.ContentHash || a.PolicyDigest != p.PolicyDigest || a.ValidationDigest != p.ValidationDigest {
			return ErrDenied
		}
		if id == lock.Root() {
			rootFound = p.Identity == l.RootIdentity && p.Kind == "game-system"
		}
		last = p.PackageID
	}
	if !rootFound || !sort.SliceIsSorted(artifacts, func(i, j int) bool { return artifacts[i].PackageID < artifacts[j].PackageID }) {
		return ErrDenied
	}
	return nil
}

// MigrationTransition is emitted exclusively by a trusted operator transaction.
// Existing records omit it byte-for-byte. It changes the reduction context,
// without modifying creation, original events, requests or receipts.
type MigrationTransition struct {
	PointID   string                     `json:"point_id"`
	PointHash string                     `json:"point_hash"`
	Direction string                     `json:"direction"`
	From      SessionLock                `json:"from"`
	To        SessionLock                `json:"to"`
	Before    checkpoint.RecoveryBinding `json:"before"`
	After     checkpoint.RecoveryBinding `json:"after"`
	WasEnded  bool                       `json:"was_ended,omitempty"`
}

// RecoveryPoint is the bounded pre-upgrade database image. Raw cache and row
// bytes are retained separately from the image independently proved by replay.
// Restoration is restricted to this recorded point and its immediate upgrade.
type RecoveryPoint struct {
	ID                     string      `json:"id"`
	Lock                   SessionLock `json:"lock"`
	Version, Cursor        uint64
	Ended                  bool
	State                  []byte
	Graph                  []byte
	OriginLock, ActiveLock []byte
	Projection, Checkpoint []byte
	Documents              []PointDocument
	Quantities             []PointQuantity
	Targets                []string
	Objects                []PointObject
	Verified               Snapshot
	HistoryHash            string
	VerifiedCheckpoint     CheckpointCache
	Hash                   string `json:"hash"`
}
type PointDocument struct {
	Row                Row
	Bytes              []byte
	CommandID, EventID string
}
type PointQuantity struct {
	Quantity           Quantity
	CommandID, EventID string
}
type PointObject struct{ Identity, Path, Key string }

const MaxPointBytes = 8 << 20

func PointHash(p RecoveryPoint) string {
	p.Hash = ""
	raw, _ := json.Marshal(p)
	return checkpoint.Hash(raw)
}
func ValidatePoint(p RecoveryPoint) error {
	raw, err := json.Marshal(p)
	if err != nil || len(raw) > MaxPointBytes || p.ID == "" || len(p.ID) > 128 || p.Hash != PointHash(p) || ValidateSessionLock(p.Lock) != nil || p.Version == 0 || p.Verified.Version != p.Version || p.Verified.Binding.GraphHash != p.Lock.GraphHash || p.Verified.SchemaHash == "" || !checkpoint.IsDigest(p.HistoryHash) || len(p.Documents) > 128 || len(p.Quantities) > 128 || len(p.Objects) > 4096 || len(p.Targets) > 128 {
		return ErrDenied
	}
	return nil
}
