// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player

import (
	"encoding/json"
	"os"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

func codec(t *testing.T) *Service {
	t.Helper()
	raw, e := os.ReadFile("../../../schemas/platform/platform-player-api-v1.schema.json")
	if e != nil {
		t.Fatal("player contract missing")
	}
	schemas, e := compile(raw)
	if e != nil {
		t.Fatal("approved player contract rejected")
	}
	d := &serviceData{schemas: schemas}
	return &Service{data: &d}
}
func TestPlayerRuntimeContractPinnedAndAllDefinitionsCompiled(t *testing.T) {
	s := codec(t)
	if len(s.state().schemas) != 39 {
		t.Fatal("definition coverage incomplete")
	}
	raw, e := os.ReadFile("../../../schemas/platform/platform-player-api-v1.schema.json")
	if e != nil {
		t.Fatal("contract unreadable")
	}
	var m map[string]any
	if json.Unmarshal(raw, &m) != nil {
		t.Fatal("contract invalid")
	}
	for _, name := range []string{"metadata", "definition", "external"} {
		t.Run(name, func(t *testing.T) {
			var n map[string]any
			if json.Unmarshal(raw, &n) != nil {
				t.Fatal("contract invalid")
			}
			switch name {
			case "metadata":
				n["x-status"] = "DRAFT"
			case "definition":
				n["$defs"].(map[string]any)["ConfigureRequest"].(map[string]any)["additionalProperties"] = true
			case "external":
				n["$ref"] = "https://example.invalid/player.json"
			}
			b, _ := json.Marshal(n)
			if _, e = compile(b); e != auth.ErrInvalid {
				t.Fatal("non-approved contract accepted")
			}
		})
	}
	if _, e = compile(append(raw, raw...)); e != auth.ErrInvalid {
		t.Fatal("trailing JSON accepted")
	}
	if _, e = compile([]byte(strings.Repeat(" ", 65537))); e != auth.ErrInvalid {
		t.Fatal("oversized contract accepted")
	}
}
func TestPlayerRejectsClientAuthorityDuplicateKeysAndUnsafeValues(t *testing.T) {
	s := codec(t)
	base := `{"schema_version":1,"connection_id":"connection-owned","command_id":"once","expected_state_version":"1","type":"increment","payload":{"kind":"table","table":{"delta":{"kind":"integer","number":"1"}}},"correlation_id":"user"}`
	if _, e := s.Decode("command", "workspace", "room", []byte(base)); e != nil {
		t.Fatal("approved request rejected")
	}
	cases := map[string]string{"duplicate": strings.Replace(base, `"schema_version":1`, `"schema_version":1,"schema_version":1`, 1), "overflow": strings.Replace(base, `"expected_state_version":"1"`, `"expected_state_version":"9223372036854775807"`, 1), "leading-zero": strings.Replace(base, `"expected_state_version":"1"`, `"expected_state_version":"01"`, 1), "noncanonical-number": strings.Replace(base, `"number":"1"`, `"number":"01"`, 1), "capability-key": strings.Replace(base, `"delta":`, `"cap:secret":`, 1), "nonfinite": strings.Replace(base, `"kind":"integer","number":"1"`, `"kind":"float","number":"NaN"`, 1), "trailing": base + `{}`}
	for _, field := range []string{"principal", "session_id", "seat_id", "factory", "filter", "model_endpoint", "api_key", "lua", "random"} {
		cases[field] = strings.TrimSuffix(base, "}") + `,"` + field + `":"untrusted"}`
	}
	for name, body := range cases {
		t.Run(name, func(t *testing.T) {
			if _, e := s.Decode("command", "workspace", "room", []byte(body)); e != auth.ErrInvalid {
				t.Fatal("unsafe request accepted; request withheld")
			}
		})
	}
	if _, e := s.Decode("lobby", "workspace", "room", []byte(`{}`)); e != auth.ErrInvalid {
		t.Fatal("GET body accepted")
	}
	if _, e := s.Decode("snapshot", "workspace", "room", []byte(`{"schema_version":1,"connection_id":"owned","after_cursor":"0","limit":129}`)); e != auth.ErrInvalid {
		t.Fatal("unbounded event page accepted")
	}
}
func TestPlayerValueDepthAndResourceLimits(t *testing.T) {
	s := codec(t)
	v := `{"kind":"nil"}`
	for i := 0; i < 34; i++ {
		v = `{"kind":"array","array":[` + v + `]}`
	}
	body := `{"schema_version":1,"connection_id":"owned","command_id":"once","expected_state_version":"1","type":"increment","payload":` + v + `,"correlation_id":"owned"}`
	if _, e := s.Decode("command", "workspace", "room", []byte(body)); e != auth.ErrInvalid {
		t.Fatal("checkpoint depth limit bypassed")
	}
	if _, e := s.Decode("configure", "workspace", "room", []byte(strings.Repeat(" ", MaxRequestBytes+1))); e != auth.ErrInvalid {
		t.Fatal("request byte limit bypassed")
	}
	for _, s := range []string{"0", "1", "9223372036854775806"} {
		if _, e := counter(s); e != nil {
			t.Fatal("signed64 counter rejected")
		}
	}
	for _, s := range []string{"-1", "01", "1.0", "9223372036854775807", "999999999999999999999"} {
		if _, e := counter(s); e != auth.ErrInvalid {
			t.Fatal("invalid counter accepted")
		}
	}
}
