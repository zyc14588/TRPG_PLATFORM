// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package archive

import (
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"fmt"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

func computeContentHash(entries []Entry) (model.ContentHash, error) {
	digest := sha256.New()
	var pathLength [4]byte
	var contentLength [8]byte
	for _, entry := range entries {
		if entry.path == PlatformLockPath || entry.path == ArtifactPath {
			return "", fmt.Errorf("outer envelope cannot enter the content hash")
		}
		binary.BigEndian.PutUint32(pathLength[:], uint32(len(entry.path)))
		binary.BigEndian.PutUint64(contentLength[:], uint64(len(entry.data)))
		_, _ = digest.Write(pathLength[:])
		_, _ = digest.Write([]byte(entry.path))
		_, _ = digest.Write(contentLength[:])
		_, _ = digest.Write(entry.data)
	}
	hash, err := model.ParseContentHash("sha256:" + hex.EncodeToString(digest.Sum(nil)))
	if err != nil {
		return "", err
	}
	return hash, nil
}
