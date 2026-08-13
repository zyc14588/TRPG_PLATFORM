// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package extension implements the generic, package-ID-independent
// namespaced JSON extension contract.
package extension

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"regexp"
	"sort"
	"strings"

	"github.com/santhosh-tekuri/jsonschema/v6"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/jsondocument"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/packagepath"
)

const (
	MaxExtensions            = 64
	MaxSchemaBytes           = 1 << 20
	MaxPayloadBytes          = 4 << 20
	SupportedContractVersion = 1
)

var namespacePattern = regexp.MustCompile(`^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?){2,7}$`)

type ErrorCode string

const (
	ErrInvalid              ErrorCode = "ERR_EXTENSION_INVALID"
	ErrRequiredUnsupported  ErrorCode = "ERR_EXTENSION_REQUIRED_UNSUPPORTED"
	ErrReadOnly             ErrorCode = "ERR_EXTENSION_READ_ONLY"
	ErrSchemaDigestMismatch ErrorCode = "ERR_EXTENSION_SCHEMA_DIGEST_MISMATCH"
	ErrSchemaMissing        ErrorCode = "ERR_EXTENSION_SCHEMA_MISSING"
	ErrPayloadMissing       ErrorCode = "ERR_EXTENSION_PAYLOAD_MISSING"
	ErrSchemaValidation     ErrorCode = "ERR_EXTENSION_SCHEMA_VALIDATION"
)

type ContractError struct {
	Code      ErrorCode `json:"code"`
	Namespace string    `json:"namespace,omitempty"`
	Detail    string    `json:"detail"`
}

func (err *ContractError) Error() string {
	if err.Namespace == "" {
		return fmt.Sprintf("%s: %s", err.Code, err.Detail)
	}
	return fmt.Sprintf("%s %s: %s", err.Code, err.Namespace, err.Detail)
}

func IsCode(err error, code ErrorCode) bool {
	contract, ok := err.(*ContractError)
	return ok && contract.Code == code
}

func contractError(code ErrorCode, namespace, format string, values ...any) error {
	return &ContractError{Code: code, Namespace: namespace, Detail: fmt.Sprintf(format, values...)}
}

// Descriptor is the exact [[extensions]] manifest-v2 representation.
type Descriptor struct {
	Namespace       string `json:"namespace"`
	Required        bool   `json:"required"`
	ContractVersion uint32 `json:"contract_version"`
	SchemaPath      string `json:"schema_path"`
	SchemaSHA256    string `json:"schema_sha256"`
	PayloadPath     string `json:"payload_path"`
	HostAPIMajor    uint32 `json:"host_api_major"`
	HostAPIMinMinor uint32 `json:"host_api_min_minor"`
	HostAPIMaxMinor uint32 `json:"host_api_max_minor"`
}

func NormalizeDescriptor(value Descriptor) (Descriptor, error) {
	if len(value.Namespace) > 128 || !namespacePattern.MatchString(value.Namespace) {
		return Descriptor{}, contractError(ErrInvalid, value.Namespace, "namespace is not canonical")
	}
	if value.Namespace == "trpg.platform" || strings.HasPrefix(value.Namespace, "trpg.platform.") {
		return Descriptor{}, contractError(ErrInvalid, value.Namespace, "reserved namespace is unavailable to every package")
	}
	if value.ContractVersion == 0 {
		return Descriptor{}, contractError(ErrInvalid, value.Namespace, "contract_version must be positive")
	}
	if value.HostAPIMinMinor > value.HostAPIMaxMinor {
		return Descriptor{}, contractError(ErrInvalid, value.Namespace, "host API minor range is inverted")
	}
	if value.HostAPIMajor == 0 {
		return Descriptor{}, contractError(ErrInvalid, value.Namespace, "host_api_major must be nonzero")
	}
	if len(value.SchemaSHA256) != len("sha256:")+64 || !strings.HasPrefix(value.SchemaSHA256, "sha256:") {
		return Descriptor{}, contractError(ErrInvalid, value.Namespace, "schema_sha256 must be sha256:<lowerhex>")
	}
	digest := strings.TrimPrefix(value.SchemaSHA256, "sha256:")
	if strings.ToLower(digest) != digest {
		return Descriptor{}, contractError(ErrInvalid, value.Namespace, "schema_sha256 must use lower-case hex")
	}
	if _, err := hex.DecodeString(digest); err != nil {
		return Descriptor{}, contractError(ErrInvalid, value.Namespace, "schema_sha256 is not hexadecimal")
	}
	prefix := "extensions/" + value.Namespace + "/"
	if err := validateReferencePath(value.SchemaPath, prefix, ".schema.json"); err != nil {
		return Descriptor{}, contractError(ErrInvalid, value.Namespace, "schema_path: %v", err)
	}
	if err := validateReferencePath(value.PayloadPath, prefix, ".json"); err != nil {
		return Descriptor{}, contractError(ErrInvalid, value.Namespace, "payload_path: %v", err)
	}
	if value.PayloadPath == value.SchemaPath {
		return Descriptor{}, contractError(ErrInvalid, value.Namespace, "schema_path and payload_path must differ")
	}
	return value, nil
}

