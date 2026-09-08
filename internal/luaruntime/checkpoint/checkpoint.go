// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package checkpoint implements a canonical, validated checkpoint envelope.
// It serializes explicit data only; it never serializes VM memory.
package checkpoint

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"reflect"
	"sort"
	"strings"
	"unicode/utf8"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile/identity"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	packagemodel "github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

const (
	FormatVersion   = 1
	MaxDocumentSize = 2 << 20
)

var (
	ErrInvalidBinding      = errors.New("checkpoint binding is invalid")
	ErrIncompatibleBinding = errors.New("checkpoint binding is incompatible")
	ErrCorrupt             = errors.New("checkpoint is corrupt")
	ErrOversized           = errors.New("checkpoint payload is oversized")
)

type PackageHash struct {
	PackageID string `json:"package_id"`
	SHA256    string `json:"sha256"`
}

// Binding ties checkpoint data to every compatibility identity needed for a
// safe reconstruction. SessionID additionally prevents cross-VM use.
type Binding struct {
	SessionID      string        `json:"session_id"`
	StateVersion   uint64        `json:"state_version"`
	PackageHashes  []PackageHash `json:"package_hashes"`
	DependencyLock string        `json:"dependency_lock"`
	LuaProfile     string        `json:"lua_profile"`
	RuntimeVersion string        `json:"runtime_version"`
}

type payload struct {
	FormatVersion int     `json:"format_version"`
	Binding       Binding `json:"binding"`
	State         Value   `json:"state"`
}

type document struct {
	Payload       payload `json:"payload"`
	PayloadSHA256 string  `json:"payload_sha256"`
}

// Decoded is the verified result of opening a checkpoint.
type Decoded struct {
	Binding Binding
	State   Value
}

func normalizeBinding(binding Binding) (Binding, error) {
	if binding.SessionID == "" || !utf8.ValidString(binding.SessionID) || binding.StateVersion == 0 ||
		binding.DependencyLock == "" || !utf8.ValidString(binding.DependencyLock) ||
		binding.LuaProfile == "" || !utf8.ValidString(binding.LuaProfile) ||
		binding.RuntimeVersion == "" || !utf8.ValidString(binding.RuntimeVersion) ||
		len(binding.PackageHashes) == 0 {
		return Binding{}, ErrInvalidBinding
	}
	if len(binding.SessionID) > 256 || len(binding.DependencyLock) > MaxStringBytes || len(binding.LuaProfile) > 256 || len(binding.RuntimeVersion) > 256 {
		return Binding{}, ErrInvalidBinding
	}
	if !identity.Supported(binding.LuaProfile, binding.RuntimeVersion) {
		return Binding{}, fmt.Errorf("%w: unsupported Lua profile/runtime", ErrInvalidBinding)
	}
	exactLock, err := dependency.ParseExactLock([]byte(binding.DependencyLock))
	if err != nil {
		return Binding{}, fmt.Errorf("%w: dependency lock: %v", ErrInvalidBinding, err)
	}
	canonicalLock, err := exactLock.CanonicalJSON()
	if err != nil {
		return Binding{}, fmt.Errorf("%w: dependency lock: %v", ErrInvalidBinding, err)
	}
	if len(canonicalLock) > MaxStringBytes {
		return Binding{}, fmt.Errorf("%w: canonical dependency lock is oversized", ErrInvalidBinding)
	}

	result := binding
	result.DependencyLock = string(canonicalLock)
	result.PackageHashes = append([]PackageHash(nil), binding.PackageHashes...)
	seen := make(map[string]struct{}, len(result.PackageHashes))
	for i, item := range result.PackageHashes {
		packageID, err := packagemodel.ParsePackageID(item.PackageID)
		if err != nil || len(item.SHA256) != sha256.Size*2 {
			return Binding{}, fmt.Errorf("%w: package hash %d", ErrInvalidBinding, i)
		}
		item.PackageID = packageID.String()
		if item.SHA256 != strings.ToLower(item.SHA256) {
			return Binding{}, fmt.Errorf("%w: package hash %d is not canonical", ErrInvalidBinding, i)
		}
		if _, err := hex.DecodeString(item.SHA256); err != nil {
			return Binding{}, fmt.Errorf("%w: package hash %d: %v", ErrInvalidBinding, i, err)
		}
		if _, exists := seen[item.PackageID]; exists {
			return Binding{}, fmt.Errorf("%w: duplicate package %q", ErrInvalidBinding, item.PackageID)
		}
		seen[item.PackageID] = struct{}{}
		result.PackageHashes[i] = item
	}
	sort.Slice(result.PackageHashes, func(i, j int) bool { return result.PackageHashes[i].PackageID < result.PackageHashes[j].PackageID })
	return result, nil
}

