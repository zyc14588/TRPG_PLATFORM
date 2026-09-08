// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package extension_test

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
)

func descriptor(schema []byte) extension.Descriptor {
	digest := sha256.Sum256(schema)
	return extension.Descriptor{
		Namespace: "third.party.probe", Required: true, ContractVersion: 1,
		SchemaPath:   "extensions/third.party.probe/value.schema.json",
		SchemaSHA256: "sha256:" + hex.EncodeToString(digest[:]),
		PayloadPath:  "extensions/third.party.probe/value.json",
		HostAPIMajor: 1, HostAPIMinMinor: 0, HostAPIMaxMinor: 0,
	}
}

func TestSupportedDocumentEdit(t *testing.T) {
	t.Parallel()
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["count"],"properties":{"count":{"type":"integer","minimum":0}}}`)
	descriptor := descriptor(schema)
	document, err := extension.Load(descriptor, map[string][]byte{descriptor.SchemaPath: schema, descriptor.PayloadPath: []byte(`{"count":1}`)}, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if document.Status != extension.Supported {
		t.Fatalf("status = %s", document.Status)
	}
	canonical, err := document.ValidateReplacement([]byte(` { "count" : 2 } `))
	if err != nil {
		t.Fatal(err)
	}
	if string(canonical) != `{"count":2}` {
		t.Fatalf("canonical = %s", canonical)
	}
	if _, err := document.ValidateReplacement([]byte(`{"count":-1}`)); !extension.IsCode(err, extension.ErrSchemaValidation) {
		t.Fatalf("schema rejection = %v", err)
	}
}

func TestHostCanonicalLimitBoundsLoadAndReplacement(t *testing.T) {
	t.Parallel()
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object"}`)
	item := descriptor(schema)
	files := map[string][]byte{item.SchemaPath: schema, item.PayloadPath: []byte(` { "value" : 1 } `)}
	if _, err := extension.LoadWithCanonicalLimit(item, files, extension.DefaultSupport, len(`{"value":1}`)-1); !extension.IsCode(err, extension.ErrInvalid) {
		t.Fatalf("bounded load result = %v", err)
	}
	document, err := extension.LoadWithCanonicalLimit(item, files, extension.DefaultSupport, len(`{"value":1}`))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := document.ValidateReplacementWithCanonicalLimit([]byte(`{"value":2}`), len(`{"value":2}`)-1); !extension.IsCode(err, extension.ErrInvalid) {
		t.Fatalf("bounded replacement result = %v", err)
	}
	if got, err := document.ValidateReplacementWithCanonicalLimit([]byte(`{"value":2}`), len(`{"value":2}`)); err != nil || string(got) != `{"value":2}` {
		t.Fatalf("boundary replacement = %q, %v", got, err)
	}

	item.Required = false
	item.ContractVersion++
	opaque := []byte(`{"duplicate":1,"duplicate":2}`)
	readonly, err := extension.LoadWithCanonicalLimit(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: opaque}, extension.DefaultSupport, 0)
	if err != nil || readonly.Status != extension.ReadOnly || string(readonly.PayloadBytes()) != string(opaque) {
		t.Fatalf("opaque optional bounded load = %#v, %v", readonly, err)
	}
}

