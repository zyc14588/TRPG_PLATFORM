// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package checkpoint defines the data-only reconstruction boundary. It never
// serializes a VM, function, coroutine, userdata or capability handle.
package checkpoint

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math"
	"reflect"
	"strconv"
	"strings"
	"unicode/utf8"
)

const (
	MaxBytes = 256 << 10
	MaxDepth = 32
	MaxNodes = 8192
)

var ErrRejected = errors.New("CHECKPOINT_REJECTED")

// Value preserves Lua integers without passing through JSON float64. Empty Lua
// tables use object form; arrays must be dense, one-based and homogeneous in key type.
type Value struct {
	Kind    string           `json:"kind"`
	Boolean bool             `json:"boolean,omitempty"`
	Number  string           `json:"number,omitempty"`
	String  string           `json:"string,omitempty"`
	Array   []Value          `json:"array,omitempty"`
	Table   map[string]Value `json:"table,omitempty"`
}

func Bool(b bool) Value               { return Value{Kind: "boolean", Boolean: b} }
func Int(n int64) Value               { return Value{Kind: "integer", Number: strconv.FormatInt(n, 10)} }
func Text(s string) Value             { return Value{Kind: "string", String: s} }
func Array(v ...Value) Value          { return Value{Kind: "array", Array: v} }
func Object(v map[string]Value) Value { return Value{Kind: "table", Table: v} }

func Validate(v Value) error {
	nodes, size := 0, 0
	var walk func(Value, int) error
	walk = func(v Value, depth int) error {
		nodes++
		size += len(v.Kind) + len(v.Number) + len(v.String) + 32
		if depth > MaxDepth || nodes > MaxNodes || size > MaxBytes {
			return ErrRejected
		}
		if (v.Boolean && v.Kind != "boolean") || (v.Number != "" && v.Kind != "integer" && v.Kind != "float") || (v.String != "" && v.Kind != "string") || (v.Array != nil && v.Kind != "array") || (v.Table != nil && v.Kind != "table") {
			return ErrRejected
		}
		switch v.Kind {
		case "nil", "boolean":
		case "integer":
			n, err := strconv.ParseInt(v.Number, 10, 64)
			if err != nil || strconv.FormatInt(n, 10) != v.Number {
				return ErrRejected
			}
		case "float":
			n, err := strconv.ParseFloat(v.Number, 64)
			if err != nil || math.IsInf(n, 0) || math.IsNaN(n) || math.Abs(n) > 1<<53 || strconv.FormatFloat(n, 'g', -1, 64) != v.Number {
				return ErrRejected
			}
		case "string":
			if !utf8.ValidString(v.String) || strings.HasPrefix(v.String, "cap:") {
				return ErrRejected
			}
		case "array":
			for _, x := range v.Array {
				if err := walk(x, depth+1); err != nil {
					return err
				}
			}
		case "table":
			for k, x := range v.Table {
				size += len(k)
				if !utf8.ValidString(k) || strings.HasPrefix(k, "cap:") {
					return ErrRejected
				}
				if err := walk(x, depth+1); err != nil {
					return err
				}
			}
		default:
			return ErrRejected
		}
		return nil
	}
	return walk(v, 0)
}

type Binding struct {
	SessionID      string            `json:"session_id"`
	StateVersion   uint64            `json:"state_version"`
	PackageHashes  map[string]string `json:"package_hashes"`
	DependencyLock string            `json:"dependency_lock"`
	LuaProfile     string            `json:"lua_profile"`
	RuntimeVersion string            `json:"runtime_version"`
}

func IsDigest(s string) bool {
	if len(s) != 71 || !strings.HasPrefix(s, "sha256:") || strings.ToLower(s) != s {
		return false
	}
	_, err := hex.DecodeString(s[7:])
	return err == nil
}

func (b Binding) Validate() error {
	if len(b.SessionID) == 0 || len(b.SessionID) > 128 || !utf8.ValidString(b.SessionID) || len(b.PackageHashes) == 0 || len(b.PackageHashes) > 256 || !IsDigest(b.DependencyLock) || b.LuaProfile == "" || len(b.LuaProfile) > 128 || !utf8.ValidString(b.LuaProfile) || b.RuntimeVersion == "" || len(b.RuntimeVersion) > 128 || !utf8.ValidString(b.RuntimeVersion) {
		return ErrRejected
	}
	for id, h := range b.PackageHashes {
		if id == "" || len(id) > 256 || !utf8.ValidString(id) || !IsDigest(h) {
			return ErrRejected
		}
	}
	return nil
}

func (b Binding) Equal(other Binding) bool { return reflect.DeepEqual(b, other) }

type Checkpoint struct {
	Binding Binding `json:"binding"`
	State   Value   `json:"state"`
	Digest  string  `json:"digest"`
}

