// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package room_test

import (
	"encoding/json"
	"github.com/santhosh-tekuri/jsonschema/v6"
	"os"
	"testing"
)

type offline struct{}

func (offline) Load(string) (any, error) { return nil, os.ErrPermission }
func TestOwnerApprovedRoomSchemaSamples(t *testing.T) {
	b, e := os.ReadFile("../../../schemas/platform/platform-room-api-v1.schema.json")
	if e != nil {
		t.Fatal("approved Schema read")
	}
	var v map[string]any
	if json.Unmarshal(b, &v) != nil {
		t.Fatal("Schema parse")
	}
	uri := "urn:trpg-platform:platform-room-api:v1"
	if v["$id"] != uri || v["x-status"] != "ACTIVE" {
		t.Fatal("Schema identity")
	}
	c := jsonschema.NewCompiler()
	c.DefaultDraft(jsonschema.Draft2020)
	c.AssertFormat()
	c.UseLoader(offline{})
	if c.AddResource(uri, v) != nil {
		t.Fatal("Schema resource")
	}
	defs, ok := v["$defs"].(map[string]any)
	if !ok || len(defs) != 25 {
		t.Fatal("definition inventory")
	}
	for name := range defs {
		if _, e = c.Compile(uri + "#/$defs/" + name); e != nil {
			t.Fatal("definition compilation")
		}
	}
	b, e = os.ReadFile("samples.json")
	if e != nil {
		t.Fatal("approved samples read")
	}
	var samples []struct {
		Schema string
		Valid  bool
		Data   any
	}
	if json.Unmarshal(b, &samples) != nil || len(samples) != 25 {
		t.Fatal("approved samples inventory")
	}
	for i, sample := range samples {
		t.Run(sample.Schema+"_"+string(rune('a'+i)), func(t *testing.T) {
			s, e := c.Compile(uri + "#/$defs/" + sample.Schema)
			if e != nil {
				t.Fatal("sample definition")
			}
			if (s.Validate(sample.Data) == nil) != sample.Valid {
				t.Fatal("approved sample result differs")
			}
		})
	}
}
