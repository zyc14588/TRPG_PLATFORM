// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package fixtureminimal

import (
	"encoding/json"
	"reflect"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
)

func TestVersionedSourceFixtureExactFiveRoles(t *testing.T) {
	pair, err := Load(install.RuntimeConfig{SHA256: checkpoint.Hash(nil), Limits: profile.DefaultLimits()})
	if err != nil {
		t.Fatal(err)
	}
	for _, graph := range []struct {
		root *archive.Package
		deps []*archive.Package
	}{{pair.Old.Root, pair.Old.Dependencies}, {pair.New.Root, pair.New.Dependencies}} {
		if len(graph.deps) != 4 {
			t.Fatal("fixture graph is not five roles")
		}
		for _, p := range append([]*archive.Package{graph.root}, graph.deps...) {
			s, err := p.Export()
			if err != nil {
				t.Fatal(err)
			}
			round, err := archive.Import(s, extension.DefaultSupport)
			if err != nil {
				t.Fatal(err)
			}
			if round.ContentHash() != p.ContentHash() || round.ArtifactIdentity().Digest() != p.ArtifactIdentity().Digest() || string(round.LockBytes()) != string(p.LockBytes()) {
				t.Fatal("source export/import changed identity")
			}
			doc, err := p.Manifest()
			if err != nil || len(doc.Package.Rights.Authors) == 0 || doc.Package.Rights.LicenseExpression != License {
				t.Fatal("fixture rights missing")
			}
		}
	}
}

func TestFixedReplayScenarioHasIndependentExpectedFacts(t *testing.T) {
	g, err := ReadGolden()
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(g.Initial, g.States[0]) || !reflect.DeepEqual(g.Checkpoint, g.States[2]) || g.Commands[0].Delta != 7 || g.Commands[1].Delta != 3 {
		t.Fatal("fixed scenario changed")
	}
	for n := range g.States {
		if eventstore.Digest(g.States[n]) != g.StateHashes[n] || eventstore.Digest(g.GMViews[n]) != g.GMHashes[n] || eventstore.Digest(g.PlayerViews[n]) != g.PlayerHashes[n] {
			t.Fatal("fixed expected hash changed")
		}
		if _, ok := g.PlayerViews[n].Table["secret"]; ok {
			t.Fatal("private fact in expected player view")
		}
	}
	for n, v := range g.EventPayloads {
		if eventstore.Digest(v) != g.EventHashes[n] || !reflect.DeepEqual(v, g.States[n+1]) {
			t.Fatal("fixed event payload changed")
		}
	}
	var snapshot map[string]any
	if json.Unmarshal(g.Snapshot["after-fixed-commands"], &snapshot) != nil || snapshot["version"] != float64(3) {
		t.Fatal("fixed snapshot missing")
	}
}
