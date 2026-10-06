// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package object stores private, immutable bytes. Object keys are integrity
// identifiers, never access credentials. Only the workspace store serves them.
package object

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

const MaxBytes = 80 << 20

var ErrIntegrity = errors.New("object integrity check failed")

type Directory struct{ root *os.Root }

// Open requires an existing, operator-owned private directory. It never accepts
// a package path. The operator must not mutate committed files outside this API.
func Open(path string) (*Directory, error) {
	if !filepath.IsAbs(path) {
		return nil, fmt.Errorf("object root must be absolute")
	}
	info, err := os.Lstat(path)
	if err != nil {
		return nil, err
	}
	if !info.IsDir() || info.Mode()&os.ModeSymlink != 0 || info.Mode().Perm()&0077 != 0 {
		return nil, fmt.Errorf("object root must be a private directory")
	}
	r, err := os.OpenRoot(path)
	if err != nil {
		return nil, err
	}
	return &Directory{root: r}, nil
}

func (d *Directory) Close() error { return d.root.Close() }

func Hash(data []byte) string {
	sum := sha256.Sum256(data)
	return "sha256:" + hex.EncodeToString(sum[:])
}
func name(key string) (string, error) {
	h, err := model.ParseContentHash(key)
	if err != nil {
		return "", ErrIntegrity
	}
	return h.String()[7:], nil
}

// Put atomically links a complete fsynced file under a never-replaced hash key.
// A losing writer verifies the winner. Orphaned private objects are retained:
// removing them after an unknown DB commit could destroy a concurrent install.
func (d *Directory) Put(ctx context.Context, data []byte) (string, error) {
	if err := ctx.Err(); err != nil {
		return "", err
	}
	if len(data) > MaxBytes {
		return "", ErrIntegrity
	}
	key := Hash(data)
	target, _ := name(key)
	if _, err := d.root.Lstat(target); err == nil {
		if err = d.Verify(ctx, key); err != nil {
			return "", err
		}
		return key, d.syncRoot()
	} else if !errors.Is(err, os.ErrNotExist) {
		return "", err
	}
	var random [16]byte
	if _, err := rand.Read(random[:]); err != nil {
		return "", err
	}
	tmp := ".pending-" + hex.EncodeToString(random[:])
	f, err := d.root.OpenFile(tmp, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0600)
	if err != nil {
		return "", err
	}
	defer d.root.Remove(tmp)
	defer f.Close()
	if _, err = f.Write(data); err != nil {
		return "", err
	}
	if err = f.Chmod(0400); err != nil {
		return "", err
	}
	if err = f.Sync(); err != nil {
		return "", err
	}
	if err = f.Close(); err != nil {
		return "", err
	}
	if err = ctx.Err(); err != nil {
		return "", err
	}
	if err = d.root.Link(tmp, target); err != nil && !errors.Is(err, os.ErrExist) {
		return "", err
	}
	if err = d.Verify(ctx, key); err != nil {
		return "", err
	}
	// Persist the directory entry before allowing any database reference to it.
	if err = d.syncRoot(); err != nil {
		return "", err
	}
	return key, nil
}

func (d *Directory) syncRoot() error {
	dir, err := d.root.Open(".")
	if err != nil {
		return err
	}
	defer dir.Close()
	return dir.Sync()
}

func (d *Directory) Read(ctx context.Context, key string) ([]byte, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	n, err := name(key)
	if err != nil {
		return nil, err
	}
	before, err := d.root.Lstat(n)
	if err != nil {
		return nil, err
	}
	if !before.Mode().IsRegular() || before.Mode().Perm()&0222 != 0 || before.Size() > MaxBytes {
		return nil, ErrIntegrity
	}
	f, err := d.root.Open(n)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	opened, err := f.Stat()
	if err != nil {
		return nil, err
	}
	if !opened.Mode().IsRegular() || !os.SameFile(before, opened) {
		return nil, ErrIntegrity
	}
	data, err := io.ReadAll(io.LimitReader(f, MaxBytes+1))
	if err != nil {
		return nil, err
	}
	if len(data) > MaxBytes || Hash(data) != key {
		return nil, ErrIntegrity
	}
	if err = ctx.Err(); err != nil {
		return nil, err
	}
	return data, nil
}

func (d *Directory) Verify(ctx context.Context, key string) error {
	_, err := d.Read(ctx, key)
	return err
}
