//go:build linux

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package object

import (
	"context"
	"os"
	"path/filepath"
	"sync"
	"testing"
)

func TestImmutableConcurrentDeduplicationAndTamperRejection(t *testing.T) {
	root := t.TempDir()
	if err := os.Chmod(root, 0700); err != nil {
		t.Fatal(err)
	}
	d, err := Open(root)
	if err != nil {
		t.Fatal(err)
	}
	defer d.Close()
	data := []byte("shared bytes, separate metadata")
	key := Hash(data)
	var wg sync.WaitGroup
	for range 12 {
		wg.Go(func() {
			got, e := d.Put(context.Background(), data)
			if e != nil || got != key {
				t.Errorf("Put = %q, %v", got, e)
			}
		})
	}
	wg.Wait()
	rows, err := os.ReadDir(root)
	if err != nil || len(rows) != 1 {
		t.Fatal("dedup/pending cleanup", rows, err)
	}
	got, err := d.Read(context.Background(), key)
	if err != nil || string(got) != string(data) {
		t.Fatal(err)
	}
	path := filepath.Join(root, key[7:])
	if err = os.Chmod(path, 0600); err != nil {
		t.Fatal(err)
	}
	if _, err = d.Put(context.Background(), data); err == nil {
		t.Fatal("mutable existing object accepted")
	}
	if err = os.WriteFile(path, []byte("wrong"), 0400); err != nil {
		t.Fatal(err)
	}
	_ = os.Chmod(path, 0400)
	if _, err = d.Read(context.Background(), key); err == nil {
		t.Fatal("tamper accepted")
	}
}

func TestObjectPathsCannotEscapeOrFollowLinks(t *testing.T) {
	root := t.TempDir()
	if err := os.Chmod(root, 0700); err != nil {
		t.Fatal(err)
	}
	d, err := Open(root)
	if err != nil {
		t.Fatal(err)
	}
	defer d.Close()
	for _, key := range []string{"../other", "sha256:../other", "sha256:" + string(make([]byte, 64)), "SHA256:bad"} {
		if _, err = d.Read(context.Background(), key); err == nil {
			t.Fatal("unsafe key accepted", key)
		}
	}
	outside := filepath.Join(t.TempDir(), "outside")
	data := []byte("outside")
	if err = os.WriteFile(outside, data, 0400); err != nil {
		t.Fatal(err)
	}
	if err = os.Symlink(outside, filepath.Join(root, Hash(data)[7:])); err != nil {
		t.Fatal(err)
	}
	if _, err = d.Put(context.Background(), data); err == nil {
		t.Fatal("symlink target accepted")
	}
	public := t.TempDir()
	_ = os.Chmod(public, 0755)
	if _, err = Open(public); err == nil {
		t.Fatal("public root accepted")
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err = d.Put(ctx, []byte("x")); err == nil {
		t.Fatal("cancellation ignored")
	}
}
