// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Source-only exporter for committed Go fuzz regression inputs.
package main

import (
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	minimal "github.com/zyc14588/TRPG_PLATFORM/tests/fixture-minimal"
	"github.com/zyc14588/TRPG_PLATFORM/tests/fuzz/support"
	"os"
	"path/filepath"
)

func main() {
	_, _, plan := support.Migration()
	seeds := []struct {
		pkg, target    string
		valid, invalid []byte
	}{
		{"protocol", "FuzzProtocolEnvelope", support.Protocol(), []byte(`{"command_id":"a","command_id":"b"}`)},
		{"package", "FuzzPackageArchive", support.Archive(), []byte("PK malformed bounded archive")},
		{"package", "FuzzPackageManifestTOML", support.Manifest(), []byte("schema_version=1\nschema_version=2\n")},
		{"package", "FuzzPackageSchema", []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"boolean"}`), []byte(`{"$ref":"https://example.invalid/remote.schema.json"}`)},
		{"callback", "FuzzHostCallbackParameters", support.Callback(), []byte(`{"kind":"callback","call":{"operation":"raw_sql"}}`)},
		{"event", "FuzzEventDeserialization", support.JSON(support.Record()), []byte(`{"schema_version":1}`)},
		{"event", "FuzzEventUpcaster", support.JSON(support.Upcast()), []byte(`{"Target":0}`)},
		{"migration", "FuzzMigrationEntrypoint", support.JSON(plan), []byte(`{"capability":"sql"}`)},
	}
	for _, s := range seeds {
		dir := filepath.Join(minimal.RepoRoot(), "tests/fuzz", s.pkg, "testdata/fuzz", s.target)
		if err := os.MkdirAll(dir, 0755); err != nil {
			panic(err)
		}
		for name, raw := range map[string][]byte{"valid": s.valid, "malformed": s.invalid} {
			corpus := fmt.Sprintf("go test fuzz v1\n[]byte(%q)\n", raw)
			if err := os.WriteFile(filepath.Join(dir, name), []byte(corpus), 0644); err != nil {
				panic(err)
			}
		}
	}
	// Authored regression for canonical JSON's empty-collection normalization.
	// Existing minimized failures are never deleted or regenerated here.
	empty := support.Upcast()
	empty.Target = 1
	empty.Event.Payload = checkpoint.Object(map[string]checkpoint.Value{})
	for path, raw := range map[string][]byte{
		"protocol/testdata/fuzz/FuzzProtocolEnvelope/empty-table": []byte(`{"payload":{"kind":"table","table":{}}}`),
		"event/testdata/fuzz/FuzzEventUpcaster/empty-table":       []byte(fmt.Sprintf(`{"Event":{"ID":"fixed-event","Type":"example.test/fuzz/change","Payload":{"kind":"table","table":{}},"SchemaHash":%q,"SchemaVersion":1},"Target":1,"Rules":[]}`, empty.Event.SchemaHash)),
	} {
		if err := os.WriteFile(filepath.Join(minimal.RepoRoot(), "tests/fuzz", path), []byte(fmt.Sprintf("go test fuzz v1\n[]byte(%q)\n", raw)), 0644); err != nil {
			panic(err)
		}
	}
}
