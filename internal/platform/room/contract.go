// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package room

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"math/big"
	"regexp"

	"github.com/santhosh-tekuri/jsonschema/v6"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

var identifier = regexp.MustCompile(`^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$`)
var linkToken = regexp.MustCompile(`^[A-Za-z0-9_-]{43}$`)
var roomCode = regexp.MustCompile(`^[0123456789ABCDEFGHJKMNPQRSTVWXYZ]{16}$`)
var requestSchemas = map[string]string{
	"create_room": "CreateRoomRequest", "create_invite": "CreateInvitationRequest",
	"close_room": "VersionOnlyRequest", "revoke_invite": "VersionOnlyRequest",
	"request_admission": "AdmissionRequest", "guest_token": "VersionOnlyRequest",
	"decide_admission": "DecisionRequest", "leave_room": "VersionOnlyRequest",
	"kick_participant": "VersionOnlyRequest", "set_role": "SetRoomRoleRequest",
}
var responseSchemas = map[string]string{
	"create_room": "RoomResponse", "get_room": "RoomResponse",
	"create_invite": "InvitationResponse", "request_admission": "AdmissionResponse",
	"get_admission": "AdmissionResponse", "guest_token": "GuestTokenResponse",
	"list_admissions": "AdmissionQueueResponse", "decide_admission": "AdmissionResponse",
	"close_room": "AppliedResponse", "revoke_invite": "AppliedResponse",
	"leave_room": "AppliedResponse", "kick_participant": "AppliedResponse", "set_role": "AppliedResponse",
}

type denyLoader struct{}

func (denyLoader) Load(string) (any, error) { return nil, auth.ErrDenied }
func compile(data []byte) (map[string]*jsonschema.Schema, error) {
	// Schema is operator supplied, with a separate bound from public request JSON.
	if len(data) > 65536 {
		return nil, auth.ErrInvalid
	}
	v, e := auth.RoomContractJSON(data)
	if e != nil {
		return nil, e
	}
	m, ok := v.(map[string]any)
	if !ok || m["$id"] != "urn:trpg-platform:platform-room-api:v1" || m["x-section-id"] != "SCHEMA-PLATFORM-ROOM-API-V1" || m["x-status"] != "ACTIVE" || m["$schema"] != "https://json-schema.org/draft/2020-12/schema" {
		return nil, auth.ErrInvalid
	}
	b, e := json.Marshal(m)
	if e != nil {
		return nil, auth.ErrInvalid
	}
	h := sha256.Sum256(b)
	if hex.EncodeToString(h[:]) != "4871e74539499c68a28c81fddcae3ddff5e929346f3c40dd19ab101103d19b51" {
		return nil, auth.ErrInvalid
	}
	c := jsonschema.NewCompiler()
	c.DefaultDraft(jsonschema.Draft2020)
	c.AssertFormat()
	c.UseLoader(denyLoader{})
	if e = c.AddResource("urn:trpg-platform:platform-room-api:v1", v); e != nil {
		return nil, auth.ErrInvalid
	}
	defs, ok := m["$defs"].(map[string]any)
	if !ok || len(defs) != 25 {
		return nil, auth.ErrInvalid
	}
	out := map[string]*jsonschema.Schema{}
	for name := range defs {
		if !identifier.MatchString(name) {
			return nil, auth.ErrInvalid
		}
		s, e := c.Compile("urn:trpg-platform:platform-room-api:v1#/$defs/" + name)
		if e != nil {
			return nil, auth.ErrInvalid
		}
		out[name] = s
	}
	return out, nil
}

func (s *Service) Decode(action, workspace, room, resource string, body []byte) (Request, error) {
	if s.state() == nil {
		return Request{}, auth.ErrUnavailable
	}
	if responseSchemas[action] == "" || workspace != "" && !identifier.MatchString(workspace) || room != "" && !identifier.MatchString(room) || resource != "" && !identifier.MatchString(resource) {
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
		for _, key := range []string{"schema_version", "expires_in_seconds", "max_uses"} {
			if n, ok := fields[key].(json.Number); ok {
				r, ok := new(big.Rat).SetString(string(n))
				if !ok || !r.IsInt() || !r.Num().IsInt64() {
					return Request{}, auth.ErrInvalid
				}
				fields[key] = r.Num().Int64()
			}
		}
	} else if len(body) != 0 {
		return Request{}, auth.ErrInvalid
	}
	switch action {
	case "request_admission":
		if workspace != "" || room != "" || resource != "" {
			return Request{}, auth.ErrInvalid
		}
	case "get_admission", "guest_token":
		if workspace != "" || room != "" || resource == "" {
			return Request{}, auth.ErrInvalid
		}
	case "create_room":
		if workspace == "" || room != "" || resource != "" {
			return Request{}, auth.ErrInvalid
		}
	case "revoke_invite", "decide_admission", "kick_participant", "set_role":
		if workspace == "" || room == "" || resource == "" {
			return Request{}, auth.ErrInvalid
		}
	default:
		if workspace == "" || room == "" || resource != "" {
			return Request{}, auth.ErrInvalid
		}
	}
	return auth.RoomSecret(RequestData{Action: action, WorkspaceID: workspace, RoomID: room, ResourceID: resource, Fields: fields}), nil
}

func (s *Service) response(action string, data map[string]any) (auth.Outcome, error) {
	v := map[string]any{"schema_version": 1, "request_id": "response", "data": data}
	b, e := json.Marshal(v)
	if e != nil {
		return auth.Outcome{}, auth.ErrUnavailable
	}
	decoded, e := auth.RoomContractJSON(b)
	if e != nil {
		return auth.Outcome{}, auth.ErrUnavailable
	}
	name := responseSchemas[action]
	if name == "" || s.state().schemas[name].Validate(decoded) != nil {
		return auth.Outcome{}, auth.ErrUnavailable
	}
	return auth.RoomOutcome(b), nil
}
