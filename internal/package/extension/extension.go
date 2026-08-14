// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package extension implements the generic, package-ID-independent
// namespaced JSON extension contract.
package extension

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"net/url"
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
	MaxSchemaNodes           = 4096
	MaxSchemaEdges           = 16384
	MaxSchemaLocationBytes   = 1024
	MaxSchemaExpansion       = 8192
	MaxSchemaPatterns        = 256
	MaxSchemaValidationSteps = 64 << 10
	MaxSchemaValidationWork  = 8 << 20
)

var namespacePattern = regexp.MustCompile(`^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?){2,7}$`)
var anchorPattern = regexp.MustCompile(`^[A-Za-z_][-A-Za-z0-9._]*$`)

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
	Descriptor     Descriptor `json:"descriptor"`
	Status         Status     `json:"status"`
	ReadOnlyReason string     `json:"read_only_reason,omitempty"`
	schemaRaw      []byte
	payloadRaw     []byte
	canonical      []byte
	payload        jsondocument.Value
	compiled       *jsonschema.Schema
	audit          schemaAudit
}

// SchemaBytes returns a defensive copy of the exact referenced schema bytes.
func (document Document) SchemaBytes() []byte { return clone(document.schemaRaw) }

// PayloadBytes returns a defensive copy of the exact imported payload bytes.
func (document Document) PayloadBytes() []byte { return clone(document.payloadRaw) }

// CanonicalPayload returns a defensive copy of the supported payload's
// canonical representation. Read-only documents return nil.
func (document Document) CanonicalPayload() []byte { return clone(document.canonical) }

// PayloadValue returns the immutable strict value for supported documents.
func (document Document) PayloadValue() (jsondocument.Value, bool) {
	return document.payload, document.Status == Supported && document.compiled != nil
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
	schema, schemaExists := files[value.SchemaPath]
	if schemaExists && len(schema) > MaxSchemaBytes {
		return Document{}, contractError(ErrInvalid, value.Namespace, "schema exceeds %d bytes", MaxSchemaBytes)
	}
	document := Document{Descriptor: value, schemaRaw: clone(schema), payloadRaw: clone(payload)}
	if !schemaExists {
		return unsupported(document, value.Required, ErrSchemaMissing, "schema is absent")
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
		return unsupported(document, value.Required, ErrRequiredUnsupported, "schema JSON is unavailable")
	}
	dialect, exists := parsedSchema.Lookup("$schema")
	dialectText, isText := dialect.Text()
	if !exists || !isText || dialectText != "https://json-schema.org/draft/2020-12/schema" {
		return unsupported(document, value.Required, ErrRequiredUnsupported, "schema dialect is unsupported")
	}
	audit, err := auditSchema(parsedSchema)
	if err != nil {
		if errors.Is(err, errSchemaUnavailable) {
			return unsupported(document, value.Required, ErrRequiredUnsupported, "schema is unavailable: "+boundedReason(err.Error()))
		}
		return Document{}, contractError(ErrInvalid, value.Namespace, "schema reference policy: %v", err)
	}
	compiled, err := compileSchema(schema)
	if err != nil {
		return unsupported(document, value.Required, ErrRequiredUnsupported, "schema compilation is unavailable")
	}
	document.compiled = compiled
	document.audit = audit
	document.Status = Supported
	parsedPayload, err := jsondocument.Parse(payload)
	if err != nil {
		return Document{}, contractError(ErrInvalid, value.Namespace, "payload JSON: %v", err)
	}
	if err := document.audit.validatePayloadShape(parsedPayload); err != nil {
		return Document{}, contractError(ErrInvalid, value.Namespace, "schema validation budget: %v", err)
	}
	canonical, err := parsedPayload.CanonicalLimited(MaxPayloadBytes)
	if err != nil {
		return Document{}, contractError(ErrInvalid, value.Namespace, "canonical payload: %v", err)
	}
	document.payload = parsedPayload
	document.canonical = canonical
	if err := document.validateValue(parsedPayload, len(canonical)); err != nil {
		return Document{}, err
	}
	return document, nil
}

