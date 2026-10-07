// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package credential

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"
)

func need(t *testing.T, e error) {
	t.Helper()
	if e != nil {
		t.Fatalf("credential operation failed: %v", auth.SafeError(e))
	}
}
func master(t *testing.T) string {
	t.Helper()
	p := filepath.Join(t.TempDir(), "master")
	need(t, os.WriteFile(p, bytes.Repeat([]byte{71}, 32), 0400))
	return p
}
func binding() Binding {
	return Binding{Scope: core.Scope{WorkspaceID: "w", RoomID: "r", GameID: "g"}, SeatID: "ai", ID: "key", OwnerKind: "account", OwnerID: "owner", Lifetime: Temporary, ExpiresAt: time.Now().UTC().Truncate(time.Microsecond).Add(time.Hour), Version: 1}
}
func TestVaultReadonlyFileAndEncryptedRoundtrip(t *testing.T) {
	v, e := New(master(t))
	need(t, e)
	defer v.Close()
	raw := []byte("fixture-provider-secret-private-9345")
	k, e := NewKey(raw)
	need(t, e)
	defer k.Close()
	b := binding()
	b.Lifetime = Retained
	b.ExpiresAt = time.Time{}
	r, e := v.Seal(context.Background(), b, k, time.Now())
	need(t, e)
	if bytes.Contains(r.StorageValue().Ciphertext, raw) {
		t.Fatal("plaintext reached storage")
	}
	opened, e := v.Open(context.Background(), r, b, time.Now())
	need(t, e)
	defer opened.Close()
	need(t, opened.Use(func(got []byte) error {
		if !bytes.Equal(got, raw) {
			t.Fatal("roundtrip mismatch")
		}
		return nil
	}))
}
func TestVaultRejectsWritableSymlinkAndWrongSize(t *testing.T) {
	for _, tc := range []struct {
		name    string
		mode    os.FileMode
		size    int
		symlink bool
	}{{"owner-writable", 0600, 32, false}, {"world-readable", 0444, 32, false}, {"short", 0400, 31, false}, {"oversized", 0400, 33, false}, {"symlink", 0400, 32, true}} {
		t.Run(tc.name, func(t *testing.T) {
			dir := t.TempDir()
			p := filepath.Join(dir, "master")
			need(t, os.WriteFile(p, bytes.Repeat([]byte{9}, tc.size), tc.mode))
			if tc.symlink {
				q := filepath.Join(dir, "link")
				need(t, os.Symlink(p, q))
				p = q
			}
			if _, e := New(p); e == nil {
				t.Fatal("unsafe secret file accepted")
			}
		})
	}
}
func TestVaultAuthenticatesEveryScopeAndIdentityField(t *testing.T) {
	v, e := New(master(t))
	need(t, e)
	defer v.Close()
	k, e := NewKey([]byte("fixture-private-bound-token"))
	need(t, e)
	defer k.Close()
	b := binding()
	r, e := v.Seal(context.Background(), b, k, time.Now())
	need(t, e)
	changes := []func(*Binding){func(x *Binding) { x.Scope.WorkspaceID = "other" }, func(x *Binding) { x.Scope.RoomID = "other" }, func(x *Binding) { x.Scope.GameID = "other" }, func(x *Binding) { x.SeatID = "other" }, func(x *Binding) { x.ID = "other" }, func(x *Binding) { x.OwnerID = "other" }, func(x *Binding) { x.OwnerKind = "guest" }, func(x *Binding) { x.Version++ }, func(x *Binding) { x.ExpiresAt = x.ExpiresAt.Add(time.Second) }, func(x *Binding) { x.Lifetime = Retained; x.ExpiresAt = time.Time{} }}
	for i, change := range changes {
		t.Run(fmt.Sprintf("field-%d", i), func(t *testing.T) {
			altered := b
			change(&altered)
			x := RecordValue(r)
			x.Binding = altered
			if _, e := v.Open(context.Background(), StoredRecord(x), altered, time.Now()); e != auth.ErrDenied {
				t.Fatal("modified authenticated binding accepted")
			}
		})
	}
}
func TestTemporaryVaultCannotRetainOrSurviveRestart(t *testing.T) {
	v, e := New("")
	need(t, e)
	defer v.Close()
	k, e := NewKey([]byte("temporary-fixture-private-key"))
	need(t, e)
	defer k.Close()
	b := binding()
	r, e := v.Seal(context.Background(), b, k, time.Now())
	need(t, e)
	other, e := New("")
	need(t, e)
	defer other.Close()
	if _, e = other.Open(context.Background(), r, b, time.Now()); e != auth.ErrDenied {
		t.Fatal("ephemeral key survived new vault")
	}
	b.Lifetime = Retained
	b.ExpiresAt = time.Time{}
	if _, e = v.Seal(context.Background(), b, k, time.Now()); e != auth.ErrDenied {
		t.Fatal("ephemeral retained key accepted")
	}
}
func TestExpiryRevocationCiphertextAndBoundedLifetime(t *testing.T) {
	v, e := New(master(t))
	need(t, e)
	defer v.Close()
	k, e := NewKey([]byte("private-fixture-token-84294"))
	need(t, e)
	defer k.Close()
	b := binding()
	r, e := v.Seal(context.Background(), b, k, time.Now())
	need(t, e)
	if _, e = v.Open(context.Background(), r, b, b.ExpiresAt); e != auth.ErrDenied {
		t.Fatal("expired key accepted")
	}
	x := RecordValue(r)
	x.Revoked = true
	if _, e = v.Open(context.Background(), StoredRecord(x), b, time.Now()); e != auth.ErrDenied {
		t.Fatal("revoked key accepted")
	}
	x = RecordValue(r)
	x.Ciphertext[len(x.Ciphertext)-1] ^= 1
	if _, e = v.Open(context.Background(), StoredRecord(x), b, time.Now()); e != auth.ErrDenied {
		t.Fatal("modified ciphertext accepted")
	}
	b.ExpiresAt = time.Now().UTC().Truncate(time.Microsecond).Add(MaxTemporaryLifetime + time.Hour)
	if _, e = v.Seal(context.Background(), b, k, time.Now()); e != auth.ErrDenied {
		t.Fatal("unbounded temporary lifetime accepted")
	}
}
func TestKeyCopiesClearsAndSanitizesAdapterFailure(t *testing.T) {
	raw := []byte("owned-copy-fixture-token-49847")
	expected := bytes.Clone(raw)
	k, e := NewKey(raw)
	need(t, e)
	clear(raw)
	var escaped []byte
	need(t, k.Use(func(b []byte) error {
		if !bytes.Equal(b, expected) {
			t.Fatal("caller alias changed key")
		}
		escaped = b
		return nil
	}))
	if !bytes.Equal(escaped, make([]byte, len(escaped))) {
		t.Fatal("adapter copy not cleared")
	}
	e = k.Use(func([]byte) error { return fmt.Errorf("%s", expected) })
	if e != auth.ErrUnavailable {
		t.Fatal("adapter error not canonical")
	}
	k.Close()
	if e = k.Use(func([]byte) error { return nil }); e != auth.ErrDenied {
		t.Fatal("destroyed key reused")
	}
}
func TestPrivateHandlesRejectJSONAndFormattingFallback(t *testing.T) {
	secret := "private-fixture-sentinel-9483794"
	k, e := NewKey([]byte(secret))
	need(t, e)
	defer k.Close()
	v, e := New(master(t))
	need(t, e)
	defer v.Close()
	r, e := v.Seal(context.Background(), binding(), k, time.Now())
	need(t, e)
	for _, handle := range []any{k, &k, v, *v, r, &r} {
		for _, format := range []string{"%v", "%+v", "%#v", "%s", "%q", "%d", "%x", "%#x", "%f", "%*s"} {
			s := fmt.Sprintf(format, handle)
			if strings.Contains(s, secret) {
				t.Fatal("formatting exposed private key")
			}
		}
		if _, e := json.Marshal(handle); e == nil {
			t.Fatal("private handle serialized")
		}
	}
}
func TestKeyAndVaultConcurrentClose(t *testing.T) {
	v, e := New(master(t))
	need(t, e)
	k, e := NewKey([]byte("concurrent-close-fixture-key"))
	need(t, e)
	b := binding()
	r, e := v.Seal(context.Background(), b, k, time.Now())
	need(t, e)
	var wg sync.WaitGroup
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := 0; j < 8; j++ {
				_ = k.Use(func([]byte) error { return nil })
				opened, e := v.Open(context.Background(), r, b, time.Now())
				if e == nil {
					opened.Close()
				}
			}
		}()
	}
	k.Close()
	v.Close()
	wg.Wait()
}
func TestKeyInputLimits(t *testing.T) {
	for _, raw := range [][]byte{nil, []byte("line\nbreak"), []byte("bad\x00key"), bytes.Repeat([]byte{'a'}, MaxKeyBytes+1)} {
		if _, e := NewKey(raw); e != auth.ErrInvalid {
			t.Fatal("invalid key accepted")
		}
	}
}
