// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package auth

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"io"
	"regexp"
	"unicode/utf8"

	"github.com/santhosh-tekuri/jsonschema/v6"
)

var wireID = regexp.MustCompile(`^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$`)
var loginID = regexp.MustCompile(`^[a-z][a-z0-9_]{2,31}$`)
var tokenID = regexp.MustCompile(`^[A-Za-z0-9_-]{43}$`)
var idemID = regexp.MustCompile(`^[A-Za-z0-9][A-Za-z0-9_.-]{15,127}$`)

var requestSchemas = map[string]string{
	"register": "RegisterRequest", "login": "LoginRequest", "logout": "LogoutRequest",
	"exchange": "GuestExchangeRequest", "claim": "GuestClaimRequest", "create_workspace": "CreateWorkspaceRequest",
	"set_member": "SetMembershipRequest", "remove_member": "RemoveMembershipRequest",
}
var responseSchemas = map[string]string{
	"context": "ContextResponse", "register": "AccountContextResponse", "login": "AccountContextResponse",
	"logout": "ContextResponse", "exchange": "GuestContextResponse", "claim": "ClaimResponse",
	"create_workspace": "WorkspaceResponse", "workspace": "WorkspaceResponse", "set_member": "MembershipResponse", "remove_member": "RemovalResponse",
}

type deniedLoader struct{}

func (deniedLoader) Load(string) (any, error) { return nil, ErrDenied }

func compileContract(data []byte) (map[string]*jsonschema.Schema, error) {
	v, e := strictJSON(data)
	if e != nil {
		return nil, ErrInvalid
	}
	m, ok := v.(map[string]any)
	if !ok || m["$id"] != "urn:trpg-platform:platform-auth-api:v1" || m["x-section-id"] != "SCHEMA-PLATFORM-AUTH-API-V1" || m["x-status"] != "ACTIVE" || m["$schema"] != "https://json-schema.org/draft/2020-12/schema" {
		return nil, ErrInvalid
	}
	canonical, e := json.Marshal(m)
	if e != nil {
		return nil, ErrInvalid
	}
	hash := sha256.Sum256(canonical)
	if hex.EncodeToString(hash[:]) != "193cf911f09205612a21f20f284ef5a718ead78cf023a1b680318d7f2a2813cc" {
		return nil, ErrInvalid
	}
	c := jsonschema.NewCompiler()
	c.DefaultDraft(jsonschema.Draft2020)
	c.AssertFormat()
	c.UseLoader(deniedLoader{})
	if e := c.AddResource("urn:trpg-platform:platform-auth-api:v1", v); e != nil {
		return nil, ErrInvalid
	}
	defs, ok := m["$defs"].(map[string]any)
	if !ok {
		return nil, ErrInvalid
	}
	out := make(map[string]*jsonschema.Schema)
	for name := range defs {
		if !wireID.MatchString(name) {
			return nil, ErrInvalid
		}
		s, e := c.Compile("urn:trpg-platform:platform-auth-api:v1#/$defs/" + name)
		if e != nil {
			return nil, ErrInvalid
		}
		out[name] = s
	}
	for _, name := range requestSchemas {
		if out[name] == nil {
			return nil, ErrInvalid
		}
	}
	for _, name := range responseSchemas {
		if out[name] == nil {
			return nil, ErrInvalid
		}
	}
	if out["ErrorResponse"] == nil || out["HealthResponse"] == nil {
		return nil, ErrInvalid
	}
	return out, nil
}

// The token walk rejects duplicate names at every depth, invalid UTF-8 and
// trailing values before Schema validation; json.Unmarshal alone permits them.
func strictJSON(data []byte) (any, error) {
	if !utf8.Valid(data) || !validUnicodeEscapes(data) {
		return nil, ErrInvalid
	}
	d := json.NewDecoder(bytes.NewReader(data))
	d.UseNumber()
	var read func(int) (any, error)
	read = func(depth int) (any, error) {
		if depth > 32 {
			return nil, ErrInvalid
		}
		t, e := d.Token()
		if e != nil {
			return nil, ErrInvalid
		}
		switch t {
		case json.Delim('{'):
			m := make(map[string]any)
			for d.More() {
				k, e := d.Token()
				if e != nil {
					return nil, ErrInvalid
				}
				name, ok := k.(string)
				if !ok {
					return nil, ErrInvalid
				}
				if _, ok = m[name]; ok {
					return nil, ErrInvalid
				}
				v, e := read(depth + 1)
				if e != nil {
					return nil, e
				}
				m[name] = v
			}
			end, e := d.Token()
			if e != nil || end != json.Delim('}') {
				return nil, ErrInvalid
			}
			return m, nil
		case json.Delim('['):
			a := []any{}
			for d.More() {
				v, e := read(depth + 1)
				if e != nil {
					return nil, e
				}
				a = append(a, v)
			}
			end, e := d.Token()
			if e != nil || end != json.Delim(']') {
				return nil, ErrInvalid
			}
			return a, nil
		default:
			if _, ok := t.(json.Delim); ok {
				return nil, ErrInvalid
			}
			return t, nil
		}
	}
	v, e := read(0)
	if e != nil {
		return nil, e
	}
	if _, e = d.Token(); e != io.EOF {
		return nil, ErrInvalid
	}
	return v, nil
}

// encoding/json substitutes U+FFFD for unmatched escaped surrogates. Reject
// those sequences rather than silently changing a password's Unicode bytes.
func validUnicodeEscapes(data []byte) bool {
	quoted := false
	hex4 := func(at int) (uint16, bool) {
		if at+4 > len(data) {
			return 0, false
		}
		var value uint16
		for _, b := range data[at : at+4] {
			value <<= 4
			switch {
			case b >= '0' && b <= '9':
				value += uint16(b - '0')
			case b >= 'a' && b <= 'f':
				value += uint16(b - 'a' + 10)
			case b >= 'A' && b <= 'F':
				value += uint16(b - 'A' + 10)
			default:
				return 0, false
			}
		}
		return value, true
	}
	for i := 0; i < len(data); i++ {
		if data[i] == '"' {
			quoted = !quoted
			continue
		}
		if !quoted || data[i] != '\\' {
			continue
		}
		i++
		if i >= len(data) {
			return false
		}
		if data[i] != 'u' {
			continue
		}
		v, ok := hex4(i + 1)
		if !ok {
			return false
		}
		i += 4
		if v >= 0xdc00 && v <= 0xdfff {
			return false
		}
		if v >= 0xd800 && v <= 0xdbff {
			if i+6 >= len(data) || data[i+1] != '\\' || data[i+2] != 'u' {
				return false
			}
			low, ok := hex4(i + 3)
			if !ok || low < 0xdc00 || low > 0xdfff {
				return false
			}
			i += 6
		}
	}
	return true
}

func (s *Service) Decode(action, workspace, account string, body []byte) (Request, error) {
	d := s.state()
	if d == nil || len(body) > 16384 {
		return Request{}, ErrInvalid
	}
	name, ok := requestSchemas[action]
	if !ok {
		return Request{}, ErrInvalid
	}
	v, e := strictJSON(body)
	if e != nil {
		return Request{}, e
	}
	if e := d.schemas[name].Validate(v); e != nil {
		return Request{}, ErrInvalid
	}
	if workspace != "" && !wireID.MatchString(workspace) || account != "" && !wireID.MatchString(account) {
		return Request{}, ErrInvalid
	}
	return protect(RequestData{Action: action, WorkspaceID: workspace, AccountID: account, Fields: v.(map[string]any)}), nil
}