func unsupported(document Document, required bool, _ ErrorCode, reason string) (Document, error) {
	reason = boundedReason(reason)
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
	if err := document.audit.validatePayloadShape(value); err != nil {
		return nil, contractError(ErrInvalid, document.Descriptor.Namespace, "schema validation budget: %v", err)
	}
	canonical, err := value.CanonicalLimited(MaxPayloadBytes)
	if err != nil {
		return nil, contractError(ErrInvalid, document.Descriptor.Namespace, "canonical payload: %v", err)
	}
	if err := document.validateValue(value, len(canonical)); err != nil {
		return nil, err
	}
	return canonical, nil
}

func (document Document) validateValue(value jsondocument.Value, canonicalBytes int) error {
	if err := document.audit.validatePayloadBytes(canonicalBytes); err != nil {
		return contractError(ErrInvalid, document.Descriptor.Namespace, "schema validation budget: %v", err)
	}
	if err := document.compiled.Validate(value.Interface()); err != nil {
		return contractError(ErrSchemaValidation, document.Descriptor.Namespace, "%s", boundedValidationDiagnostic(err))
	}
	return nil
}

var errSchemaUnavailable = errors.New("schema unavailable")

type schemaReference struct {
	source    string
	reference string
}

type schemaAudit struct {
	expansion    int
	patternCount int
}

type schemaGraphBuilder struct {
	nodes        map[string]struct{}
	edges        map[string][]string
	anchors      map[string]string
	references   []schemaReference
	edgeCount    int
	patternCount int
}

// auditSchema walks only positions that Draft 2020-12 defines as schemas.
// Values under const/examples and property names that happen to equal "$ref"
// remain ordinary data. The resulting applicator/$ref graph is cycle checked
// and assigned a bounded expansion cost before the third-party compiler runs.
func auditSchema(value jsondocument.Value) (schemaAudit, error) {
	builder := schemaGraphBuilder{
		nodes:   make(map[string]struct{}),
		edges:   make(map[string][]string),
		anchors: make(map[string]string),
	}
	if err := builder.visitSchema(value, "#"); err != nil {
		return schemaAudit{}, err
	}
	for _, pending := range builder.references {
		target, err := builder.resolve(pending.reference)
		if err != nil {
			return schemaAudit{}, fmt.Errorf("%w: local reference cannot be resolved: %v", errSchemaUnavailable, err)
		}
		if err := builder.addEdge(pending.source, target); err != nil {
			return schemaAudit{}, err
		}
	}
	expansion, err := boundedAcyclicExpansion(builder.nodes, builder.edges)
	if err != nil {
		return schemaAudit{}, err
	}
	return schemaAudit{expansion: expansion, patternCount: builder.patternCount}, nil
}

