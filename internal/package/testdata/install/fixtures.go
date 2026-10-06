// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package fixtures provides deterministic package inputs for installation tests.
package fixtures

import (
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/manifest"
)

func Files(id, kind, source string) map[string][]byte {
	text := fmt.Sprintf(`schema_version = 1
artifact_type = "package"
package_id = %q
package_kind = %q
version = "1.0.0"
display_name = "Installation Fixture"
`, id, kind)
	if source != "" {
		text += `entrypoint = "lua/main.lua"
lua_profile = "platform-lua-5.5-p1"
[host_api]
major = 1
min_minor = 0
max_minor = 0
`
	}
	text += `[build]
source = "fixture-local"
revision = "fixture-v1"
builder = "install-tests/1"
[rights]
authors = ["Fixture Author"]
source = "original"
license_expression = "MIT"
[capabilities]
required = []
`
	files := map[string][]byte{archive.ManifestPath: []byte(text), "content/readme.txt": []byte("immutable fixture\n")}
	if source != "" {
		files["lua/main.lua"] = []byte(source)
	}
	return files
}

func Build(files map[string][]byte, dependencies ...dependency.LockedPackage) (*archive.Package, error) {
	d, err := manifest.Parse(files[archive.ManifestPath])
	if err != nil {
		return nil, err
	}
	ids := make([]string, len(dependencies))
	for i, dep := range dependencies {
		ids[i] = string(dep.PackageID)
	}
	root, err := dependency.NewLockedPackage(string(d.Package.PackageID), string(d.Package.Version), "sha256:0000000000000000000000000000000000000000000000000000000000000000", nil, ids)
	if err != nil {
		return nil, err
	}
	lock, err := dependency.BuildExactLock(string(d.Package.PackageID), append([]dependency.LockedPackage{root}, dependencies...))
	if err != nil {
		return nil, err
	}
	return archive.FromFiles(files, lock, extension.DefaultSupport)
}

func Archive(pkg *archive.Package) ([]byte, error) {
	s, err := pkg.Export()
	if err != nil {
		return nil, err
	}
	return s.Bytes(), nil
}