func validateReferencePath(value, prefix, suffix string) error {
	if err := packagepath.Validate(value); err != nil {
		return err
	}
	if !strings.HasPrefix(value, prefix) || !strings.HasSuffix(value, suffix) {
		return fmt.Errorf("must remain under %q and end in %q", prefix, suffix)
	}
	return nil
}

// NormalizeDescriptors rejects duplicate namespaces and file ownership, then
// returns namespace order for deterministic Host and manifest models.
func NormalizeDescriptors(values []Descriptor) ([]Descriptor, error) {
	if len(values) > MaxExtensions {
		return nil, contractError(ErrInvalid, "", "package has %d extensions, maximum is %d", len(values), MaxExtensions)
	}
	result := make([]Descriptor, 0, len(values))
	namespaces := make(map[string]struct{}, len(values))
	paths := make(map[string]string, len(values)*2)
	for _, input := range values {
		value, err := NormalizeDescriptor(input)
		if err != nil {
			return nil, err
		}
		if _, exists := namespaces[value.Namespace]; exists {
			return nil, contractError(ErrInvalid, value.Namespace, "duplicate namespace")
		}
		namespaces[value.Namespace] = struct{}{}
		for _, file := range []string{value.SchemaPath, value.PayloadPath} {
			key := packagepath.CollisionKey(file)
			if owner, exists := paths[key]; exists {
				return nil, contractError(ErrInvalid, value.Namespace, "path %q is already owned by %s", file, owner)
			}
			paths[key] = value.Namespace
		}
		result = append(result, value)
	}
	sort.Slice(result, func(i, j int) bool { return result[i].Namespace < result[j].Namespace })
	return result, nil
}

type Status string

const (
	Supported Status = "SUPPORTED_EDITABLE"
	ReadOnly  Status = "UNSUPPORTED_OPTIONAL_READ_ONLY"
)

// Support identifies the generic contract and Host API implemented by a
// loader. It carries no package, publisher, trust, or official identity.
type Support struct {
	ContractVersion uint32
	HostAPIMajor    uint32
	HostAPIMinor    uint32
}

var DefaultSupport = Support{ContractVersion: SupportedContractVersion, HostAPIMajor: 1, HostAPIMinor: 0}

// Document preserves descriptor and raw referenced bytes. Editable documents
// additionally hold the strict parsed payload and compiled local schema.
type Document struct {
	Descriptor       Descriptor         `json:"descriptor"`
	Status           Status             `json:"status"`
	ReadOnlyReason   string             `json:"read_only_reason,omitempty"`
	SchemaRaw        []byte             `json:"-"`
	PayloadRaw       []byte             `json:"-"`
	PayloadCanonical []byte             `json:"payload,omitempty"`
	Payload          jsondocument.Value `json:"-"`
	compiled         *jsonschema.Schema
}

// Load validates one descriptor against package-local files. Missing optional
// schemas, unknown versions/dialects, and incompatible Host ranges are
// preserved read-only. Digest mismatch and invalid referenced content fail.
func Load(descriptor Descriptor, files map[string][]byte, support Support) (Document, error) {
	value, err := NormalizeDescriptor(descriptor)
	if err != nil {
		return Document{}, err
	}
	payload, payloadExists := files[value.PayloadPath]
	if !payloadExists {
		return Document{}, contractError(ErrPayloadMissing, value.Namespace, "payload %q is absent", value.PayloadPath)
	}
	if len(payload) > MaxPayloadBytes {
		return Document{}, contractError(ErrInvalid, value.Namespace, "payload exceeds %d bytes", MaxPayloadBytes)
	}
	document := Document{Descriptor: value, SchemaRaw: clone(files[value.SchemaPath]), PayloadRaw: clone(payload)}
	schema, schemaExists := files[value.SchemaPath]
	if !schemaExists {
		return unsupported(document, value.Required, ErrSchemaMissing, "schema is absent")
	}
	if len(schema) > MaxSchemaBytes {
		return Document{}, contractError(ErrInvalid, value.Namespace, "schema exceeds %d bytes", MaxSchemaBytes)
	}
	digest := sha256.Sum256(schema)
	if got := "sha256:" + hex.EncodeToString(digest[:]); got != value.SchemaSHA256 {
		return Document{}, contractError(ErrSchemaDigestMismatch, value.Namespace, "schema digest is %s, manifest declares %s", got, value.SchemaSHA256)
	}
	if value.ContractVersion != support.ContractVersion {
		return unsupported(document, value.Required, ErrRequiredUnsupported, "contract_version is unsupported")
	}
	if value.HostAPIMajor != support.HostAPIMajor || support.HostAPIMinor < value.HostAPIMinMinor || support.HostAPIMinor > value.HostAPIMaxMinor {
		return unsupported(document, value.Required, ErrRequiredUnsupported, "Host API range is unsupported")
	}
	parsedSchema, err := jsondocument.Parse(schema)
	if err != nil {
		return unsupported(document, value.Required, ErrRequiredUnsupported, fmt.Sprintf("schema JSON is unavailable: %v", err))
	}
	dialect, exists := parsedSchema.Lookup("$schema")
	dialectText, isText := dialect.Text()
	if !exists || !isText || dialectText != "https://json-schema.org/draft/2020-12/schema" {
		return unsupported(document, value.Required, ErrRequiredUnsupported, "schema dialect is unsupported")
	}
	if err := auditSchema(parsedSchema); err != nil {
		return Document{}, contractError(ErrInvalid, value.Namespace, "schema reference policy: %v", err)
	}
	compiled, err := compileSchema(parsedSchema.Canonical())
	if err != nil {
		return Document{}, contractError(ErrInvalid, value.Namespace, "compile schema: %v", err)
	}
	document.compiled = compiled
	document.Status = Supported
	parsedPayload, err := jsondocument.Parse(payload)
	if err != nil {
		return Document{}, contractError(ErrInvalid, value.Namespace, "payload JSON: %v", err)
	}
	document.Payload = parsedPayload
	document.PayloadCanonical = parsedPayload.Canonical()
	if err := document.validateCanonical(document.PayloadCanonical); err != nil {
		return Document{}, err
	}
	return document, nil
}