func TestOptionalUnknownPreservesRawAndRequiredFails(t *testing.T) {
	t.Parallel()
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema"}`)
	descriptor := descriptor(schema)
	descriptor.ContractVersion = 2
	descriptor.Required = false
	payload := []byte("{ \"opaque\" : true, \"opaque\" : false }\n")
	document, err := extension.Load(descriptor, map[string][]byte{descriptor.SchemaPath: schema, descriptor.PayloadPath: payload}, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if document.Status != extension.ReadOnly || string(document.PayloadBytes()) != string(payload) {
		t.Fatalf("optional unknown was not preserved")
	}
	descriptor.Required = true
	if _, err := extension.Load(descriptor, map[string][]byte{descriptor.SchemaPath: schema, descriptor.PayloadPath: payload}, extension.DefaultSupport); !extension.IsCode(err, extension.ErrRequiredUnsupported) {
		t.Fatalf("required unknown = %v", err)
	}
}

func TestDocumentRawAccessorsAreDefensive(t *testing.T) {
	t.Parallel()
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object"}`)
	payload := []byte(`{"value":1}`)
	item := descriptor(schema)
	document, err := extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: payload}, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	schema[0], payload[0] = 'x', 'x'
	if got := string(document.SchemaBytes()); !strings.HasPrefix(got, `{"$schema"`) {
		t.Fatalf("schema alias changed document: %q", got)
	}
	if got := string(document.PayloadBytes()); got != `{"value":1}` {
		t.Fatalf("payload alias changed document: %q", got)
	}
	first := document.CanonicalPayload()
	first[0] = 'x'
	if got := string(document.CanonicalPayload()); got != `{"value":1}` {
		t.Fatalf("canonical accessor leaked backing storage: %q", got)
	}
	if _, ok := document.PayloadValue(); !ok {
		t.Fatal("supported payload value is unavailable")
	}
}

func TestLocalReferenceCycleUsesOptionalAndRequiredSemantics(t *testing.T) {
	t.Parallel()
	for _, schema := range [][]byte{
		[]byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","$ref":"#"}`),
		[]byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","$anchor":"loop","$ref":"#loop"}`),
		[]byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","$defs":{"a":{"$ref":"#/$defs/b"},"b":{"$ref":"#/$defs/a"}},"$ref":"#/$defs/a"}`),
	} {
		item := descriptor(schema)
		item.Required = false
		files := map[string][]byte{item.SchemaPath: schema, item.PayloadPath: []byte(`{}`)}
		document, err := extension.Load(item, files, extension.DefaultSupport)
		if err != nil {
			t.Fatalf("optional cyclic schema: %v", err)
		}
		if document.Status != extension.ReadOnly || !strings.Contains(document.ReadOnlyReason, "cyclic") {
			t.Fatalf("optional cyclic schema status=%s reason=%q", document.Status, document.ReadOnlyReason)
		}
		item.Required = true
		if _, err := extension.Load(item, files, extension.DefaultSupport); !extension.IsCode(err, extension.ErrRequiredUnsupported) {
			t.Fatalf("required cyclic schema = %v", err)
		}
	}
}

func TestAcyclicSharedLocalReferencesRemainSupported(t *testing.T) {
	t.Parallel()
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","$defs":{"leaf":{"type":"integer"},"pair":{"allOf":[{"$ref":"#/$defs/leaf"},{"$ref":"#/$defs/leaf"}]}},"$ref":"#/$defs/pair"}`)
	item := descriptor(schema)
	document, err := extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: []byte(`1`)}, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if document.Status != extension.Supported {
		t.Fatalf("status = %s", document.Status)
	}
}

