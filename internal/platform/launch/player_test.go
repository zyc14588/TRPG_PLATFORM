// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package launch

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestPlayerCatalogProjectionOwnsSlicesAndDropsModelCapabilities(t *testing.T) {
	c := ConfigurationData{WorkspaceID: "workspace", ID: "installed", ContentTags: []string{"visible"}, SafetyTags: []string{"boundary"}, Seats: []SeatRule{{ID: "ai", Required: true, Modes: []string{"ai"}, ModelCapabilities: []string{"private-native-capability"}}}}
	v := publicConfiguration(c)
	c.ContentTags[0] = "changed"
	c.Seats[0].Modes[0] = "human"
	x := v.StorageValue()
	if x.ContentTags[0] != "visible" || x.Seats[0].Modes[0] != "ai" || len(x.Seats[0].ModelCapabilities) != 0 {
		t.Fatal("catalog copied private authority or aliased mutable source")
	}
	b, e := json.Marshal(x)
	if e != nil || strings.Contains(string(b), "private-native-capability") || strings.Contains(string(b), "Factory") {
		t.Fatal("catalog authority escaped")
	}
}
