// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package room

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

type emptyStorage struct{ Storage }

func (emptyStorage) Bind(core.Transaction) (Transaction, error) { return nil, auth.ErrDenied }

type emptyAuth struct{ auth.Repository }

func (emptyAuth) Transact(context.Context, func(auth.Transaction) error) error {
	return auth.ErrUnavailable
}
func contractService(t *testing.T) *Service {
	t.Helper()
	key := bytes.Repeat([]byte{3}, 32)
	store := emptyStorage{}
	verifier, e := NewAdmissionVerifier(store, key)
	if e != nil {
		t.Fatal("verifier construction")
	}
	as, e := os.ReadFile("../../../schemas/platform/platform-auth-api-v1.schema.json")
	if e != nil {
		t.Fatal("auth Schema read")
	}
	a, e := auth.NewRoomAuthority(emptyAuth{}, bytes.Repeat([]byte{1}, 32), bytes.Repeat([]byte{2}, 32), key, as, verifier, verifier)
	if e != nil {
		t.Fatal("authority construction")
	}
	schema, e := os.ReadFile("../../../schemas/platform/platform-room-api-v1.schema.json")
	if e != nil {
		t.Fatal("room Schema read")
	}
	s, e := NewService(a, store, key, schema)
	if e != nil {
		t.Fatal("room service construction")
	}
	return s
}
func TestStrictRoomRequests(t *testing.T) {
	s := contractService(t)
	bad := []string{
		`{"schema_version":1,"name":"room","game_id":"g","owner_account_id":"admin"}`,
		`{"schema_version":1,"schema_version":1,"name":"room","game_id":"g"}`,
		`{"schema_version":2,"name":"room","game_id":"g"}`,
		`{"schema_version":1,"name":"room","game_id":"g"} {}`,
		`{"schema_version":1,"name":"\ud800","game_id":"g"}`,
		`{"schema_version":1,"name":" ","game_id":"g"}`,
		`{"schema_version":1,"name":"room\n","game_id":"g"}`,
		`{"schema_version":1,"name":"room","game_id":"../g"}`,
		string([]byte{'{', '"', 0xff, '"', ':', '1', '}'}),
		strings.Repeat("[", 34) + "0" + strings.Repeat("]", 34),
		strings.Repeat(" ", 16385),
	}
	for i, b := range bad {
		if _, e := s.Decode("create_room", "w", "", "", []byte(b)); e != auth.ErrInvalid {
			t.Fatalf("invalid sample %d accepted", i)
		}
	}
	for _, body := range []string{`{"schema_version":1,"name":"大厅","game_id":"g"}`, `{"schema_version":1.0,"name":"大厅","game_id":"g"}`, `{"schema_version":1e0,"name":"大厅","game_id":"g"}`} {
		r, e := s.Decode("create_room", "w", "", "", []byte(body))
		if e != nil || r.StorageValue().Fields["schema_version"] != int64(1) {
			t.Fatal("integer normalization failed")
		}
	}
}
func TestRoomCanonicalIntegerEquivalence(t *testing.T) {
	s := contractService(t)
	var first []byte
	for _, n := range []string{"1", "1.0", "1e0"} {
		r, e := s.Decode("create_invite", "w", "r", "", []byte(`{"schema_version":`+n+`,"approval_required":true,"expires_in_seconds":60.0,"max_uses":`+n+`}`))
		if e != nil {
			t.Fatal("valid numeric sample denied")
		}
		b, e := json.Marshal(r.StorageValue())
		if e != nil {
			t.Fatal("canonical serialization")
		}
		if first == nil {
			first = b
		} else if !bytes.Equal(first, b) {
			t.Fatal("equivalent integers differ")
		}
	}
}
func TestRoomSchemaPinAndFormat(t *testing.T) {
	s := contractService(t)
	b, e := os.ReadFile("../../../schemas/platform/platform-room-api-v1.schema.json")
	if e != nil {
		t.Fatal("Schema read")
	}
	mutated := bytes.Replace(b, []byte(`"maxItems": 64`), []byte(`"maxItems": 65`), 1)
	if bytes.Equal(b, mutated) {
		t.Fatal("mutation fixture did not change")
	}
	if _, e = compile(mutated); e != auth.ErrInvalid {
		t.Fatal("Schema drift accepted")
	}
	var v any
	if e = json.Unmarshal(b, &v); e != nil {
		t.Fatal("Schema parse")
	}
	compact, e := json.Marshal(v)
	if e != nil {
		t.Fatal("Schema compact")
	}
	if _, e = compile(compact); e != nil {
		t.Fatal("equivalent Schema formatting denied")
	}
	if s.state().schemas["Invitation"].Validate(map[string]any{"invitation_id": "i", "approval_required": false, "expires_at": "invalid", "max_uses": 1, "remaining_uses": 1, "revoked": false}) == nil {
		t.Fatal("date-time format not enforced")
	}
}
func TestRoomTokensAndPrivateHandles(t *testing.T) {
	s := contractService(t)
	seen := map[string]bool{}
	for i := 0; i < 256; i++ {
		token, e := randomToken()
		if e != nil || !linkToken.MatchString(token) || seen[token] {
			t.Fatal("token entropy/shape")
		}
		seen[token] = true
		code, e := randomCode()
		if e != nil || !roomCode.MatchString(code) || seen[code] {
			t.Fatal("code entropy/shape")
		}
		seen[code] = true
		id, e := randomID()
		if e != nil || !identifier.MatchString(id) {
			t.Fatal("server identifier shape")
		}
		if s.hash("link", token) == token || s.hash("link", token) == s.hash("admission", token) {
			t.Fatal("secret digest domain separation")
		}
	}
	marker := "synthetic-private-marker"
	r := auth.RoomSecret(RequestData{Fields: map[string]any{"invite_token": marker}})
	inv := auth.RoomSecret(InvitationData{LinkHash: marker})
	adm := auth.RoomSecret(AdmissionData{OwnerSession: marker})
	out := auth.RoomOutcome([]byte(marker))
	for _, v := range []any{s, s.state().authority, r, inv, adm, out} {
		for _, verb := range []string{"%v", "%+v", "%#v", "%s", "%q", "%d", "%x", "%!"} {
			if strings.Contains(fmt.Sprintf(verb, v), marker) {
				t.Fatal("ordinary formatting leaked private bytes")
			}
		}
		if b, e := json.Marshal(v); e == nil || bytes.Contains(b, []byte(marker)) {
			t.Fatal("ordinary export accepted")
		}
	}
	if AuthorizePrivateView() != auth.ErrDenied || AuthorizeSeatControl() != auth.ErrDenied {
		t.Fatal("lobby acquired game capability")
	}
}
func TestApprovedAccountNicknameCompatibility(t *testing.T) {
	s := contractService(t)
	for _, n := range []int{80, 81, 128, 129} {
		v := map[string]any{"participant_id": "p", "display_name": strings.Repeat("N", n), "kind": "account", "roles": []any{}}
		if (s.state().schemas["Participant"].Validate(v) == nil) != (n <= 128) {
			t.Fatalf("account nickname boundary %d", n)
		}
		v["kind"] = "guest"
		if (s.state().schemas["Participant"].Validate(v) == nil) != (n <= 80) {
			t.Fatalf("guest nickname boundary %d", n)
		}
	}
	if _, e := s.Decode("create_room", "w", "", "", []byte(`{"schema_version":1,"name":"`+strings.Repeat("N", 81)+`","game_id":"g"}`)); e != auth.ErrInvalid {
		t.Fatal("room name bound changed")
	}
}
