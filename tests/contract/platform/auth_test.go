// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package platform_test

import (
	"encoding/json"
	"os"
	"testing"

	"github.com/santhosh-tekuri/jsonschema/v6"
)

type offline struct{}

func (offline) Load(string) (any, error) { return nil, os.ErrPermission }
func TestApprovedPlatformSchemasCompileAndValidateResponses(t *testing.T) {
	data, e := os.ReadFile("../../../schemas/platform/platform-auth-api-v1.schema.json")
	if e != nil {
		t.Fatal("approved schema absent")
	}
	var schema map[string]any
	if json.Unmarshal(data, &schema) != nil {
		t.Fatal("invalid schema")
	}
	c := jsonschema.NewCompiler()
	c.DefaultDraft(jsonschema.Draft2020)
	c.AssertFormat()
	c.UseLoader(offline{})
	uri := "urn:trpg-platform:platform-auth-api:v1"
	if e := c.AddResource(uri, schema); e != nil {
		t.Fatal("invalid schema resource")
	}
	defs := schema["$defs"].(map[string]any)
	if len(defs) != 26 {
		t.Fatal("approved definition inventory changed")
	}
	for name := range defs {
		if _, e := c.Compile(uri + "#/$defs/" + name); e != nil {
			t.Fatal("definition does not compile")
		}
	}
	account := map[string]any{"state": "authenticated", "principal": map[string]any{"kind": "account", "account_id": "local", "display_name": "玩家"}, "csrf_token": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "expires_at": "2026-10-07T05:00:00Z"}
	scope := map[string]any{"workspace_id": "workspace", "room_id": "room", "game_id": "game"}
	guest := map[string]any{"state": "authenticated", "principal": map[string]any{"kind": "guest", "participation_id": "participant", "scope": scope}, "csrf_token": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "expires_at": "2026-10-07T05:00:00Z"}
	for name, v := range map[string]any{
		"ContextResponse":        map[string]any{"state": "anonymous", "csrf_token": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "csrf_expires_at": "2026-10-07T05:00:00Z"},
		"AccountContextResponse": account, "GuestContextResponse": guest,
		"ClaimResponse":      map[string]any{"context": account, "participation": map[string]any{"participation_id": "participant", "scope": scope}},
		"WorkspaceResponse":  map[string]any{"workspace_id": "workspace", "name": "工作区", "owner_account_id": "local"},
		"MembershipResponse": map[string]any{"workspace_id": "workspace", "account_id": "local", "role": "owner"},
		"RemovalResponse":    map[string]any{"removed": true},
	} {
		t.Run(name, func(t *testing.T) {
			s, e := c.Compile(uri + "#/$defs/" + name)
			if e != nil {
				t.Fatal("invalid definition")
			}
			envelope := map[string]any{"schema_version": 1, "request_id": "request", "data": v}
			if e := s.Validate(envelope); e != nil {
				t.Fatal("approved response rejected")
			}
			envelope["session_token"] = "RAW_TOKEN_MARKER"
			if e := s.Validate(envelope); e == nil {
				t.Fatal("body credential accepted")
			}
		})
	}
	health, _ := c.Compile(uri + "#/$defs/HealthResponse")
	if e := health.Validate(map[string]any{"schema_version": 1, "status": "ready", "dsn": "private"}); e == nil {
		t.Fatal("health diagnostic accepted")
	}
	errorSchema, _ := c.Compile(uri + "#/$defs/ErrorResponse")
	for _, code := range []string{"INVALID_REQUEST", "UNAUTHENTICATED", "DENIED", "CONFLICT", "CLAIM_REQUIRED", "RATE_LIMITED", "UNAVAILABLE", "OUTCOME_UNKNOWN"} {
		if e := errorSchema.Validate(map[string]any{"schema_version": 1, "request_id": "request", "error": map[string]any{"code": code, "message": "Safe message"}}); e != nil {
			t.Fatal("approved error rejected")
		}
	}
}
