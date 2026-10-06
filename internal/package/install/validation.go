// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package install

import (
	"bytes"
	"encoding/json"
	"fmt"
	"path"
	"strings"
	"unicode/utf8"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
)

func validateGraph(items []staged) error {
	lock := items[0].pkg.ExactLock()
	if len(lock.Packages()) != len(items) {
		return fmt.Errorf("exact dependency artifacts are missing or extra")
	}
	seen := map[string]bool{}
	for _, item := range items {
		d, err := item.pkg.Manifest()
		if err != nil {
			return err
		}
		id := string(d.Package.PackageID)
		if seen[id] {
			return fmt.Errorf("duplicate dependency artifact")
		}
		seen[id] = true
		node, ok := lock.Package(d.Package.PackageID)
		if !ok || node.ContentHash != item.pkg.ContentHash() || node.Version != d.Package.Version {
			return fmt.Errorf("dependency identity differs from exact root lock")
		}
		// Each standalone artifact must prove the identical reachable subgraph,
		// including features and transitive edges, not just the root version/hash.
		for _, sub := range item.pkg.ExactLock().Packages() {
			outer, exists := lock.Package(sub.PackageID)
			a, _ := json.Marshal(sub)
			b, _ := json.Marshal(outer)
			if !exists || !bytes.Equal(a, b) {
				return fmt.Errorf("dependency lock subgraph differs")
			}
		}
	}
	return nil
}

func validateContent(pkg *archive.Package) error {
	for _, entry := range pkg.Entries() {
		name, data := entry.Path(), entry.Bytes()
		ext := strings.ToLower(path.Ext(name))
		// There is no native/bytecode declaration in the current package schema.
		// Reject executable encodings under any extension, including asset names.
		if bytes.HasPrefix(data, []byte("\x1bLua")) || bytes.HasPrefix(data, []byte("\x1bLJ")) || bytes.HasPrefix(data, []byte("\x7fELF")) || bytes.HasPrefix(data, []byte("MZ")) || bytes.HasPrefix(data, []byte("\x00asm")) || bytes.HasPrefix(data, []byte{0xfe, 0xed, 0xfa, 0xce}) || bytes.HasPrefix(data, []byte{0xce, 0xfa, 0xed, 0xfe}) || bytes.HasPrefix(data, []byte{0xfe, 0xed, 0xfa, 0xcf}) || bytes.HasPrefix(data, []byte{0xcf, 0xfa, 0xed, 0xfe}) || bytes.HasPrefix(data, []byte{0xca, 0xfe, 0xba, 0xbe}) {
			return fmt.Errorf("executable/bytecode entry rejected")
		}
		switch ext {
		case ".exe", ".dll", ".so", ".dylib", ".wasm", ".luac", ".o", ".a":
			return fmt.Errorf("undeclared executable entry rejected")
		}
		if ext == ".lua" {
			if path.Ext(name) != ".lua" || profile.ValidateSource(data) != nil {
				return profile.Fail(profile.ErrSource)
			}
		} else if !utf8.Valid(data) || bytes.IndexByte(data, 0) >= 0 {
			// The closed manifest has no arbitrary binary declaration. Known inert
			// image/audio encodings can be carried as assets; native code never can.
			if !inertMedia(ext, data) {
				return fmt.Errorf("undeclared binary entry rejected")
			}
		}
	}
	return nil
}
func inertMedia(ext string, b []byte) bool {
	switch ext {
	case ".png":
		return bytes.HasPrefix(b, []byte("\x89PNG\r\n\x1a\n"))
	case ".jpg", ".jpeg":
		return bytes.HasPrefix(b, []byte{0xff, 0xd8, 0xff})
	case ".gif":
		return bytes.HasPrefix(b, []byte("GIF87a")) || bytes.HasPrefix(b, []byte("GIF89a"))
	case ".ogg":
		return bytes.HasPrefix(b, []byte("OggS"))
	case ".wav":
		return len(b) >= 12 && string(b[:4]) == "RIFF" && string(b[8:12]) == "WAVE"
	case ".webp":
		return len(b) >= 12 && string(b[:4]) == "RIFF" && string(b[8:12]) == "WEBP"
	}
	return false
}
