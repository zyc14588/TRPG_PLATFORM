// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package archive

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

// Snapshot owns one immutable copy of archive bytes. The size check happens
// before copying so an oversized caller buffer cannot trigger a second large
// allocation. All public byte accessors remain defensive.
type Snapshot struct {
	data []byte
	hash model.ContentHash
}

// NewSnapshot copies data into a bounded immutable archive snapshot.
func NewSnapshot(data []byte) (Snapshot, error) {
	if len(data) > MaxSnapshotBytes {
		return Snapshot{}, fmt.Errorf("archive snapshot exceeds %d bytes", MaxSnapshotBytes)
	}
	return newOwnedSnapshot(append([]byte(nil), data...))
}

// newOwnedSnapshot takes ownership of data. It is private so every external
// caller still crosses the defensive-copy boundary in NewSnapshot.
func newOwnedSnapshot(data []byte) (Snapshot, error) {
	if len(data) > MaxSnapshotBytes {
		return Snapshot{}, fmt.Errorf("archive snapshot exceeds %d bytes", MaxSnapshotBytes)
	}
	digest := sha256.Sum256(data)
	hash, err := model.ParseContentHash("sha256:" + hex.EncodeToString(digest[:]))
	if err != nil {
		return Snapshot{}, err
	}
	return Snapshot{data: data, hash: hash}, nil
}

// Bytes returns a defensive copy of the complete ZIP snapshot.
func (snapshot Snapshot) Bytes() []byte { return append([]byte(nil), snapshot.data...) }

// Size returns the immutable snapshot byte length.
func (snapshot Snapshot) Size() int { return len(snapshot.data) }

// Hash returns SHA-256 over the final ZIP bytes, not the package content hash.
func (snapshot Snapshot) Hash() model.ContentHash { return snapshot.hash }
