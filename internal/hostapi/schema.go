// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package hostapi

import (
	"encoding/json"
	"strconv"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
)

// Schema is bound to a verified immutable package entry. The already audited
// local-only 2020-12 validator enforces expansion, regex and data work budgets.
// This internal declaration creates no new manifest field or extension payload.
type Schema struct {
	document                             extension.Document
	packageID, packageHash, path, digest string
}

func BindSchema(pkg *archive.Package, path, digest string, initial checkpoint.Value) (Schema, error) {
	if pkg == nil || !checkpoint.IsDigest(digest) {
		return Schema{}, profile.Fail(profile.ErrConfiguration)
	}
	file, ok := pkg.Entry(path)
	if !ok || checkpoint.Hash(file.Bytes()) != digest {
		return Schema{}, profile.Fail(profile.ErrConfiguration)
	}
	d, err := pkg.Manifest()
	if err != nil || d.Package == nil {
		return Schema{}, profile.Fail(profile.ErrConfiguration)
	}
	payload, err := nativeJSON(initial)
	if err != nil {
		return Schema{}, err
	}
	// Synthetic paths exist only inside the bounded validator. The authority is
	// still the original artifact's actual path/digest, saved alongside the schema.
	descriptor := extension.Descriptor{Namespace: "platform.host.schema", Required: true, ContractVersion: 1, SchemaPath: "extensions/platform.host.schema/value.schema.json", SchemaSHA256: digest, PayloadPath: "extensions/platform.host.schema/value.json", HostAPIMajor: profile.HostMajor, HostAPIMinMinor: profile.HostMinor, HostAPIMaxMinor: profile.HostMinor}
	doc, err := extension.Load(descriptor, map[string][]byte{descriptor.SchemaPath: file.Bytes(), descriptor.PayloadPath: payload}, extension.DefaultSupport)
	if err != nil {
		return Schema{}, profile.Fail("SCHEMA_REJECTED")
	}
	return Schema{document: doc, packageID: string(d.Package.PackageID), packageHash: string(pkg.ContentHash()), path: path, digest: digest}, nil
}
func (s Schema) Validate(v checkpoint.Value) error {
	raw, err := nativeJSON(v)
	if err != nil {
		return err
	}
	if _, err = s.document.ValidateReplacementWithCanonicalLimit(raw, checkpoint.MaxBytes); err != nil {
		return profile.Fail("SCHEMA_REJECTED")
	}
	return nil
}
func (s Schema) Digest() string { return s.digest }
func nativeJSON(v checkpoint.Value) ([]byte, error) {
	if checkpoint.Validate(v) != nil {
		return nil, profile.Fail(profile.ErrValue)
	}
	x, err := native(v)
	if err != nil {
		return nil, err
	}
	return json.Marshal(x)
}
func native(v checkpoint.Value) (any, error) {
	switch v.Kind {
	case "nil":
		return nil, nil
	case "boolean":
		return v.Boolean, nil
	case "integer", "float":
		return json.Number(v.Number), nil
	case "string":
		return v.String, nil
	case "array":
		a := make([]any, len(v.Array))
		for i, x := range v.Array {
			n, e := native(x)
			if e != nil {
				return nil, e
			}
			a[i] = n
		}
		return a, nil
	case "table":
		a := map[string]any{}
		for k, x := range v.Table {
			n, e := native(x)
			if e != nil {
				return nil, e
			}
			a[k] = n
		}
		return a, nil
	}
	return nil, profile.Fail(profile.ErrValue)
}
func digest(v any) string { raw, _ := json.Marshal(v); return checkpoint.Hash(raw) }
func clone[T any](v T) (T, error) {
	var copy T
	raw, err := json.Marshal(v)
	if err == nil {
		err = json.Unmarshal(raw, &copy)
	}
	return copy, err
}
func integer(v checkpoint.Value) (int64, bool) {
	if v.Kind != "integer" {
		return 0, false
	}
	n, err := strconv.ParseInt(v.Number, 10, 64)
	return n, err == nil
}