// NewBinding validates and canonicalizes compatibility metadata.
func NewBinding(binding Binding) (Binding, error) {
	return normalizeBinding(binding)
}

// Marshal creates deterministic canonical JSON with an integrity checksum.
func Marshal(binding Binding, state any) ([]byte, error) {
	canonicalBinding, err := normalizeBinding(binding)
	if err != nil {
		return nil, err
	}
	value, err := FromGo(state)
	if err != nil {
		return nil, err
	}
	value, err = Normalize(value)
	if err != nil {
		return nil, err
	}
	p := payload{FormatVersion: FormatVersion, Binding: canonicalBinding, State: value}
	payloadBytes, err := json.Marshal(p)
	if err != nil {
		return nil, err
	}
	digest := sha256.Sum256(payloadBytes)
	encoded, err := json.Marshal(document{Payload: p, PayloadSHA256: hex.EncodeToString(digest[:])})
	if err != nil {
		return nil, err
	}
	if len(encoded) > MaxDocumentSize {
		return nil, ErrOversized
	}
	return encoded, nil
}

// Unmarshal verifies syntax, canonical value constraints, integrity and the
// complete expected binding before returning any state.
func Unmarshal(encoded []byte, expected Binding) (Decoded, error) {
	if len(encoded) == 0 {
		return Decoded{}, ErrCorrupt
	}
	if len(encoded) > MaxDocumentSize {
		return Decoded{}, ErrOversized
	}
	var doc document
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&doc); err != nil {
		return Decoded{}, fmt.Errorf("%w: %v", ErrCorrupt, err)
	}
	if err := requireEOF(decoder); err != nil {
		return Decoded{}, err
	}
	canonicalDocument, err := json.Marshal(doc)
	if err != nil || !bytes.Equal(encoded, canonicalDocument) {
		return Decoded{}, fmt.Errorf("%w: noncanonical JSON", ErrCorrupt)
	}
	if doc.Payload.FormatVersion != FormatVersion {
		return Decoded{}, fmt.Errorf("%w: format version %d", ErrCorrupt, doc.Payload.FormatVersion)
	}

	canonicalBinding, err := normalizeBinding(doc.Payload.Binding)
	if err != nil || !reflect.DeepEqual(canonicalBinding, doc.Payload.Binding) {
		return Decoded{}, fmt.Errorf("%w: noncanonical binding", ErrCorrupt)
	}
	canonicalState, err := Normalize(doc.Payload.State)
	if err != nil || !reflect.DeepEqual(canonicalState, doc.Payload.State) {
		return Decoded{}, fmt.Errorf("%w: invalid state", ErrCorrupt)
	}
	payloadBytes, err := json.Marshal(doc.Payload)
	if err != nil {
		return Decoded{}, fmt.Errorf("%w: %v", ErrCorrupt, err)
	}
	digest := sha256.Sum256(payloadBytes)
	if doc.PayloadSHA256 != hex.EncodeToString(digest[:]) {
		return Decoded{}, ErrCorrupt
	}

	canonicalExpected, err := normalizeBinding(expected)
	if err != nil {
		return Decoded{}, err
	}
	if !reflect.DeepEqual(doc.Payload.Binding, canonicalExpected) {
		return Decoded{}, ErrIncompatibleBinding
	}
	return Decoded{Binding: doc.Payload.Binding, State: doc.Payload.State}, nil
}

func requireEOF(decoder *json.Decoder) error {
	var extra any
	if err := decoder.Decode(&extra); !errors.Is(err, io.EOF) {
		if err == nil {
			return ErrCorrupt
		}
		return fmt.Errorf("%w: %v", ErrCorrupt, err)
	}
	return nil
}
