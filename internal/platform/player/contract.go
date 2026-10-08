// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"math"
	"math/big"
	"slices"
	"strconv"

	"github.com/santhosh-tekuri/jsonschema/v6"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

const MaxRequestBytes = 16 << 10
const MaxResponseBytes = 256 << 10
const schemaID = "urn:trpg-platform:platform-player-api:v1"
const schemaDigest = "87a437710aa3cd153530897b33d8b19f0a6daa78e236487b4b66733ff620d7c9"

var requestSchemas = map[string]string{
	"configure": "ConfigureRequest", "consent": "AcknowledgeRequest", "launch": "LaunchRequest",
	"connect": "ConnectRequest", "snapshot": "SnapshotRequest", "command": "CommandRequest",
	"disconnect": "DisconnectRequest", "pause": "ControlRequest", "resume": "ControlRequest",
	"export": "ExportRequest", "create_point": "VersionOnlyRequest",
}
var responseSchemas = map[string]string{
	"catalog": "CatalogResponse", "lobby": "LobbyResponse", "readiness": "LobbyResponse", "configure": "LobbyResponse",
	"consent": "AppliedResponse", "launch": "LaunchResponse", "connect": "ConnectionResponse", "snapshot": "SnapshotResponse",
	"command": "CommandResponse", "disconnect": "ControlResponse", "pause": "ControlResponse", "resume": "ControlResponse",
	"export": "ExportResponse", "create_point": "RecoveryPointResponse", "read_point": "RecoveryPointResponse",
}

type denyLoader struct{}

func (denyLoader) Load(string) (any, error) { return nil, auth.ErrDenied }
func compile(raw []byte) (map[string]*jsonschema.Schema, error) {
	v, e := auth.RoomContractJSON(raw)
	if e != nil {
		return nil, auth.ErrInvalid
	}
	m, ok := v.(map[string]any)
	if !ok || m["$id"] != schemaID || m["$schema"] != "https://json-schema.org/draft/2020-12/schema" || m["x-status"] != "ACTIVE" || m["x-section-id"] != "SCHEMA-PLATFORM-PLAYER-API-V1" {
		return nil, auth.ErrInvalid
	}
	var b bytes.Buffer
	enc := json.NewEncoder(&b)
	enc.SetEscapeHTML(false)
	if enc.Encode(m) != nil {
		return nil, auth.ErrInvalid
	}
	h := sha256.Sum256(bytes.TrimSuffix(b.Bytes(), []byte("\n")))
	if hex.EncodeToString(h[:]) != schemaDigest {
		return nil, auth.ErrInvalid
	}
	defs, ok := m["$defs"].(map[string]any)
	if !ok || len(defs) != 39 {
		return nil, auth.ErrInvalid
	}
	c := jsonschema.NewCompiler()
	c.DefaultDraft(jsonschema.Draft2020)
	c.AssertFormat()
	c.UseLoader(denyLoader{})
	if c.AddResource(schemaID, v) != nil {
		return nil, auth.ErrInvalid
	}
	if _, e = c.Compile(schemaID); e != nil {
		return nil, auth.ErrInvalid
	}
	names := make([]string, 0, len(defs))
	for name := range defs {
		names = append(names, name)
	}
	slices.Sort(names)
	out := map[string]*jsonschema.Schema{}
	for _, name := range names {
		if !store.ValidID(name) {
			return nil, auth.ErrInvalid
		}
		s, e := c.Compile(schemaID + "#/$defs/" + name)
		if e != nil {
			return nil, auth.ErrInvalid
		}
		out[name] = s
	}
	return out, nil
}
func counter(s string) (uint64, error) {
	v, e := strconv.ParseUint(s, 10, 63)
	if e != nil || v >= math.MaxInt64 || strconv.FormatUint(v, 10) != s {
		return 0, auth.ErrInvalid
	}
	return v, nil
}
func validCounters(vs ...uint64) bool {
	for _, v := range vs {
		if v >= math.MaxInt64 {
			return false
		}
	}
	return true
}
func integer(v any) (int, error) {
	n, ok := v.(json.Number)
	if !ok {
		return 0, auth.ErrInvalid
	}
	r, ok := new(big.Rat).SetString(string(n))
	if !ok || !r.IsInt() || !r.Num().IsInt64() || r.Num().Int64() < 0 || r.Num().Int64() > 128 {
		return 0, auth.ErrInvalid
	}
	return int(r.Num().Int64()), nil
}

type RequestData struct {
	Action, WorkspaceID, RoomID string
	Fields                      map[string]any
	Canonical                   []byte
}
type Request = auth.Secret[RequestData]

func (RequestData) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private player request data>")
}

func (s *Service) Decode(action, w, r string, body []byte) (Request, error) {
	if s.state() == nil {
		return Request{}, auth.ErrUnavailable
	}
	if responseSchemas[action] == "" || !store.ValidID(w) || action == "catalog" && r != "" || action != "catalog" && !store.ValidID(r) {
		return Request{}, auth.ErrInvalid
	}
	fields := map[string]any{}
	if schema := requestSchemas[action]; schema != "" {
		v, e := auth.RoomJSON(body)
		if e != nil {
			return Request{}, auth.ErrInvalid
		}
		m, ok := v.(map[string]any)
		if !ok || s.state().schemas[schema].Validate(m) != nil {
			return Request{}, auth.ErrInvalid
		}
		fields = m
		for _, key := range []string{"revision", "after_cursor", "expected_state_version", "expected_control_revision"} {
			if v, ok := fields[key]; ok {
				x, ok := v.(string)
				if !ok {
					return Request{}, auth.ErrInvalid
				}
				if _, e = counter(x); e != nil {
					return Request{}, e
				}
			}
		}
		if raw, ok := fields["payload"]; ok {
			b, e := json.Marshal(raw)
			if e != nil {
				return Request{}, auth.ErrInvalid
			}
			var v checkpoint.Value
			if checkpoint.StrictDecode(b, &v, MaxRequestBytes) != nil || checkpoint.Validate(v) != nil {
				return Request{}, auth.ErrInvalid
			}
		}
	} else if len(body) != 0 {
		return Request{}, auth.ErrInvalid
	}
	b, e := auth.CanonicalRoomFields(fields)
	if e != nil {
		return Request{}, e
	}
	return auth.RoomSecret(RequestData{Action: action, WorkspaceID: w, RoomID: r, Fields: fields, Canonical: b}), nil
}
func (s *Service) response(action string, data any) (auth.Outcome, error) {
	v := map[string]any{"schema_version": 1, "request_id": "response", "data": data}
	b, e := json.Marshal(v)
	if e != nil || len(b) > MaxResponseBytes {
		return auth.Outcome{}, auth.ErrUnavailable
	}
	// Responses use their own byte bound, retain strict duplicate-key decoding,
	// and validate the exact approved definition before leaving the facade.
	var decoded any
	dec := json.NewDecoder(bytes.NewReader(b))
	dec.UseNumber()
	if dec.Decode(&decoded) != nil || s.state().schemas[responseSchemas[action]].Validate(decoded) != nil {
		return auth.Outcome{}, auth.ErrUnavailable
	}
	return auth.RoomOutcome(b), nil
}
