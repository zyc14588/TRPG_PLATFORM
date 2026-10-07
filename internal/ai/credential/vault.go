// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package credential isolates provider keys from configuration, prompts and
// exports. Only the server's credential adapter may use plaintext briefly.
package credential

import (
	"context"
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"encoding/json"
	"fmt"
	"io"
	"slices"
	"sync"
	"time"

	data "github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

type Lifetime string

const (
	Temporary Lifetime = "session"
	Retained  Lifetime = "retained"
)
const MaxKeyBytes = 4096
const MaxTemporaryLifetime = 24 * time.Hour

// Binding is trusted storage metadata, never a client identity assertion.
// Every key is tied to one existing game and seat, including retained keys.
type Binding struct {
	Scope                          core.Scope
	SeatID, ID, OwnerKind, OwnerID string
	Lifetime                       Lifetime
	ExpiresAt                      time.Time
	Version                        uint64
}
type RecordData struct {
	Binding    Binding
	Ciphertext []byte
	Revoked    bool
}
type Record = auth.Secret[RecordData]

func StoredRecord(v RecordData) Record {
	v.Ciphertext = slices.Clone(v.Ciphertext)
	return auth.RoomSecret(v)
}
func RecordValue(v Record) RecordData {
	d := v.StorageValue()
	d.Ciphertext = slices.Clone(d.Ciphertext)
	return d
}

type Key struct{ data **keyData }
type keyData struct {
	mu    sync.RWMutex
	bytes []byte
}

func (Key) Format(s fmt.State, _ rune)   { _, _ = io.WriteString(s, "<provider credential>") }
func (Key) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func NewKey(raw []byte) (Key, error) {
	if len(raw) < 1 || len(raw) > MaxKeyBytes {
		return Key{}, auth.ErrInvalid
	}
	for _, b := range raw {
		if b < 33 || b > 126 {
			return Key{}, auth.ErrInvalid
		}
	}
	d := &keyData{bytes: slices.Clone(raw)}
	return Key{data: &d}, nil
}

// Use supplies an owned copy to the trusted adapter and clears it on return.
// The adapter must not retain it or include it in errors, prompts or logs.
func (k Key) Use(f func([]byte) error) error {
	if k.data == nil || *k.data == nil || f == nil {
		return auth.ErrInvalid
	}
	d := *k.data
	d.mu.RLock()
	raw := slices.Clone(d.bytes)
	d.mu.RUnlock()
	if len(raw) == 0 {
		return auth.ErrDenied
	}
	defer clear(raw)
	return auth.SafeError(f(raw))
}

func (k Key) Close() {
	if k.data != nil && *k.data != nil {
		d := *k.data
		d.mu.Lock()
		clear(d.bytes)
		d.bytes = nil
		d.mu.Unlock()
	}
}

type Vault struct{ data **vaultData }
type vaultData struct {
	mu       sync.RWMutex
	gcm      cipher.AEAD
	retained bool
}

func (*Vault) Format(s fmt.State, _ rune)   { _, _ = io.WriteString(s, "<credential vault>") }
func (*Vault) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (v *Vault) state() *vaultData {
	if v == nil || v.data == nil {
		return nil
	}
	return *v.data
}

// An absent file explicitly selects restart-ephemeral, one-game credentials.
// A retained vault accepts exactly 32 raw bytes from a read-only secret file.
func New(masterKeyFile string) (*Vault, error) {
	var raw []byte
	retained := masterKeyFile != ""
	if retained {
		s, e := auth.ReadSecretFile(masterKeyFile, 32)
		if e != nil {
			return nil, auth.SafeError(e)
		}
		raw = s.StorageValue()
	} else {
		raw = make([]byte, 32)
		if _, e := rand.Read(raw); e != nil {
			return nil, auth.ErrUnavailable
		}
	}
	defer clear(raw)
	if len(raw) != 32 {
		return nil, auth.ErrInvalid
	}
	block, e := aes.NewCipher(raw)
	if e != nil {
		return nil, auth.ErrUnavailable
	}
	gcm, e := cipher.NewGCM(block)
	if e != nil {
		return nil, auth.ErrUnavailable
	}
	d := &vaultData{gcm: gcm, retained: retained}
	return &Vault{data: &d}, nil
}
func (v *Vault) Close() {
	if d := v.state(); d != nil {
		d.mu.Lock()
		d.gcm = nil
		d.mu.Unlock()
	}
}
func ValidBinding(b Binding) bool {
	if !data.ValidID(b.Scope.WorkspaceID) || !data.ValidID(b.Scope.RoomID) || !data.ValidID(b.Scope.GameID) || !data.ValidID(b.SeatID) || !data.ValidID(b.ID) || !data.ValidID(b.OwnerID) || b.Version < 1 || b.Version > 1<<53 {
		return false
	}
	if b.OwnerKind != "account" && b.OwnerKind != "guest" {
		return false
	}
	switch b.Lifetime {
	case Temporary:
		return !b.ExpiresAt.IsZero() && b.ExpiresAt.Equal(b.ExpiresAt.UTC().Truncate(time.Microsecond))
	case Retained:
		return b.OwnerKind == "account" && b.ExpiresAt.IsZero()
	default:
		return false
	}
}
func aad(b Binding) []byte {
	raw, _ := json.Marshal(b)
	return append([]byte("platform-model-credential/v1\x00"), raw...)
}
func (v *Vault) Seal(ctx context.Context, b Binding, k Key, now time.Time) (Record, error) {
	d := v.state()
	if d == nil || ctx == nil || ctx.Err() != nil || !ValidBinding(b) || now.IsZero() {
		return Record{}, auth.ErrInvalid
	}
	if b.Lifetime == Temporary && (!now.Before(b.ExpiresAt) || b.ExpiresAt.Sub(now) > MaxTemporaryLifetime) {
		return Record{}, auth.ErrDenied
	}
	d.mu.RLock()
	defer d.mu.RUnlock()
	if d.gcm == nil {
		return Record{}, auth.ErrUnavailable
	}
	if b.Lifetime == Retained && !d.retained {
		return Record{}, auth.ErrDenied
	}
	nonce := make([]byte, d.gcm.NonceSize())
	if _, e := rand.Read(nonce); e != nil {
		return Record{}, auth.ErrUnavailable
	}
	var sealed []byte
	e := k.Use(func(raw []byte) error { sealed = d.gcm.Seal(nonce, nonce, raw, aad(b)); return nil })
	if e != nil {
		return Record{}, e
	}
	return StoredRecord(RecordData{Binding: b, Ciphertext: sealed}), nil
}
func (v *Vault) Open(ctx context.Context, r Record, expected Binding, now time.Time) (Key, error) {
	d := v.state()
	x := RecordValue(r)
	if d == nil || ctx == nil || ctx.Err() != nil || !ValidBinding(expected) || x.Binding != expected || x.Revoked || now.IsZero() {
		return Key{}, auth.ErrDenied
	}
	if expected.Lifetime == Temporary && !now.Before(expected.ExpiresAt) {
		return Key{}, auth.ErrDenied
	}
	d.mu.RLock()
	defer d.mu.RUnlock()
	if d.gcm == nil {
		return Key{}, auth.ErrUnavailable
	}
	if expected.Lifetime == Retained && !d.retained {
		return Key{}, auth.ErrDenied
	}
	n := d.gcm.NonceSize()
	if len(x.Ciphertext) < n+d.gcm.Overhead()+1 || len(x.Ciphertext) > n+d.gcm.Overhead()+MaxKeyBytes {
		return Key{}, auth.ErrDenied
	}
	raw, e := d.gcm.Open(nil, x.Ciphertext[:n], x.Ciphertext[n:], aad(expected))
	if e != nil {
		return Key{}, auth.ErrDenied
	}
	defer clear(raw)
	key, e := NewKey(raw)
	if e != nil {
		return Key{}, auth.ErrDenied
	}
	return key, nil
}