func (builder *schemaGraphBuilder) visitSchema(value jsondocument.Value, pointer string) error {
	if value.Kind() == jsondocument.Boolean {
		if len(builder.nodes) >= MaxSchemaNodes {
			return fmt.Errorf("%w: schema exceeds %d subschemas", errSchemaUnavailable, MaxSchemaNodes)
		}
		builder.nodes[pointer] = struct{}{}
		return nil
	}
	if value.Kind() != jsondocument.Object {
		return fmt.Errorf("%w: subschema is neither object nor boolean", errSchemaUnavailable)
	}
	if _, exists := builder.nodes[pointer]; exists {
		return nil
	}
	if len(builder.nodes) >= MaxSchemaNodes {
		return fmt.Errorf("%w: schema exceeds %d subschemas", errSchemaUnavailable, MaxSchemaNodes)
	}
	builder.nodes[pointer] = struct{}{}
	members := make(map[string]jsondocument.Value)
	for _, member := range value.Members() {
		members[member.Name] = member.Value
	}
	if identifier, exists := members["$id"]; exists {
		if _, ok := identifier.Text(); !ok {
			return fmt.Errorf("%w: $id must be a string", errSchemaUnavailable)
		}
		if pointer != "#" {
			return fmt.Errorf("%w: nested $id changes local reference scope", errSchemaUnavailable)
		}
	}
	for _, keyword := range []string{"$dynamicRef", "$recursiveRef"} {
		if _, exists := members[keyword]; exists {
			return fmt.Errorf("%s is forbidden", keyword)
		}
	}
	if referenceValue, exists := members["$ref"]; exists {
		reference, ok := referenceValue.Text()
		if !ok || !strings.HasPrefix(reference, "#") {
			return fmt.Errorf("$ref must be a same-document fragment")
		}
		if len(reference) > 1+3*MaxSchemaLocationBytes {
			return fmt.Errorf("%w: local reference location is too long", errSchemaUnavailable)
		}
		builder.references = append(builder.references, schemaReference{source: pointer, reference: reference})
	}
	if _, exists := members["pattern"]; exists {
		builder.patternCount++
		if builder.patternCount > MaxSchemaPatterns {
			return fmt.Errorf("%w: schema exceeds %d regular-expression assertions", errSchemaUnavailable, MaxSchemaPatterns)
		}
	}
	for _, keyword := range []string{"$anchor", "$dynamicAnchor"} {
		anchorValue, exists := members[keyword]
		if !exists {
			continue
		}
		anchor, ok := anchorValue.Text()
		if !ok || len(anchor)+1 > MaxSchemaLocationBytes || !anchorPattern.MatchString(anchor) {
			return fmt.Errorf("%w: %s is not a valid anchor", errSchemaUnavailable, keyword)
		}
		if previous, duplicate := builder.anchors[anchor]; duplicate && previous != pointer {
			return fmt.Errorf("%w: duplicate schema anchor", errSchemaUnavailable)
		}
		builder.anchors[anchor] = pointer
	}
	for _, keyword := range []string{
		"additionalProperties", "contains", "contentSchema", "else", "if", "items", "not",
		"propertyNames", "then", "unevaluatedItems", "unevaluatedProperties",
	} {
		if child, exists := members[keyword]; exists {
			childPointer, err := schemaPointerChild(pointer, keyword)
			if err != nil {
				return err
			}
			if err := builder.visitSchema(child, childPointer); err != nil {
				return err
			}
			if child.Kind() == jsondocument.Object || child.Kind() == jsondocument.Boolean {
				if err := builder.addEdge(pointer, childPointer); err != nil {
					return err
				}
			}
		}
	}
	for _, keyword := range []string{"allOf", "anyOf", "oneOf", "prefixItems"} {
		collection, exists := members[keyword]
		if !exists {
			continue
		}
		if collection.Kind() != jsondocument.Array {
			return fmt.Errorf("%w: %s must contain subschemas", errSchemaUnavailable, keyword)
		}
		for index, child := range collection.Elements() {
			collectionPointer, err := schemaPointerChild(pointer, keyword)
			if err != nil {
				return err
			}
			childPointer, err := schemaPointerChild(collectionPointer, fmt.Sprint(index))
			if err != nil {
				return err
			}
			if err := builder.visitSchema(child, childPointer); err != nil {
				return err
			}
			if child.Kind() == jsondocument.Object || child.Kind() == jsondocument.Boolean {
				if err := builder.addEdge(pointer, childPointer); err != nil {
					return err
				}
			}
		}
	}
	for _, keyword := range []string{"$defs", "definitions", "dependentSchemas", "patternProperties", "properties"} {
		collection, exists := members[keyword]
		if !exists {
			continue
		}
		if collection.Kind() != jsondocument.Object {
			return fmt.Errorf("%w: %s must be an object of subschemas", errSchemaUnavailable, keyword)
		}
		entries := collection.Members()
		if keyword == "patternProperties" {
			builder.patternCount += len(entries)
			if builder.patternCount > MaxSchemaPatterns {
				return fmt.Errorf("%w: schema exceeds %d patternProperties", errSchemaUnavailable, MaxSchemaPatterns)
			}
		}
		for _, entry := range entries {
			collectionPointer, err := schemaPointerChild(pointer, keyword)
			if err != nil {
				return err
			}
			childPointer, err := schemaPointerChild(collectionPointer, entry.Name)
			if err != nil {
				return err
			}
			if err := builder.visitSchema(entry.Value, childPointer); err != nil {
				return err
			}
			if keyword != "$defs" && keyword != "definitions" && (entry.Value.Kind() == jsondocument.Object || entry.Value.Kind() == jsondocument.Boolean) {
				if err := builder.addEdge(pointer, childPointer); err != nil {
					return err
				}
			}
		}
	}
	return nil
}