func Hash(data []byte) string {
	sum := sha256.Sum256(data)
	return "sha256:" + hex.EncodeToString(sum[:])
}

func Seal(binding Binding, state Value) (Checkpoint, error) {
	if err := binding.Validate(); err != nil {
		return Checkpoint{}, err
	}
	if err := Validate(state); err != nil {
		return Checkpoint{}, err
	}
	c := Checkpoint{Binding: binding, State: state}
	raw, err := json.Marshal(c)
	if err != nil || len(raw)+71 > MaxBytes {
		return Checkpoint{}, ErrRejected
	}
	// Decode our canonical bytes to break every caller-owned map/slice alias.
	if err = json.Unmarshal(raw, &c); err != nil {
		return Checkpoint{}, err
	}
	c.Digest = Hash(raw)
	return c, nil
}

func Encode(c Checkpoint) ([]byte, error) {
	if err := c.Binding.Validate(); err != nil {
		return nil, err
	}
	if err := Validate(c.State); err != nil {
		return nil, err
	}
	raw, err := json.Marshal(c)
	if err != nil || len(raw) > MaxBytes {
		return nil, ErrRejected
	}
	return raw, nil
}

func Decode(raw []byte, expected Binding) (Checkpoint, error) {
	var c Checkpoint
	if err := StrictDecode(raw, &c, MaxBytes); err != nil {
		return c, err
	}
	if !c.Binding.Equal(expected) {
		return c, fmt.Errorf("%w: binding mismatch", ErrRejected)
	}
	sealed, err := Seal(c.Binding, c.State)
	if err != nil {
		return c, err
	}
	if sealed.Digest != c.Digest {
		return c, fmt.Errorf("%w: digest mismatch", ErrRejected)
	}
	return sealed, nil
}

// StrictDecode rejects duplicate keys and trailing values before typed decoding.
// This is also used for bounded IPC envelopes, which cannot smuggle overrides.
func StrictDecode(raw []byte, target any, limit int) error {
	if len(raw) == 0 || len(raw) > limit || !utf8.Valid(raw) || !validUnicodeEscapes(raw) {
		return ErrRejected
	}
	d := json.NewDecoder(bytes.NewReader(raw))
	d.UseNumber()
	var scan func(int) error
	scan = func(depth int) error {
		// Each Value adds an object and an array/map wrapper to the JSON depth.
		if depth > 2*MaxDepth+8 {
			return ErrRejected
		}
		token, err := d.Token()
		if err != nil {
			return err
		}
		delim, ok := token.(json.Delim)
		if !ok {
			return nil
		}
		switch delim {
		case '{':
			seen := map[string]bool{}
			for d.More() {
				key, err := d.Token()
				if err != nil {
					return err
				}
				s, ok := key.(string)
				if !ok || seen[s] {
					return ErrRejected
				}
				seen[s] = true
				if err := scan(depth + 1); err != nil {
					return err
				}
			}
		case '[':
			for d.More() {
				if err := scan(depth + 1); err != nil {
					return err
				}
			}
		default:
			return ErrRejected
		}
		_, err = d.Token()
		return err
	}
	if err := scan(0); err != nil {
		return fmt.Errorf("%w: invalid JSON", ErrRejected)
	}
	if _, err := d.Token(); err != io.EOF {
		return ErrRejected
	}
	d = json.NewDecoder(bytes.NewReader(raw))
	d.DisallowUnknownFields()
	if err := d.Decode(target); err != nil {
		return fmt.Errorf("%w: invalid fields", ErrRejected)
	}
	return nil
}

// encoding/json replaces unpaired UTF-16 surrogate escapes with U+FFFD. Reject
// them instead of accepting a different checkpoint string from the wire input.
func validUnicodeEscapes(raw []byte) bool {
	inString := false
	for i := 0; i < len(raw); i++ {
		if raw[i] == '"' {
			inString = !inString
			continue
		}
		if !inString || raw[i] != '\\' {
			continue
		}
		i++
		if i >= len(raw) {
			return false
		}
		if raw[i] != 'u' {
			continue
		}
		if i+4 >= len(raw) {
			return false
		}
		r, err := strconv.ParseUint(string(raw[i+1:i+5]), 16, 16)
		if err != nil {
			return false
		}
		i += 4
		if r >= 0xdc00 && r <= 0xdfff {
			return false
		}
		if r >= 0xd800 && r <= 0xdbff {
			if i+6 >= len(raw) || raw[i+1] != '\\' || raw[i+2] != 'u' {
				return false
			}
			low, err := strconv.ParseUint(string(raw[i+3:i+7]), 16, 16)
			if err != nil || low < 0xdc00 || low > 0xdfff {
				return false
			}
			i += 6
		}
	}
	return true
}
