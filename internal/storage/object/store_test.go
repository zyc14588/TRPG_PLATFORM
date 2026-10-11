// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package object

import (
	"bytes"
	"context"
	"os"
	"path/filepath"
	"testing"
)

func TestImmutableStoreBoundaryRetainsSharedObjectsAndRejectsCorruption(t *testing.T) {
	dir := t.TempDir()
	if e := os.Chmod(dir, 0700); e != nil {
		t.Fatal(e)
	}
	d, e := Open(dir)
	if e != nil {
		t.Fatal(e)
	}
	defer d.Close()
	var store Store = d
	ctx := context.Background()
	body := []byte("owned shared immutable archive bytes")
	key, e := store.Put(ctx, body)
	if e != nil {
		t.Fatal(e)
	}
	other, e := Open(dir)
	if e != nil {
		t.Fatal(e)
	}
	defer other.Close()
	duplicate, e := other.Put(ctx, bytes.Clone(body))
	if e != nil || duplicate != key {
		t.Fatal("duplicate writer changed immutable identity", e)
	}
	cancelled, cancel := context.WithCancel(ctx)
	cancel()
	if _, e = store.Put(cancelled, []byte("cancelled staging")); e == nil {
		t.Fatal("cancelled write published")
	}
	got, e := store.Read(ctx, key)
	if e != nil || !bytes.Equal(got, body) || store.Verify(ctx, key) != nil {
		t.Fatal("shared committed bytes lost", e)
	}
	file := filepath.Join(dir, key[7:])
	if e = os.Chmod(file, 0600); e != nil {
		t.Fatal(e)
	}
	if e = os.WriteFile(file, []byte("owned corruption"), 0400); e != nil {
		t.Fatal(e)
	}
	if _, e = store.Read(ctx, key); e == nil || store.Verify(ctx, key) == nil {
		t.Fatal("corrupt object crossed immutable boundary")
	}
	if _, e = store.Put(ctx, body); e == nil {
		t.Fatal("duplicate Put replaced a corrupt shared object")
	}
}