func unsupported(document Document, required bool, _ ErrorCode, reason string) (Document, error) {
	if required {
		return Document{}, contractError(ErrRequiredUnsupported, document.Descriptor.Namespace, "%s", reason)
	}
	document.Status = ReadOnly
	document.ReadOnlyReason = reason
	return document, nil
}

// ValidateReplacement is the only edit path. It refuses read-only documents,
// runs the strict JSON parser before schema validation, and returns canonical
// bytes suitable for deterministic export.
func (document Document) ValidateReplacement(data []byte) ([]byte, error) {
	if document.Status != Supported || document.compiled == nil {
		return nil, contractError(ErrReadOnly, document.Descriptor.Namespace, "optional unsupported extension is read-only")
	}
	if len(data) > MaxPayloadBytes {
		return nil, contractError(ErrInvalid, document.Descriptor.Namespace, "payload exceeds %d bytes", MaxPayloadBytes)
	}
	value, err := jsondocument.Parse(data)
	if err != nil {
		return nil, contractError(ErrInvalid, document.Descriptor.Namespace, "payload JSON: %v", err)
	}
	canonical := value.Canonical()
	if err := document.validateCanonical(canonical); err != nil {
		return nil, err
	}
	return canonical, nil
}

func (document Document) validateCanonical(data []byte) error {
	value, err := jsondocument.Parse(data)
	if err != nil { return contractError(ErrInvalid, document.Descriptor.Namespace, "decode canonical payload: %v", err) }
	if err := document.compiled.Validate(value.Interface()); err != nil {
		return contractError(ErrSchemaValidation, document.Descriptor.Namespace, "%v", err)
	}
	return nil
}

func auditSchema(value jsondocument.Value) error {
	switch value.Kind() {
	case jsondocument.Object:
		for _, member := range value.Members() {
			switch member.Name {
			case "$dynamicRef", "$recursiveRef":
				return fmt.Errorf("%s is forbidden", member.Name)
			case "$ref":
				reference, ok := member.Value.Text()
				if !ok || !strings.HasPrefix(reference, "#") {
					return fmt.Errorf("$ref must be a same-document fragment")
				}
			}
			if err := auditSchema(member.Value); err != nil {
				return err
			}
		}
	case jsondocument.Array:
		for _, child := range value.Elements() {
			if err := auditSchema(child); err != nil {
				return err
			}
		}
	}
	return nil
}

type denyLoader struct{}

func (denyLoader) Load(url string) (any, error) {
	return nil, fmt.Errorf("external schema load denied: %s", url)
}

func compileSchema(data []byte) (*jsonschema.Schema, error) {
	document, err := jsonschema.UnmarshalJSON(bytes.NewReader(data))
	if err != nil {
		return nil, err
	}
	compiler := jsonschema.NewCompiler()
	compiler.DefaultDraft(jsonschema.Draft2020)
	compiler.UseLoader(denyLoader{})
	const resource = "urn:trpg-platform:package-extension:local"
	if err := compiler.AddResource(resource, document); err != nil {
		return nil, err
	}
	return compiler.Compile(resource)
}

func clone(value []byte) []byte { return append([]byte(nil), value...) }