func TestReferenceLikeDataIsNotAuditedAsSchema(t *testing.T) {
	t.Parallel()
	for _, test := range []struct {
		schema  []byte
		payload []byte
	}{
		{schema: []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","const":{"$ref":"https://example.invalid/data"}}`), payload: []byte(`{"$ref":"https://example.invalid/data"}`)},
		{schema: []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"$ref":{"type":"string"}}}`), payload: []byte(`{"$ref":"value"}`)},
	} {
		item := descriptor(test.schema)
		if _, err := extension.Load(item, map[string][]byte{item.SchemaPath: test.schema, item.PayloadPath: test.payload}, extension.DefaultSupport); err != nil {
			t.Fatalf("reference-like data rejected: %v", err)
		}
	}
}

func TestAcyclicReferenceExpansionIsBounded(t *testing.T) {
	t.Parallel()
	schema := doublingSchema(t, 20, map[string]any{"type": "integer"})
	item := descriptor(schema)
	item.Required = false
	files := map[string][]byte{item.SchemaPath: schema, item.PayloadPath: []byte(`1`)}
	document, err := extension.Load(item, files, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if document.Status != extension.ReadOnly || !strings.Contains(document.ReadOnlyReason, "expansion") {
		t.Fatalf("status=%s reason=%q", document.Status, document.ReadOnlyReason)
	}
	item.Required = true
	if _, err := extension.Load(item, files, extension.DefaultSupport); !extension.IsCode(err, extension.ErrRequiredUnsupported) {
		t.Fatalf("required expansion = %v", err)
	}
}

func TestBooleanLocalReferenceRemainsSupported(t *testing.T) {
	t.Parallel()
	for _, test := range []struct {
		name    string
		boolean bool
		wantErr bool
	}{
		{name: "allow", boolean: true},
		{name: "deny", boolean: false, wantErr: true},
	} {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			schema, err := json.Marshal(map[string]any{
				"$schema": "https://json-schema.org/draft/2020-12/schema",
				"$defs":   map[string]any{"target": test.boolean},
				"$ref":    "#/$defs/target",
			})
			if err != nil {
				t.Fatal(err)
			}
			item := descriptor(schema)
			_, err = extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: []byte(`null`)}, extension.DefaultSupport)
			if test.wantErr && !extension.IsCode(err, extension.ErrSchemaValidation) {
				t.Fatalf("false schema result = %v", err)
			}
			if !test.wantErr && err != nil {
				t.Fatalf("true schema result = %v", err)
			}
		})
	}
}

func TestAnchorGrammarMatchesDraft202012(t *testing.T) {
	t.Parallel()
	valid := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","$defs":{"leaf":{"$anchor":"_leaf-name","type":"integer"}},"$ref":"#_leaf-name"}`)
	item := descriptor(valid)
	if _, err := extension.Load(item, map[string][]byte{item.SchemaPath: valid, item.PayloadPath: []byte(`1`)}, extension.DefaultSupport); err != nil {
		t.Fatalf("valid underscore anchor: %v", err)
	}
	invalid := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","$anchor":"bad:anchor"}`)
	item = descriptor(invalid)
	item.Required = false
	document, err := extension.Load(item, map[string][]byte{item.SchemaPath: invalid, item.PayloadPath: []byte(`null`)}, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if document.Status != extension.ReadOnly {
		t.Fatalf("invalid anchor status = %s", document.Status)
	}
}

func TestAcyclicExpansionUsesPayloadByteBudget(t *testing.T) {
	t.Parallel()
	schema := doublingSchema(t, 12, map[string]any{"type": "string", "pattern": "^z$"})
	item := descriptor(schema)
	payload := []byte(`"` + strings.Repeat("x", 2048) + `"`)
	_, err := extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: payload}, extension.DefaultSupport)
	if !extension.IsCode(err, extension.ErrInvalid) || !strings.Contains(err.Error(), "byte evaluation") {
		t.Fatalf("byte-weighted expansion result = %v", err)
	}
}

func TestRegexpSourceAndProgramAreBoundedBeforeCompiler(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name   string
		schema func() []byte
	}{
		{
			name: "pattern source",
			schema: func() []byte {
				data, _ := json.Marshal(map[string]any{
					"$schema": "https://json-schema.org/draft/2020-12/schema",
					"pattern": strings.Repeat("a?", extension.MaxSchemaPatternBytes/2+1),
				})
				return data
			},
		},
		{
			name: "patternProperties source",
			schema: func() []byte {
				data, _ := json.Marshal(map[string]any{
					"$schema": "https://json-schema.org/draft/2020-12/schema",
					"patternProperties": map[string]any{
						strings.Repeat("a?", extension.MaxSchemaPatternBytes/2+1): true,
					},
				})
				return data
			},
		},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			schema := test.schema()
			item := descriptor(schema)
			item.Required = false
			files := map[string][]byte{item.SchemaPath: schema, item.PayloadPath: []byte(`"value"`)}
			document, err := extension.Load(item, files, extension.DefaultSupport)
			if err != nil {
				t.Fatal(err)
			}
			if document.Status != extension.ReadOnly || !strings.Contains(document.ReadOnlyReason, "byte budget") {
				t.Fatalf("optional regex status=%s reason=%q", document.Status, document.ReadOnlyReason)
			}
			item.Required = true
			if _, err := extension.Load(item, files, extension.DefaultSupport); !extension.IsCode(err, extension.ErrRequiredUnsupported) {
				t.Fatalf("required regex result = %v", err)
			}
		})
	}
}

