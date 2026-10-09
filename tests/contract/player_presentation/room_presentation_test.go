// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package presentation_test

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"os"
	"strings"
	"testing"

	"github.com/santhosh-tekuri/jsonschema/v6"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

func TestRoomPresentationReusesUnchangedClosedEightDefinitionSchema(t *testing.T) {
	raw, e := os.ReadFile("../../../schemas/platform/platform-player-presentation-api-v1.schema.json")
	if e != nil {
		t.Fatal(e)
	}
	hash := sha256.Sum256(raw)
	if len(raw) > 64<<10 || hex.EncodeToString(hash[:]) != "c4b1d49becd67852e4a6e92c4754ab986458d6fb46134c5f4c1e4446c152cd91" {
		t.Fatal("shared presentation Schema bytes or bounds changed")
	}
	value, e := auth.RoomContractJSON(raw)
	if e != nil {
		t.Fatal(e)
	}
	m := value.(map[string]any)
	if len(m["$defs"].(map[string]any)) != 8 {
		t.Fatal("presentation definitions changed")
	}
	c := jsonschema.NewCompiler()
	c.DefaultDraft(jsonschema.Draft2020)
	if e = c.AddResource(m["$id"].(string), value); e != nil {
		t.Fatal(e)
	}
	s, e := c.Compile(m["$id"].(string) + "#/$defs/PresentationResponse")
	if e != nil {
		t.Fatal(e)
	}
	response := map[string]any{"schema_version": 1, "request_id": "response", "data": map[string]any{
		"workspace_id": "workspace", "configuration_id": "installed", "game_id": "game", "configuration_hash": "sha256:" + strings.Repeat("a", 64), "graph_hash": "sha256:" + strings.Repeat("b", 64),
		"packages":         []any{map[string]any{"package_id": "fixture.example/game", "version": "1.0.0", "title": "Trusted game", "artifact_digest": "sha256:" + strings.Repeat("c", 64), "rights_digest": "sha256:" + strings.Repeat("d", 64), "license": "MIT", "permissions": []any{"state.read"}}},
		"model_selections": []any{map[string]any{"selection_id": "selected", "label": "Trusted AI", "seat_ids": []any{"ai"}, "capabilities": []any{"structured-actions"}, "ready": false}},
	}}
	if e = s.Validate(response); e != nil {
		t.Fatal("valid shared response rejected")
	}
	for _, name := range []string{"room_id", "endpoint", "credential", "actor", "consent"} {
		t.Run(name, func(t *testing.T) {
			b, _ := json.Marshal(response)
			var v map[string]any
			_ = json.Unmarshal(b, &v)
			v["data"].(map[string]any)[name] = "private"
			if s.Validate(v) == nil {
				t.Fatal("room response expanded the shared closed contract")
			}
		})
	}
}
