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
		if strings.ContainsAny(segment, `<>"|?*`) {
			return fmt.Errorf("package path segment contains a Windows-forbidden character")
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
	return norm.NFC.String(cases.Fold().String(norm.NFC.String(value)))
}

func windowsDeviceName(segment string) bool {
	base := segment
	if index := strings.IndexByte(base, '.'); index >= 0 {
		base = base[:index]
	}
	base = strings.TrimRight(base, " .")
	base = strings.ToUpper(base)
	switch base {
	case "CON", "PRN", "AUX", "NUL", "CLOCK$", "CONIN$", "CONOUT$":
		return true
	}
	if len(base) == 4 && (strings.HasPrefix(base, "COM") || strings.HasPrefix(base, "LPT")) {
		return base[3] >= '1' && base[3] <= '9'
	}
	if len([]rune(base)) == 4 && (strings.HasPrefix(base, "COM") || strings.HasPrefix(base, "LPT")) {
		last := []rune(base)[3]
		return last == '\u00b9' || last == '\u00b2' || last == '\u00b3'
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

// TreeSet extends Set's portable collision checks with the archive invariant
// that a regular file cannot also be an ancestor of another regular file.
// Comparisons use the same full Unicode fold as CollisionKey, so the tree has
// identical semantics on every supported host.
type TreeSet struct {
	root treeNode
}

type treeNode struct {
	spelling  string
	path      string
	regular   bool
	directory bool
	children  map[string]*treeNode
}

// Add is the regular-file shorthand used by archive entry sets.
func (set *TreeSet) Add(value string) error {
	return set.AddFile(value)
}

// AddFile adds one regular file. It rejects duplicate files, directory/file
// aliases, ancestors, descendants, and folded spelling changes at every path
// component.
func (set *TreeSet) AddFile(value string) error {
	return set.add(value, true)
}

// AddDirectory adds one explicit directory. Repeating the same canonical
// directory is harmless, while aliases and file/directory conflicts fail.
func (set *TreeSet) AddDirectory(value string) error {
	return set.add(value, false)
}

func (set *TreeSet) add(value string, regular bool) error {
	if err := Validate(value); err != nil {
		return fmt.Errorf("%q: %w", value, err)
	}
	node := &set.root
	original := strings.Split(value, "/")
	folded := strings.Split(CollisionKey(value), "/")
	for index, segment := range folded {
		if node.regular {
			return fmt.Errorf("package path %q descends from regular file %q", value, node.path)
		}
		if node.children == nil {
			node.children = make(map[string]*treeNode)
		}
		child, exists := node.children[segment]
		if !exists {
			child = &treeNode{spelling: original[index], path: strings.Join(original[:index+1], "/")}
			node.children[segment] = child
		} else if child.spelling != original[index] {
			return fmt.Errorf("package path %q changes portable spelling of %q", value, child.path)
		}
		node = child
		if index < len(folded)-1 {
			node.directory = true
			continue
		}
		if regular {
			if node.regular {
				return fmt.Errorf("package path %q collides with %q", value, node.path)
			}
			if node.directory || len(node.children) != 0 {
				return fmt.Errorf("package path %q conflicts with a directory", value)
			}
			node.regular = true
		} else {
			if node.regular {
				return fmt.Errorf("package directory %q conflicts with regular file %q", value, node.path)
			}
			node.directory = true
		}
	}
	return nil
}