func TestRegexpAggregateProgramIsBoundedBeforeCompiler(t *testing.T) {
	t.Parallel()
	patterns := make(map[string]any)
	for index := 0; index < 48; index++ {
		// Each source remains below the per-pattern limit and the collection
		// remains below the total source-byte limit. Its compiled program is
		// intentionally larger than the independent instruction budget.
		pattern := fmt.Sprintf("%02d", index) + strings.Repeat("a?", 449)
		patterns[pattern] = true
	}
	schema, err := json.Marshal(map[string]any{
		"$schema":           "https://json-schema.org/draft/2020-12/schema",
		"patternProperties": patterns,
	})
	if err != nil {
		t.Fatal(err)
	}
	item := descriptor(schema)
	item.Required = false
	document, err := extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: []byte(`{}`)}, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if document.Status != extension.ReadOnly || !strings.Contains(document.ReadOnlyReason, "instruction budget") {
		t.Fatalf("aggregate regexp status=%s reason=%q", document.Status, document.ReadOnlyReason)
	}
}

func TestCompactRepeatProgramIsRejectedBeforeSimplify(t *testing.T) {
	t.Parallel()
	for _, literalBytes := range []int{3300, 4086} {
		literalBytes := literalBytes
		t.Run(fmt.Sprint(literalBytes), func(t *testing.T) {
			t.Parallel()
			schema, err := json.Marshal(map[string]any{
				"$schema": "https://json-schema.org/draft/2020-12/schema",
				"pattern": "(?:" + strings.Repeat("a", literalBytes) + "){1000}",
			})
			if err != nil {
				t.Fatal(err)
			}
			item := descriptor(schema)
			item.Required = false
			document, err := extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: []byte(`"a"`)}, extension.DefaultSupport)
			if err != nil {
				t.Fatal(err)
			}
			if document.Status != extension.ReadOnly || (literalBytes == 3300 && !strings.Contains(document.ReadOnlyReason, "instruction budget")) {
				t.Fatalf("compact repeat status=%s reason=%q", document.Status, document.ReadOnlyReason)
			}
		})
	}
}

func TestNestedUnboundedRepeatCannotHideExpandedProgram(t *testing.T) {
	t.Parallel()
	schema, err := json.Marshal(map[string]any{
		"$schema": "https://json-schema.org/draft/2020-12/schema",
		"pattern": "(?:(?:" + strings.Repeat("a", 3300) + "){1000}){0,}",
	})
	if err != nil {
		t.Fatal(err)
	}
	item := descriptor(schema)
	item.Required = false
	document, err := extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: []byte(`"a"`)}, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if document.Status != extension.ReadOnly || !strings.Contains(document.ReadOnlyReason, "instruction budget") {
		t.Fatalf("nested repeat status=%s reason=%q", document.Status, document.ReadOnlyReason)
	}
}

