// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package task dispatches committed external intents. Workers own operational
// leases and results; only a new authenticated Actor command changes game state.
package task

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
)

var (
	ErrDenied      = errors.New("TASK_DENIED")
	ErrInvalid     = errors.New("TASK_INVALID")
	ErrUnavailable = errors.New("TASK_UNAVAILABLE")
	ErrNotFound    = errors.New("TASK_NOT_FOUND")
	ErrBusy        = errors.New("TASK_BACKPRESSURE")
	ErrExpired     = errors.New("TASK_EXPIRED")
	ErrCancelled   = errors.New("TASK_CANCELLED")
	ErrStale       = errors.New("TASK_STALE_VERSION")
	ErrFailed      = errors.New("TASK_FAILED")
)

// SafeError never propagates worker/provider/driver diagnostics. Those may
// contain the protected request, result, connection string or provider key.
func SafeError(err error) error {
	if err == nil {
		return nil
	}
	if errors.Is(err, context.Canceled) {
		return context.Canceled
	}
	if errors.Is(err, context.DeadlineExceeded) {
		return context.DeadlineExceeded
	}
	for _, known := range []error{ErrDenied, ErrInvalid, ErrUnavailable, ErrNotFound, ErrBusy, ErrExpired, ErrCancelled, ErrStale, ErrFailed} {
		if errors.Is(err, known) {
			return known
		}
	}
	return ErrUnavailable
}

type Value struct{ data **valueData }
type valueData struct{ raw []byte }

func (Value) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<private task value>") }
func (Value) MarshalJSON() ([]byte, error) { return nil, ErrDenied }
func (v Value) state() *valueData {
	if v.data == nil {
		return nil
	}
	return *v.data
}
func NewValue(v checkpoint.Value) (Value, error) {
	if checkpoint.Validate(v) != nil {
		return Value{}, ErrInvalid
	}
	raw, err := json.Marshal(v)
	if err != nil || len(raw) > 256<<10 {
		return Value{}, ErrInvalid
	}
	d := &valueData{raw: raw}
	return Value{data: &d}, nil
}

// StorageValue is an explicit trusted composition boundary, returning a copy.
func (v Value) StorageValue() (checkpoint.Value, error) {
	var out checkpoint.Value
	if v.state() == nil || checkpoint.StrictDecode(v.state().raw, &out, 256<<10) != nil || checkpoint.Validate(out) != nil {
		return out, ErrDenied
	}
	return out, nil
}
func (v Value) Digest() string {
	if v.state() == nil {
		return ""
	}
	x := sha256.Sum256(v.state().raw)
	return hex.EncodeToString(x[:])
}
func (v Value) Bytes() int {
	if v.state() == nil {
		return 0
	}
	return len(v.state().raw)
}

// A lease token is never persisted or serialized. Storage receives only its
// digest and compares it with the current tenant-bound operational lease.
type Token struct{ data **tokenData }
type tokenData struct{ key [32]byte }

func (Token) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<private task lease>") }
func (Token) MarshalJSON() ([]byte, error) { return nil, ErrDenied }
func (t Token) state() *tokenData {
	if t.data == nil {
		return nil
	}
	return *t.data
}
func NewToken() (Token, error) {
	d := &tokenData{}
	if _, err := rand.Read(d.key[:]); err != nil {
		return Token{}, ErrUnavailable
	}
	return Token{data: &d}, nil
}
func (t Token) Digest() string {
	if t.state() == nil {
		return ""
	}
	h := sha256.Sum256(t.state().key[:])
	return hex.EncodeToString(h[:])
}

type Inputs struct{ data **inputsData }
type inputsData struct {
	time   int64
	random []int64
}

func (Inputs) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<recorded task inputs>") }
func (Inputs) MarshalJSON() ([]byte, error) { return nil, ErrDenied }
func (i Inputs) state() *inputsData {
	if i.data == nil {
		return nil
	}
	return *i.data
}
func NewInputs(at int64, random []int64) (Inputs, error) {
	if at < 0 || len(random) > 256 {
		return Inputs{}, ErrInvalid
	}
	for _, n := range random {
		if n < 0 {
			return Inputs{}, ErrInvalid
		}
	}
	d := &inputsData{time: at, random: append([]int64(nil), random...)}
	return Inputs{data: &d}, nil
}
func (i Inputs) StorageValue() (int64, []int64, error) {
	if i.state() == nil {
		return 0, nil, ErrDenied
	}
	return i.state().time, append([]int64(nil), i.state().random...), nil
}
