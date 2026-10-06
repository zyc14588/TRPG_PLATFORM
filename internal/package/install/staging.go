// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package install

import (
	"context"
	"fmt"
	"io"
	"os"
	"path/filepath"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
)

const MaxGraphPackages = 128
const MaxGraphBytes = 256 << 20

type Input struct {
	Archive  io.Reader
	Evidence Evidence
}
type staged struct {
	pkg      *archive.Package
	approval Approval
	evidence Evidence
}

// staging never unpacks a path from the package onto the host. All validation
// uses bounded immutable snapshots in a fresh private operator-selected root.
func stage(ctx context.Context, root string, inputs []Input, support extension.Support) ([]staged, func(), error) {
	if len(inputs) == 0 || len(inputs) > MaxGraphPackages {
		return nil, func() {}, fmt.Errorf("package graph size rejected")
	}
	dir, err := os.MkdirTemp(root, "install-")
	if err != nil {
		return nil, func() {}, err
	}
	cleanup := func() { _ = os.RemoveAll(dir) }
	result := make([]staged, 0, len(inputs))
	total := int64(0)
	expanded := int64(0)
	for index, in := range inputs {
		if err = ctx.Err(); err != nil {
			return nil, cleanup, err
		}
		if in.Archive == nil {
			return nil, cleanup, fmt.Errorf("archive input is missing")
		}
		file := filepath.Join(dir, fmt.Sprintf("%03d.zip", index))
		f, e := os.OpenFile(file, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0600)
		if e != nil {
			return nil, cleanup, e
		}
		limit := int64(archive.MaxSnapshotBytes)
		if remaining := int64(MaxGraphBytes) - total; remaining < limit {
			limit = remaining
		}
		n, e := io.Copy(f, io.LimitReader(contextReader{ctx, in.Archive}, limit+1))
		total += n
		if e == nil && n > limit {
			e = fmt.Errorf("archive or graph bytes exceed limit")
		}
		if e == nil {
			e = f.Chmod(0400)
		}
		if e == nil {
			e = f.Sync()
		}
		closeErr := f.Close()
		if e == nil {
			e = closeErr
		}
		if e != nil {
			return nil, cleanup, e
		}
		pkg, e := archive.ImportFile(file, support)
		if e != nil {
			return nil, cleanup, e
		}
		expanded += int64(len(pkg.LockBytes()) + len(pkg.ArtifactBytes()))
		for _, entry := range pkg.Entries() {
			expanded += int64(len(entry.Bytes()))
		}
		if expanded > MaxGraphBytes {
			return nil, cleanup, fmt.Errorf("expanded package graph exceeds limit")
		}
		result = append(result, staged{pkg: pkg, evidence: in.Evidence})
	}
	return result, cleanup, nil
}

type contextReader struct {
	ctx    context.Context
	reader io.Reader
}

func (r contextReader) Read(b []byte) (int, error) {
	if err := r.ctx.Err(); err != nil {
		return 0, err
	}
	return r.reader.Read(b)
}