func TestUnicodeRegexpWithinBudgetRemainsSupported(t *testing.T) {
	t.Parallel()
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"string","pattern":"^[\\p{L}_][\\p{L}\\p{N}_-]*$"}`)
	item := descriptor(schema)
	if _, err := extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: []byte(`"规则_7"`)}, extension.DefaultSupport); err != nil {
		t.Fatalf("bounded Unicode regexp rejected: %v", err)
	}
}

func TestUnicodeClassSourceAndRuneTablesAreBoundedBeforeCompile(t *testing.T) {
	t.Parallel()
	properties := make(map[string]any, 16)
	for index := 0; index < 16; index++ {
		properties[fmt.Sprintf("%02d", index)+strings.Repeat(`\pL`, 1364)] = true
	}
	schema, err := json.Marshal(map[string]any{
		"$schema":           "https://json-schema.org/draft/2020-12/schema",
		"patternProperties": properties,
	})
	if err != nil {
		t.Fatal(err)
	}
	item := descriptor(schema)
	item.Required = false
	document, err := extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: []byte(`{}`)}, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if document.Status != extension.ReadOnly || !strings.Contains(document.ReadOnlyReason, "Unicode-class budget") {
		t.Fatalf("Unicode-class family status=%s reason=%q", document.Status, document.ReadOnlyReason)
	}

	schema, err = json.Marshal(map[string]any{
		"$schema": "https://json-schema.org/draft/2020-12/schema",
		"pattern": strings.Repeat(`\pL`, 50),
	})
	if err != nil {
		t.Fatal(err)
	}
	item = descriptor(schema)
	item.Required = false
	document, err = extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: []byte(`"value"`)}, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if document.Status != extension.ReadOnly || !strings.Contains(document.ReadOnlyReason, "rune table") {
		t.Fatalf("rune-table status=%s reason=%q", document.Status, document.ReadOnlyReason)
	}
}

func TestRegexpProgramWorkIsWeightedByPayloadBytes(t *testing.T) {
	t.Parallel()
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"string","pattern":"(?:a|A){1000}b"}`)
	item := descriptor(schema)
	payload := []byte(`"` + strings.Repeat("a", extension.MaxPayloadBytes-2) + `"`)
	_, err := extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: payload}, extension.DefaultSupport)
	if !extension.IsCode(err, extension.ErrInvalid) || !strings.Contains(err.Error(), "regular-expression byte evaluation") {
		t.Fatalf("regexp program work result = %v", err)
	}

	small := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"string","pattern":"^a+b$"}`)
	item = descriptor(small)
	if _, err := extension.Load(item, map[string][]byte{item.SchemaPath: small, item.PayloadPath: []byte(`"aaab"`)}, extension.DefaultSupport); err != nil {
		t.Fatalf("small regexp rejected: %v", err)
	}
}

func TestUniqueItemsCollisionFamilyIsRejectedBeforeValidation(t *testing.T) {
	t.Parallel()
	payload := uniqueHashCollisionFamily(t, 1024)
	valueSchema := func(unique bool) []byte {
		data, err := json.Marshal(map[string]any{
			"$schema":     "https://json-schema.org/draft/2020-12/schema",
			"type":        "array",
			"uniqueItems": unique,
		})
		if err != nil {
			t.Fatal(err)
		}
		return data
	}
	schema := valueSchema(true)
	item := descriptor(schema)
	_, err := extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: payload}, extension.DefaultSupport)
	if !extension.IsCode(err, extension.ErrInvalid) || !strings.Contains(err.Error(), "uniqueItems byte evaluation") {
		t.Fatalf("uniqueItems collision budget = %v", err)
	}

	// False is not an assertion and must not consume the unique-items budget.
	schema = valueSchema(false)
	item = descriptor(schema)
	if _, err := extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: payload}, extension.DefaultSupport); err != nil {
		t.Fatalf("uniqueItems=false was charged: %v", err)
	}
}

func TestUniqueItemsTypeFailureUsesSupportSemantics(t *testing.T) {
	t.Parallel()
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","uniqueItems":"yes"}`)
	item := descriptor(schema)
	item.Required = false
	files := map[string][]byte{item.SchemaPath: schema, item.PayloadPath: []byte(`[]`)}
	document, err := extension.Load(item, files, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if document.Status != extension.ReadOnly || !strings.Contains(document.ReadOnlyReason, "uniqueItems") {
		t.Fatalf("uniqueItems type status=%s reason=%q", document.Status, document.ReadOnlyReason)
	}
	item.Required = true
	if _, err := extension.Load(item, files, extension.DefaultSupport); !extension.IsCode(err, extension.ErrRequiredUnsupported) {
		t.Fatalf("required uniqueItems type result = %v", err)
	}
}