func (builder *schemaGraphBuilder) addEdge(source, target string) error {
	if builder.edgeCount >= MaxSchemaEdges {
		return fmt.Errorf("%w: schema exceeds %d evaluation edges", errSchemaUnavailable, MaxSchemaEdges)
	}
	builder.edges[source] = append(builder.edges[source], target)
	builder.edgeCount++
	return nil
}

func (builder *schemaGraphBuilder) resolve(reference string) (string, error) {
	if len(reference) > 1+3*MaxSchemaLocationBytes {
		return "", fmt.Errorf("local reference location is too long")
	}
	fragment, err := url.PathUnescape(strings.TrimPrefix(reference, "#"))
	if err != nil {
		return "", fmt.Errorf("invalid URI fragment: %w", err)
	}
	if fragment == "" {
		return "#", nil
	}
	if !strings.HasPrefix(fragment, "/") {
		pointer, exists := builder.anchors[fragment]
		if !exists {
			return "", fmt.Errorf("unknown local anchor")
		}
		return pointer, nil
	}
	pointer := "#"
	for _, encoded := range strings.Split(fragment[1:], "/") {
		name, err := unescapePointerToken(encoded)
		if err != nil {
			return "", err
		}
		pointer, err = schemaPointerChild(pointer, name)
		if err != nil {
			return "", err
		}
	}
	if _, exists := builder.nodes[pointer]; !exists {
		return "", fmt.Errorf("local fragment does not identify a subschema")
	}
	return pointer, nil
}

func pointerChild(pointer, name string) string {
	name = strings.ReplaceAll(name, "~", "~0")
	name = strings.ReplaceAll(name, "/", "~1")
	return pointer + "/" + name
}

func schemaPointerChild(pointer, name string) (string, error) {
	escapedLength := len(name)
	for index := 0; index < len(name); index++ {
		if name[index] == '~' || name[index] == '/' {
			escapedLength++
		}
	}
	if len(pointer)+1+escapedLength > MaxSchemaLocationBytes {
		return "", fmt.Errorf("%w: schema location exceeds %d bytes", errSchemaUnavailable, MaxSchemaLocationBytes)
	}
	return pointerChild(pointer, name), nil
}

func unescapePointerToken(value string) (string, error) {
	var result strings.Builder
	for index := 0; index < len(value); index++ {
		if value[index] != '~' {
			result.WriteByte(value[index])
			continue
		}
		if index+1 >= len(value) || (value[index+1] != '0' && value[index+1] != '1') {
			return "", fmt.Errorf("invalid JSON Pointer escape")
		}
		index++
		if value[index] == '0' {
			result.WriteByte('~')
		} else {
			result.WriteByte('/')
		}
	}
	return result.String(), nil
}

