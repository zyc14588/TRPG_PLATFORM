// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package packagepath validates portable, package-local paths before they are
// used by a project directory or archive reader.
package packagepath

import (
	"fmt"
	"path"
	"strings"
	"unicode"
	"unicode/utf8"

	"golang.org/x/text/cases"
	"golang.org/x/text/unicode/norm"
)

const MaxBytes = 240

// Validate rejects paths whose spelling can change across supported hosts.
func Validate(value string) error {
	if value == "" || len(value) > MaxBytes {
		return fmt.Errorf("package path must contain 1..%d bytes", MaxBytes)
	}
	if !utf8.ValidString(value) {
		return fmt.Errorf("package path is not valid UTF-8")
	}
	if !norm.NFC.IsNormalString(value) {
		return fmt.Errorf("package path is not NFC-normalized")
	}
	if strings.HasPrefix(value, "/") || path.IsAbs(value) {
		return fmt.Errorf("package path must be relative")
	}
	if strings.ContainsAny(value, "\\:\x00") {
		return fmt.Errorf("package path contains a forbidden separator, drive marker, or NUL")
	}
	for _, segment := range strings.Split(value, "/") {
		if segment == "" || segment == "." || segment == ".." {
			return fmt.Errorf("package path contains an empty, dot, or parent segment")
		}
		if strings.TrimSpace(segment) != segment {
			return fmt.Errorf("package path segment has leading or trailing whitespace")
		}
		if strings.HasSuffix(segment, ".") {
			return fmt.Errorf("package path segment has a trailing dot")
		}
		if windowsDeviceName(segment) {
			return fmt.Errorf("package path segment is a reserved device name")
		}
		for _, character := range segment {
			if unicode.IsControl(character) {
				return fmt.Errorf("package path contains a control character")
			}
		}
	}
	if cleaned := path.Clean(value); cleaned != value {
		return fmt.Errorf("package path is not canonical")
	}
	return nil
}

// CollisionKey is the portable comparison form. Inputs must first pass
// Validate. Unicode lower-casing intentionally makes case-only names collide.
func CollisionKey(value string) string {
	return cases.Fold().String(norm.NFC.String(value))
}

func windowsDeviceName(segment string) bool {
	base := segment
	if index := strings.IndexByte(base, '.'); index >= 0 {
		base = base[:index]
	}
	base = strings.ToUpper(base)
	switch base {
	case "CON", "PRN", "AUX", "NUL", "CLOCK$":
		return true
	}
	if len(base) == 4 && (strings.HasPrefix(base, "COM") || strings.HasPrefix(base, "LPT")) {
		return base[3] >= '1' && base[3] <= '9'
	}
	return false
}

// Set validates paths and rejects byte-identical, case-folded, and
// normalization-equivalent collisions.
type Set struct {
	seen map[string]string
}

func (set *Set) Add(value string) error {
	if err := Validate(value); err != nil {
		return fmt.Errorf("%q: %w", value, err)
	}
	if set.seen == nil {
		set.seen = make(map[string]string)
	}
	key := CollisionKey(value)
	if previous, exists := set.seen[key]; exists {
		return fmt.Errorf("package path %q collides with %q", value, previous)
	}
	set.seen[key] = value
	return nil
}