func TestLegacySchemaDependenciesCannotBypassGraphAudit(t *testing.T) {
	t.Parallel()
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","properties":{"nested":{"dependencies":{"trigger":{"allOf":[{"$ref":"#/$defs/node"},{"$ref":"#/$defs/node"}]}}}},"$defs":{"node":{"type":"object"}}}`)
	item := descriptor(schema)
	item.Required = false
	files := map[string][]byte{item.SchemaPath: schema, item.PayloadPath: []byte(`{}`)}
	document, err := extension.Load(item, files, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if document.Status != extension.ReadOnly || !strings.Contains(document.ReadOnlyReason, "legacy dependencies") {
		t.Fatalf("legacy dependencies status=%s reason=%q", document.Status, document.ReadOnlyReason)
	}
	item.Required = true
	if _, err := extension.Load(item, files, extension.DefaultSupport); !extension.IsCode(err, extension.ErrRequiredUnsupported) {
		t.Fatalf("required legacy dependencies result = %v", err)
	}
}

func TestAssertionDataIsWeightedByDAGReachability(t *testing.T) {
	t.Parallel()
	enumeration := make([]any, 100000)
	for index := range enumeration {
		enumeration[index] = index
	}
	required := make([]string, 50000)
	for index := range required {
		required[index] = fmt.Sprintf("required-%05d", index)
	}
	dependent := make([]string, 100)
	for index := range dependent {
		dependent[index] = fmt.Sprintf("dependent-%03d", index)
	}
	tests := []struct {
		name string
		leaf map[string]any
	}{
		{name: "enum data", leaf: map[string]any{"enum": enumeration}},
		{name: "required names", leaf: map[string]any{"type": "object", "required": required}},
		{name: "dependentRequired names", leaf: map[string]any{"type": "object", "dependentRequired": map[string]any{"trigger": dependent}}},
		{name: "const data", leaf: map[string]any{"const": strings.Repeat("c", 4096)}},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			schema := doublingSchema(t, 12, test.leaf)
			if len(schema) > extension.MaxSchemaBytes {
				t.Fatalf("test schema size = %d", len(schema))
			}
			item := descriptor(schema)
			item.Required = false
			files := map[string][]byte{item.SchemaPath: schema, item.PayloadPath: []byte(`{}`)}
			document, err := extension.Load(item, files, extension.DefaultSupport)
			if err != nil {
				t.Fatal(err)
			}
			if document.Status != extension.ReadOnly || !strings.Contains(document.ReadOnlyReason, "assertion") {
				t.Fatalf("assertion status=%s reason=%q", document.Status, document.ReadOnlyReason)
			}
			if test.name == "enum data" {
				item.Required = true
				if _, err := extension.Load(item, files, extension.DefaultSupport); !extension.IsCode(err, extension.ErrRequiredUnsupported) {
					t.Fatalf("required assertion result = %v", err)
				}
			}
		})
	}
}

func TestSmallAssertionsRemainSupported(t *testing.T) {
	t.Parallel()
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","required":["a"],"dependentRequired":{"a":["b"]},"properties":{"a":{"enum":[1,2]},"b":{"const":true}}}`)
	item := descriptor(schema)
	if _, err := extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: []byte(`{"a":1,"b":true}`)}, extension.DefaultSupport); err != nil {
		t.Fatalf("small assertions rejected: %v", err)
	}
}

