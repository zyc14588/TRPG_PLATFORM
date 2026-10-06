// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package fixtureminimal loads the versioned, source-only M1 certification
// fixture. Its synthetic publisher keys and private facts are test data.
package fixtureminimal

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/migration"
)

const License = "PolyForm-Noncommercial-1.0.0"

// SourcePackage contains UTF-8 source, never executable or archive binaries.
// The exact lock remains the production lock document, including all hashes.
type SourcePackage struct {
	PackageID string            `json:"package_id"`
	Role      string            `json:"role"`
	Version   string            `json:"version"`
	Content   string            `json:"content_hash"`
	Artifact  string            `json:"artifact_hash"`
	Lock      json.RawMessage   `json:"exact_lock"`
	Files     map[string]string `json:"source_files"`
}

type Version struct {
	Version string   `json:"version"`
	Lock    string   `json:"graph_lock_hash"`
	Sources []string `json:"source_files"`
}

type Catalog struct {
	Format   int               `json:"format"`
	License  string            `json:"license_expression"`
	Purpose  string            `json:"purpose"`
	Versions []Version         `json:"versions"`
	SHA256   map[string]string `json:"source_sha256"`
}

type FixedCommand struct {
	ID      string  `json:"id"`
	Delta   int64   `json:"delta"`
	Time    int64   `json:"time"`
	Random  []int64 `json:"random"`
	Counter int64   `json:"expected_counter"`
	Version uint64  `json:"expected_version"`
	Cursor  uint64  `json:"expected_cursor"`
}

// Golden is authored fixed arithmetic, independent of observed execution.
// Binding/Runner digests in snapshots and checkpoints are supplied by the
// authenticated installed graph, rather than weakening their validation.
type Golden struct {
	Format        int                        `json:"format"`
	License       string                     `json:"license_expression"`
	Initial       checkpoint.Value           `json:"initial_state"`
	Commands      []FixedCommand             `json:"commands"`
	States        []checkpoint.Value         `json:"expected_states"`
	StateHashes   []string                   `json:"expected_state_hashes"`
	PlayerViews   []checkpoint.Value         `json:"expected_player_views"`
	PlayerHashes  []string                   `json:"expected_player_view_hashes"`
	GMViews       []checkpoint.Value         `json:"expected_gm_views"`
	GMHashes      []string                   `json:"expected_gm_view_hashes"`
	EventPayloads []checkpoint.Value         `json:"expected_event_payloads"`
	EventHashes   []string                   `json:"expected_event_payload_hashes"`
	Checkpoint    checkpoint.Value           `json:"expected_checkpoint_value"`
	Migrated      checkpoint.Value           `json:"expected_migrated_state"`
	Plan          json.RawMessage            `json:"safe_boundary_plan"`
	Snapshot      map[string]json.RawMessage `json:"snapshot_expectations"`
}

func RepoRoot() string {
	// Trimmed builds have module-relative caller paths. Search the current
	// checkout first; certification still checks its exact clean Git HEAD.
	if cwd, err := os.Getwd(); err == nil {
		for dir := cwd; ; dir = filepath.Dir(dir) {
			if raw, err := os.ReadFile(filepath.Join(dir, "go.mod")); err == nil && strings.HasPrefix(string(raw), "module github.com/zyc14588/TRPG_PLATFORM\n") {
				return dir
			}
			if filepath.Dir(dir) == dir {
				break
			}
		}
	}
	_, file, _, _ := runtime.Caller(0)
	return filepath.Clean(filepath.Join(filepath.Dir(file), "../.."))
}

func SourceRoot() string { return filepath.Join(RepoRoot(), "games/fixture-minimal") }

// Template produces source and synthetic policy data only. It never starts a
// Runner, installs an artifact, or grants runtime authority.
func Template() (fixture.Pair, error) {
	return fixture.BuildPair(install.RuntimeConfig{SHA256: checkpoint.Hash(nil), Limits: profile.DefaultLimits()}, true, false)
}

func ReadGolden() (Golden, error) {
	var g Golden
	raw, err := os.ReadFile(filepath.Join(SourceRoot(), "fixed-replay-v1.json"))
	if err == nil {
		err = checkpoint.StrictDecode(raw, &g, 256<<10)
	}
	if err == nil && (g.Format != 1 || g.License != License || len(g.Commands) != 2 || len(g.States) != 3 || len(g.PlayerViews) != 3 || len(g.GMViews) != 3 || len(g.EventPayloads) != 2) {
		err = fmt.Errorf("invalid fixed fixture shape")
	}
	return g, err
}

