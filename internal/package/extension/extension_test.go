// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package extension_test

import (
	"crypto/sha256"
	"encoding/hex"
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
	if document.Status != extension.ReadOnly || string(document.PayloadRaw) != string(payload) {
		t.Fatalf("optional unknown was not preserved")
	}
	descriptor.Required = true
	if _, err := extension.Load(descriptor, map[string][]byte{descriptor.SchemaPath: schema, descriptor.PayloadPath: payload}, extension.DefaultSupport); !extension.IsCode(err, extension.ErrRequiredUnsupported) {
		t.Fatalf("required unknown = %v", err)
	}
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