type collisionArray struct {
	children []*collisionArray
}

func uniqueHashCollisionFamily(t *testing.T, count int) []byte {
	t.Helper()
	values := make([]any, count)
	for mask := 0; mask < count; mask++ {
		root := &collisionArray{}
		first := &collisionArray{}
		root.children = append(root.children, first)
		last := first
		for bit := 0; bit < 10; bit++ {
			child := &collisionArray{}
			if mask&(1<<bit) == 0 {
				root.children = append(root.children, child)
			} else {
				last.children = append(last.children, child)
			}
			last = child
		}
		values[mask] = collisionArrayValue(root)
	}
	data, err := json.Marshal(values)
	if err != nil {
		t.Fatal(err)
	}
	return data
}

func collisionArrayValue(node *collisionArray) []any {
	value := make([]any, 1, 1+len(node.children))
	value[0] = nil
	for _, child := range node.children {
		value = append(value, collisionArrayValue(child))
	}
	return value
}

func TestValidationDiagnosticIsBoundedAndDoesNotEchoPayload(t *testing.T) {
	t.Parallel()
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"string","pattern":"^allowed$"}`)
	item := descriptor(schema)
	secret := strings.Repeat("SECRET-PAYLOAD-", 128)
	_, err := extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: []byte(`"` + secret + `"`)}, extension.DefaultSupport)
	var contract *extension.ContractError
	if !errors.As(err, &contract) || contract.Code != extension.ErrSchemaValidation {
		t.Fatalf("validation result = %v", err)
	}
	if len(contract.Detail) > 400 || strings.Contains(contract.Detail, "SECRET-PAYLOAD") {
		t.Fatalf("unsafe diagnostic length=%d detail=%q", len(contract.Detail), contract.Detail)
	}
	if !strings.Contains(contract.Detail, "instance=") || !strings.Contains(contract.Detail, "keyword=") {
		t.Fatalf("diagnostic lacks bounded locations: %q", contract.Detail)
	}
}

func TestValidationDiagnosticBoundsHugeInstanceKey(t *testing.T) {
	t.Parallel()
	key := strings.Repeat("k", 128<<10)
	schema, err := json.Marshal(map[string]any{
		"$schema":              "https://json-schema.org/draft/2020-12/schema",
		"type":                 "object",
		"additionalProperties": map[string]any{"type": "integer"},
	})
	if err != nil {
		t.Fatal(err)
	}
	payload, err := json.Marshal(map[string]any{key: "wrong"})
	if err != nil {
		t.Fatal(err)
	}
	item := descriptor(schema)
	_, err = extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: payload}, extension.DefaultSupport)
	var contract *extension.ContractError
	if !errors.As(err, &contract) || contract.Code != extension.ErrSchemaValidation {
		t.Fatalf("validation result = %v", err)
	}
	if len(contract.Detail) > 400 {
		t.Fatalf("huge-key diagnostic length = %d", len(contract.Detail))
	}
}

func TestCanonicalPayloadExpansionCannotExceedPayloadLimit(t *testing.T) {
	t.Parallel()
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"array"}`)
	item := descriptor(schema)
	payload := []byte("[" + strings.Repeat("1e127,", 33000) + "1e127]")
	if len(payload) >= extension.MaxPayloadBytes {
		t.Fatalf("test payload raw size = %d", len(payload))
	}
	if _, err := extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: payload}, extension.DefaultSupport); !extension.IsCode(err, extension.ErrInvalid) {
		t.Fatalf("canonical payload amplification = %v", err)
	}
}

