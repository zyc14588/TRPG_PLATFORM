// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player

import (
	"encoding/json"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"os"
	"strings"
	"testing"
)

func presentationSchemas(t *testing.T) *PresentationService {
	t.Helper()
	raw, e := os.ReadFile("../../../schemas/platform/platform-player-presentation-api-v1.schema.json")
	if e != nil {
		t.Fatal(e)
	}
	s, e := compilePresentation(raw)
	if e != nil {
		t.Fatal(e)
	}
	return &PresentationService{schemas: s}
}
func TestPresentationResponseByteLimitWithoutTruncation(t *testing.T) {
	s := presentationSchemas(t)
	v := validPresentation()
	permissions := []string{}
	for i := 0; i < 64; i++ {
		permissions = append(permissions, fmt.Sprintf("permission-%02d-%s", i, strings.Repeat("x", 110)))
	}
	v.Packages = nil
	for i := 0; i < 64; i++ {
		p := validPresentation().Packages[0]
		p.PackageID = fmt.Sprintf("fixture.example/package-%02d", i)
		p.Permissions = permissions
		v.Packages = append(v.Packages, p)
	}
	raw, e := json.Marshal(v)
	if e != nil || len(raw) <= MaxResponseBytes {
		t.Fatal("fixture did not exceed wire byte bound")
	}
	if _, e := s.encode(v); e != auth.ErrUnavailable {
		t.Fatal("oversized complete list accepted or truncated")
	}
}
func validPresentation() Presentation {
	h := "sha256:" + strings.Repeat("a", 64)
	return Presentation{"workspace", "configuration", "game", h, h, []install.PresentationPackage{{PackageID: "test.example/game", Version: "1.0.0", Title: "Installed game", ArtifactDigest: h, RightsDigest: h, License: "MIT", Permissions: []string{"host.rules", "host.state"}}}, []model.PresentationSelection{{SelectionID: "selected", Label: "Verified selection", SeatIDs: []string{"ai"}, Capabilities: []string{"ai-player"}, Ready: false}}}
}
func TestPresentationClosedBoundedSchema(t *testing.T) {
	s := presentationSchemas(t)
	if len(s.schemas) != 8 {
		t.Fatal("all eight definitions must compile")
	}
	if _, e := s.encode(validPresentation()); e != nil {
		t.Fatal(e)
	}
	cases := map[string]func(*Presentation){
		"package_native_id": func(v *Presentation) { v.Packages[0].PackageID = "fictional-alias" },
		"invalid_graph":     func(v *Presentation) { v.GraphHash = strings.Repeat("a", 64) },
		"label_81":          func(v *Presentation) { v.Packages[0].Title = strings.Repeat("界", 81) },
		"label_control":     func(v *Presentation) { v.ModelSelections[0].Label = "private\ntext" },
		"package_65": func(v *Presentation) {
			for len(v.Packages) < 65 {
				v.Packages = append(v.Packages, v.Packages[0])
			}
		},
		"selection_65": func(v *Presentation) {
			for len(v.ModelSelections) < 65 {
				v.ModelSelections = append(v.ModelSelections, v.ModelSelections[0])
			}
		},
		"seat_65": func(v *Presentation) {
			for i := 0; i < 65; i++ {
				v.ModelSelections[0].SeatIDs = append(v.ModelSelections[0].SeatIDs, "seat")
			}
		},
		"capability_unknown":    func(v *Presentation) { v.Packages[0].Permissions = []string{"external/permission"} },
		"duplicate_permissions": func(v *Presentation) { v.Packages[0].Permissions = []string{"host.state", "host.state"} },
		"duplicate_seats":       func(v *Presentation) { v.ModelSelections[0].SeatIDs = []string{"ai", "ai"} },
		"version_overbound":     func(v *Presentation) { v.Packages[0].Version = strings.Repeat("v", 65) },
	}
	for name, mutate := range cases {
		t.Run(name, func(t *testing.T) {
			v := validPresentation()
			mutate(&v)
			if _, e := s.encode(v); e != auth.ErrUnavailable {
				t.Fatal("unbounded or malformed response accepted")
			}
		})
	}
	raw, _ := json.Marshal(validPresentation())
	var fields map[string]any
	_ = json.Unmarshal(raw, &fields)
	fields["endpoint"] = "https://secret.invalid"
	if s.schemas["Presentation"].Validate(fields) == nil {
		t.Fatal("private extra field accepted")
	}
}
func TestPresentationRejectsChangedOrIncompleteContract(t *testing.T) {
	raw, e := os.ReadFile("../../../schemas/platform/platform-player-presentation-api-v1.schema.json")
	if e != nil {
		t.Fatal(e)
	}
	for name, b := range map[string][]byte{"missing": nil, "wrong_status": []byte(strings.Replace(string(raw), "ACTIVE", "DRAFT", 1)), "unused_external": []byte(strings.Replace(string(raw), `"$defs": {`, `"unused":{"$ref":"https://untrusted.invalid/schema"},"$defs": {`, 1)), "duplicate": []byte(strings.Replace(string(raw), `"x-status": "ACTIVE",`, `"x-status":"DRAFT","x-status":"ACTIVE",`, 1)), "oversize": append(raw, []byte(strings.Repeat(" ", 65537))...), "invalid_utf8": append(raw, 255)} {
		t.Run(name, func(t *testing.T) {
			if _, e := compilePresentation(b); e != auth.ErrInvalid {
				t.Fatal("changed contract accepted")
			}
		})
	}
}
