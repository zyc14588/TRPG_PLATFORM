// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package presentation_test

import (
	"github.com/santhosh-tekuri/jsonschema/v6"
	native "github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"os"
	"strings"
	"testing"
)

func TestPresentationProductContractNativePackageIdentity(t *testing.T) {
	raw, e := os.ReadFile("../../../schemas/platform/platform-player-presentation-api-v1.schema.json")
	if e != nil {
		t.Fatal(e)
	}
	v, e := auth.RoomContractJSON(raw)
	if e != nil {
		t.Fatal(e)
	}
	m := v.(map[string]any)
	if m["x-status"] != "ACTIVE" || len(m["$defs"].(map[string]any)) != 8 {
		t.Fatal("wrong product contract")
	}
	c := jsonschema.NewCompiler()
	c.DefaultDraft(jsonschema.Draft2020)
	if c.AddResource("urn:trpg-platform:platform-player-presentation-api:v1", v) != nil {
		t.Fatal("contract resource")
	}
	s, e := c.Compile("urn:trpg-platform:platform-player-presentation-api:v1#/$defs/Package/properties/package_id")
	if e != nil {
		t.Fatal(e)
	}
	for _, id := range []string{"fixture.example/game", "a/b", "test-name/content_pack", "alias", "Upper/game", "a//b", "a/b/c", "a/" + strings.Repeat("b", 129), "a/b-"} {
		_, nativeError := native.ParsePackageID(id)
		schemaError := s.Validate(id)
		if (nativeError == nil) != (schemaError == nil) {
			t.Fatalf("native identity disagreement for bounded synthetic ID")
		}
	}
}
func TestPresentationOnlyAddsOneReadRoute(t *testing.T) {
	raw, e := os.ReadFile("../../../docs/20-architecture/PLAYER_PRESENTATION_API.md")
	if e != nil {
		t.Fatal(e)
	}
	if strings.Count(string(raw), "GET /api/v1/workspaces/{workspace}/games/{configuration}/presentation") != 1 {
		t.Fatal("additive exact read route missing/duplicated")
	}
}