func TestManyFailingItemsCannotBuildUnboundedErrorTree(t *testing.T) {
	t.Parallel()
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"array","items":{"type":"integer"}}`)
	item := descriptor(schema)
	payload := []byte("[" + strings.Repeat("null,", 32999) + "null]")
	_, err := extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: payload}, extension.DefaultSupport)
	if !extension.IsCode(err, extension.ErrInvalid) || !strings.Contains(err.Error(), "evaluation exceeds") {
		t.Fatalf("many-item validation budget = %v", err)
	}
}

func TestSchemaLocationLengthPreventsPointerPrefixAmplification(t *testing.T) {
	t.Parallel()
	children := make(map[string]any, 1000)
	for index := 0; index < 1000; index++ {
		children[fmt.Sprintf("child-%04d", index)] = map[string]any{"type": "integer"}
	}
	schema, err := json.Marshal(map[string]any{
		"$schema": "https://json-schema.org/draft/2020-12/schema",
		"properties": map[string]any{
			strings.Repeat("p", extension.MaxSchemaLocationBytes): map[string]any{"properties": children},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	item := descriptor(schema)
	item.Required = false
	document, err := extension.Load(item, map[string][]byte{item.SchemaPath: schema, item.PayloadPath: []byte(`{}`)}, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if document.Status != extension.ReadOnly || !strings.Contains(document.ReadOnlyReason, "location") {
		t.Fatalf("location-bound status=%s reason=%q", document.Status, document.ReadOnlyReason)
	}
}

func doublingSchema(t *testing.T, depth int, leaf map[string]any) []byte {
	t.Helper()
	definitions := make(map[string]any, depth)
	for index := 0; index < depth; index++ {
		name := fmt.Sprintf("n%d", index)
		if index == depth-1 {
			definitions[name] = leaf
			continue
		}
		next := fmt.Sprintf("#/$defs/n%d", index+1)
		definitions[name] = map[string]any{"allOf": []any{map[string]any{"$ref": next}, map[string]any{"$ref": next}}}
	}
	schema, err := json.Marshal(map[string]any{
		"$schema": "https://json-schema.org/draft/2020-12/schema",
		"$defs":   definitions,
		"$ref":    "#/$defs/n0",
	})
	if err != nil {
		t.Fatal(err)
	}
	return schema
}

func TestRejectsNamespaceDigestAndExternalRefs(t *testing.T) {
	t.Parallel()
	base := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","$ref":"https://example.invalid/schema"}`)
	item := descriptor(base)
	files := map[string][]byte{item.SchemaPath: base, item.PayloadPath: []byte(`{}`)}
	if _, err := extension.Load(item, files, extension.DefaultSupport); err == nil {
		t.Fatal("remote $ref accepted")
	}
	reserved := item
	reserved.Namespace = "trpg.platform.probe"
	reserved.SchemaPath = "extensions/trpg.platform.probe/value.schema.json"
	reserved.PayloadPath = "extensions/trpg.platform.probe/value.json"
	if _, err := extension.NormalizeDescriptor(reserved); err == nil {
		t.Fatal("reserved namespace accepted")
	}
	overlong := item
	overlong.Namespace = strings.Repeat("a", 63) + "." + strings.Repeat("b", 63) + ".cc"
	overlong.SchemaPath = "extensions/" + overlong.Namespace + "/value.schema.json"
	overlong.PayloadPath = "extensions/" + overlong.Namespace + "/value.json"
	if len(overlong.Namespace) <= extension.MaxNamespaceBytes {
		t.Fatal("test namespace is not over the exported boundary")
	}
	if _, err := extension.NormalizeDescriptor(overlong); err == nil {
		t.Fatal("overlong namespace accepted")
	}
	zeroMajor := descriptor(base)
	zeroMajor.HostAPIMajor = 0
	if _, err := extension.NormalizeDescriptor(zeroMajor); err == nil {
		t.Fatal("zero Host API major accepted")
	}
	item.SchemaSHA256 = "sha256:" + string(make([]byte, 64))
	if _, err := extension.Load(item, files, extension.DefaultSupport); err == nil {
		t.Fatal("digest mismatch accepted")
	}
}