func boundedAcyclicExpansion(nodes map[string]struct{}, edges map[string][]string) (int, error) {
	indegree := make(map[string]int, len(nodes))
	for node := range nodes {
		indegree[node] = 0
	}
	for _, targets := range edges {
		for _, target := range targets {
			if _, exists := nodes[target]; !exists {
				return 0, fmt.Errorf("%w: reference target is not a subschema", errSchemaUnavailable)
			}
			indegree[target]++
		}
	}
	queue := make([]string, 0, len(nodes))
	for node, count := range indegree {
		if count == 0 {
			queue = append(queue, node)
		}
	}
	order := make([]string, 0, len(nodes))
	for head := 0; head < len(queue); head++ {
		node := queue[head]
		order = append(order, node)
		for _, target := range edges[node] {
			indegree[target]--
			if indegree[target] == 0 {
				queue = append(queue, target)
			}
		}
	}
	if len(order) != len(nodes) {
		return 0, fmt.Errorf("%w: cyclic local $ref/applicator graph", errSchemaUnavailable)
	}
	cost := make(map[string]int, len(nodes))
	maximum := 1
	for index := len(order) - 1; index >= 0; index-- {
		node := order[index]
		value := 1
		for _, target := range edges[node] {
			if cost[target] > MaxSchemaExpansion-value {
				return 0, fmt.Errorf("%w: schema evaluation expansion exceeds %d", errSchemaUnavailable, MaxSchemaExpansion)
			}
			value += cost[target]
		}
		cost[node] = value
		if value > maximum {
			maximum = value
		}
	}
	return maximum, nil
}

func (audit schemaAudit) validatePayloadShape(value jsondocument.Value) error {
	metrics := value.Complexity()
	factor := audit.expansion
	if factor < 1 {
		factor = 1
	}
	if metrics.Nodes > MaxSchemaValidationSteps/factor {
		return fmt.Errorf("schema/payload evaluation exceeds %d steps", MaxSchemaValidationSteps)
	}
	if audit.patternCount != 0 && metrics.ObjectMembers > MaxSchemaValidationSteps/audit.patternCount {
		return fmt.Errorf("pattern/property evaluation exceeds %d steps", MaxSchemaValidationSteps)
	}
	return nil
}

func (audit schemaAudit) validatePayloadBytes(canonicalBytes int) error {
	factor := audit.expansion
	if factor < 1 {
		factor = 1
	}
	if canonicalBytes > MaxSchemaValidationWork/factor {
		return fmt.Errorf("schema/payload byte evaluation exceeds %d work units", MaxSchemaValidationWork)
	}
	if audit.patternCount != 0 && canonicalBytes > MaxSchemaValidationWork/audit.patternCount {
		return fmt.Errorf("pattern/property byte evaluation exceeds %d work units", MaxSchemaValidationWork)
	}
	return nil
}

func boundedValidationDiagnostic(err error) string {
	const maxLocationBytes = 160
	var validation *jsonschema.ValidationError
	if !errors.As(err, &validation) {
		return "payload does not satisfy the declared schema"
	}
	leaf := validation
	for depth := 0; depth < 16 && len(leaf.Causes) != 0; depth++ {
		leaf = leaf.Causes[0]
	}
	instance := boundedPointer(leaf.InstanceLocation, maxLocationBytes)
	keyword := "#unknown"
	if leaf.ErrorKind != nil {
		keyword = boundedPointer(leaf.ErrorKind.KeywordPath(), maxLocationBytes)
	}
	return fmt.Sprintf(
		"payload does not satisfy the declared schema (instance=%s keyword=%s)",
		instance, keyword,
	)
}

func boundedPointer(tokens []string, limit int) string {
	var result strings.Builder
	result.Grow(limit + 3)
	result.WriteByte('#')
	truncated := false
	for _, token := range tokens {
		if result.Len()+1 > limit {
			truncated = true
			break
		}
		result.WriteByte('/')
		for _, character := range token {
			var addition string
			switch character {
			case '~':
				addition = "~0"
			case '/':
				addition = "~1"
			default:
				addition = string(character)
			}
			if result.Len()+len(addition) > limit {
				truncated = true
				break
			}
			result.WriteString(addition)
		}
		if truncated {
			break
		}
	}
	if truncated {
		result.WriteString("...")
	}
	return result.String()
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

func boundedReason(value string) string {
	const limit = 512
	if len(value) <= limit {
		return value
	}
	var result strings.Builder
	result.Grow(limit + 3)
	for _, character := range value {
		width := len(string(character))
		if result.Len()+width > limit {
			break
		}
		result.WriteRune(character)
	}
	result.WriteString("...")
	return result.String()
}

func clone(value []byte) []byte { return append([]byte(nil), value...) }
