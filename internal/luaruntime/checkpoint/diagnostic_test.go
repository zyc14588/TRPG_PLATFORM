// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package checkpoint

import (
	"bytes"
	"encoding/json"
	"fmt"
	"reflect"
	"strings"
	"testing"
)

func TestValueDiagnosticsNeverRenderPrivatePayloads(t *testing.T) {
	const marker = "synthetic-private-diagnostic-marker"
	value := Object(map[string]Value{marker: Text(marker), "nested": Array(Text(marker))})
	cyclic := Value{Kind: marker, String: marker, Table: make(map[string]Value)}
	cyclic.Table[marker] = cyclic
	for _, format := range []string{"%v", "%+v", "%#v", "%s", "%q", "%x", "%X", "%d"} {
		for _, input := range []any{value, &value, []Value{value}, map[string]Value{"state": value}, struct{ State Value }{value}, cyclic} {
			output := fmt.Sprintf(format, input)
			if strings.Contains(output, marker) || len(output) > 256 {
				t.Fatal("private or unbounded ordinary diagnostic")
			}
		}
	}
}

func TestDiagnosticRedactionPreservesExplicitJSONState(t *testing.T) {
	value := Object(map[string]Value{"counter": Int(7), "secret": Text("synthetic-authorized-codec-marker")})
	raw, err := json.Marshal(value)
	if err != nil || !bytes.Contains(raw, []byte(value.Table["secret"].String)) {
		t.Fatal("explicit state codec changed")
	}
	var restored Value
	if err = json.Unmarshal(raw, &restored); err != nil || !reflect.DeepEqual(value, restored) {
		t.Fatal("explicit state roundtrip changed")
	}
	before := Hash(raw)
	_ = fmt.Sprintf("%+v", value)
	after, err := json.Marshal(value)
	if err != nil || Hash(after) != before {
		t.Fatal("diagnostic formatting mutated state")
	}
}
