// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player_test

import (
	"encoding/json"
	"os"
	"sort"
	"testing"

	"github.com/santhosh-tekuri/jsonschema/v6"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

type denyLoader struct{}

func (denyLoader) Load(string) (any, error) { return nil, auth.ErrDenied }
func TestApprovedPlayerSchemaHasClosedDefinitionsAndDecimalCounters(t *testing.T) {
	raw, e := os.ReadFile("../../../schemas/platform/platform-player-api-v1.schema.json")
	if e != nil {
		t.Fatal("player contract missing")
	}
	value, e := auth.RoomContractJSON(raw)
	if e != nil {
		t.Fatal("player contract invalid")
	}
	root := value.(map[string]any)
	if root["x-status"] != "ACTIVE" || root["x-section-id"] != "SCHEMA-PLATFORM-PLAYER-API-V1" {
		t.Fatal("adopted identity mismatch")
	}
	defs := root["$defs"].(map[string]any)
	if len(defs) != 39 {
		t.Fatal("definition set incomplete")
	}
	c := jsonschema.NewCompiler()
	c.DefaultDraft(jsonschema.Draft2020)
	c.AssertFormat()
	c.UseLoader(denyLoader{})
	if c.AddResource("urn:trpg-platform:platform-player-api:v1", root) != nil {
		t.Fatal("contract resource invalid")
	}
	names := make([]string, 0, len(defs))
	for n := range defs {
		names = append(names, n)
	}
	sort.Strings(names)
	for _, n := range names {
		t.Run(n, func(t *testing.T) {
			if _, e := c.Compile("urn:trpg-platform:platform-player-api:v1#/$defs/" + n); e != nil {
				t.Fatal("approved definition does not compile")
			}
		})
	}
	for _, n := range []string{"CommandRequest", "ConnectRequest", "ControlRequest", "ConfigureRequest", "ExportRequest"} {
		if defs[n].(map[string]any)["additionalProperties"] != false {
			t.Fatal("client authority fields left open")
		}
	}
	request, e := c.Compile("urn:trpg-platform:platform-player-api:v1#/$defs/ControlRequest")
	if e != nil {
		t.Fatal("control definition missing")
	}
	for _, raw := range []string{`{"schema_version":1,"expected_control_revision":1}`, `{"schema_version":1,"expected_control_revision":"01"}`, `{"schema_version":1,"expected_control_revision":"1","consent_for_others":true}`} {
		var v any
		if json.Unmarshal([]byte(raw), &v) != nil || request.Validate(v) == nil {
			t.Fatal("unsafe control contract vector accepted")
		}
	}
}