// Load binds the on-disk source to the same exact five-role graph used by the
// existing internal platformd fixture CLI. Every entry, lock and identity must
// match before that CLI or a Session factory can be used for certification.
func Load(r install.RuntimeConfig) (fixture.Pair, error) {
	pair, err := fixture.BuildPair(r, true, false)
	if err != nil {
		return pair, err
	}
	raw, err := os.ReadFile(filepath.Join(SourceRoot(), "catalog.json"))
	if err != nil {
		return pair, err
	}
	var catalog Catalog
	if checkpoint.StrictDecode(raw, &catalog, 64<<10) != nil || catalog.Format != 1 || catalog.License != License || len(catalog.Versions) != 2 || len(catalog.SHA256) != 10 {
		return pair, fmt.Errorf("invalid source catalog")
	}
	for i, expected := range []fixture.Graph{pair.Old, pair.New} {
		v := catalog.Versions[i]
		if v.Version != []string{"1.0.0", "1.1.0"}[i] || v.Lock != expected.Hash() || len(v.Sources) != 5 {
			return pair, fmt.Errorf("fixture version/lock mismatch")
		}
		want := map[string]*archive.Package{}
		for _, p := range append([]*archive.Package{expected.Root}, expected.Dependencies...) {
			doc, _ := p.Manifest()
			want[string(doc.Package.PackageID)] = p
		}
		loaded := map[string]*archive.Package{}
		roles := map[string]bool{}
		for _, name := range v.Sources {
			if filepath.ToSlash(filepath.Clean(name)) != name || filepath.IsAbs(name) || filepath.Dir(name) != v.Version || filepath.Ext(name) != ".json" {
				return pair, fmt.Errorf("invalid fixture source path")
			}
			b, err := os.ReadFile(filepath.Join(SourceRoot(), name))
			if err != nil || checkpoint.Hash(b) != catalog.SHA256[name] {
				return pair, fmt.Errorf("fixture source digest mismatch")
			}
			var source SourcePackage
			if checkpoint.StrictDecode(b, &source, 256<<10) != nil || source.Version != v.Version || roles[source.Role] || loaded[source.PackageID] != nil {
				return pair, fmt.Errorf("invalid source package")
			}
			lock, err := dependency.ParseExactLock(source.Lock)
			if err != nil {
				return pair, err
			}
			files := map[string][]byte{}
			for name, content := range source.Files {
				files[name] = []byte(content)
			}
			p, err := archive.FromFiles(files, lock, extension.DefaultSupport)
			if err != nil {
				return pair, err
			}
			doc, err := p.Manifest()
			e := want[source.PackageID]
			if err != nil || doc.Package == nil || e == nil || string(doc.Package.PackageKind) != source.Role || string(p.ContentHash()) != source.Content || string(p.ArtifactIdentity().Digest()) != source.Artifact || p.ContentHash() != e.ContentHash() || p.ArtifactIdentity().Digest() != e.ArtifactIdentity().Digest() || string(p.LockBytes()) != string(e.LockBytes()) {
				return pair, fmt.Errorf("source does not match authenticated fixture graph")
			}
			if len(p.Entries()) != len(e.Entries()) {
				return pair, fmt.Errorf("fixture source entry mismatch")
			}
			for _, entry := range e.Entries() {
				actual, ok := p.Entry(entry.Path())
				if !ok || string(actual.Bytes()) != string(entry.Bytes()) {
					return pair, fmt.Errorf("fixture source byte mismatch")
				}
			}
			loaded[source.PackageID], roles[source.Role] = p, true
		}
		if len(roles) != 5 {
			return pair, fmt.Errorf("missing fixture role")
		}
		g := fixture.Graph{Root: loaded[fixture.PackageID]}
		for _, e := range expected.Dependencies {
			doc, _ := e.Manifest()
			g.Dependencies = append(g.Dependencies, loaded[string(doc.Package.PackageID)])
		}
		if i == 0 {
			pair.Old = g
		} else {
			pair.New = g
		}
	}
	return pair, nil
}
