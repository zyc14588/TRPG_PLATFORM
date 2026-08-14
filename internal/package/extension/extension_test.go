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
