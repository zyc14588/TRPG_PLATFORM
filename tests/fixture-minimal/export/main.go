// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// This source-only exporter never executes or certifies a Lua package.
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/migration"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	minimal "github.com/zyc14588/TRPG_PLATFORM/tests/fixture-minimal"
)

func write(path string, v any) []byte {
	raw, err := json.MarshalIndent(v, "", "  ")
	if err != nil {
		panic(err)
	}
	raw = append(raw, '\n')
	if err = os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		panic(err)
	}
	if err = os.WriteFile(path, raw, 0644); err != nil {
		panic(err)
	}
	return raw
}

func main() {
	pair, err := minimal.Template()
	if err != nil {
		panic(err)
	}
	root := minimal.SourceRoot()
	catalog := minimal.Catalog{Format: 1, License: minimal.License, Purpose: "Linux M1 synthetic source-only certification fixture", SHA256: map[string]string{}}
	for i, g := range []fixture.Graph{pair.Old, pair.New} {
		v := minimal.Version{Version: []string{"1.0.0", "1.1.0"}[i], Lock: g.Hash()}
		packages := append([]*archive.Package{g.Root}, g.Dependencies...)
		sort.Slice(packages, func(i, j int) bool { return string(packages[i].ContentHash()) < string(packages[j].ContentHash()) })
		for _, p := range packages {
			doc, err := p.Manifest()
			if err != nil {
				panic(err)
			}
			s := minimal.SourcePackage{PackageID: string(doc.Package.PackageID), Role: string(doc.Package.PackageKind), Version: v.Version, Content: string(p.ContentHash()), Artifact: string(p.ArtifactIdentity().Digest()), Lock: p.LockBytes(), Files: map[string]string{}}
			for _, e := range p.Entries() {
				s.Files[e.Path()] = string(e.Bytes())
			}
			name := v.Version + "/" + s.Role + ".json"
			catalog.SHA256[name] = checkpoint.Hash(write(filepath.Join(root, name), s))
			v.Sources = append(v.Sources, name)
		}
		sort.Strings(v.Sources)
		catalog.Versions = append(catalog.Versions, v)
	}
	write(filepath.Join(root, "catalog.json"), catalog)
	// These values are fixed scenario expectations, not captured actual output.
	g := minimal.Golden{Format: 1, License: minimal.License, Initial: fixture.State(false, 1), Commands: []minimal.FixedCommand{
		{ID: "fixed-7", Delta: 7, Time: 1700000000, Random: []int64{7, 17}, Counter: 8, Version: 2, Cursor: 1},
		{ID: "fixed-3", Delta: 3, Time: 1700000001, Random: []int64{3, 19}, Counter: 11, Version: 3, Cursor: 2},
	}, States: []checkpoint.Value{fixture.State(false, 1), fixture.State(false, 8), fixture.State(false, 11)}, Checkpoint: fixture.State(false, 11), Migrated: fixture.State(true, 11), Snapshot: map[string]json.RawMessage{}}
	for _, state := range g.States {
		g.StateHashes = append(g.StateHashes, eventstore.Digest(state))
		g.GMViews = append(g.GMViews, state)
		g.GMHashes = append(g.GMHashes, eventstore.Digest(state))
		player := checkpoint.Object(map[string]checkpoint.Value{"counter": state.Table["counter"]})
		g.PlayerViews = append(g.PlayerViews, player)
		g.PlayerHashes = append(g.PlayerHashes, eventstore.Digest(player))
	}
	g.EventPayloads = append(g.EventPayloads, g.States[1:]...)
	for _, v := range g.EventPayloads {
		g.EventHashes = append(g.EventHashes, eventstore.Digest(v))
	}
	g.Plan, _ = json.Marshal(pair.Plan())
	rowEntry, _ := pair.Old.Root.Entry("schemas/row.schema.json")
	stateEntry, _ := pair.Old.Root.Entry("schemas/state.schema.json")
	snapshot := data.Snapshot{Binding: data.Binding{Workspace: "$workspace", Session: "$session", GraphHash: pair.Old.Hash()}, Version: 3, SchemaHash: checkpoint.Hash(stateEntry.Bytes()), State: g.States[2], Rows: []data.Row{{PackageID: fixture.PackageID, Namespace: "docs", Key: "one", SchemaHash: checkpoint.Hash(rowEntry.Bytes()), Value: checkpoint.Object(map[string]checkpoint.Value{"score": checkpoint.Int(11)})}}, Quantities: []data.Quantity{{PackageID: fixture.PackageID, Table: "quantity", Key: "one", Value: 10}}}
	g.Snapshot["after-fixed-commands"], _ = json.Marshal(snapshot)
	write(filepath.Join(root, "fixed-replay-v1.json"), g)
	fmt.Println("source-only fixture: 2 versions, 5 exact roles each; fixed scenario exported; runtime NOT_RUN")
}
