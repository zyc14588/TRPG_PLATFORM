// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package command

import (
	"encoding/json"
	"errors"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

func testAuthority(t *testing.T) (*Authority, Identity, Envelope) {
	t.Helper()
	a, err := NewFixtureAuthority([]FixtureSeat{{Credential: "fixture-client-0123456789abcdef", Binding: data.Binding{Workspace: "workspace", Session: "session", GraphHash: "sha256:" + strings.Repeat("a", 64)}, Principal: "person", Seat: "player", Commands: map[string]func(checkpoint.Value) error{"increment": func(v checkpoint.Value) error {
		if v.Kind != "integer" {
			return ErrEnvelope
		}
		return nil
	}}}})
	if err != nil {
		t.Fatal(err)
	}
	i, err := a.Authenticate("fixture-client-0123456789abcdef", "session", "player")
	if err != nil {
		t.Fatal(err)
	}
	return a, i, Envelope{CommandID: "command", SessionID: "session", SeatID: "player", Type: "increment", Payload: checkpoint.Int(1), ExpectedStateVersion: 1, CorrelationID: "correlation"}
}
func TestAuthenticatedEnvelopeBoundary(t *testing.T) {
	a, i, e := testAuthority(t)
	if _, err := a.Validate(i, e); err != nil {
		t.Fatal(err)
	}
	for _, c := range []struct {
		name   string
		change func(*Envelope)
	}{{"missing-id", func(e *Envelope) { e.CommandID = "" }}, {"session", func(e *Envelope) { e.SessionID = "other" }}, {"seat", func(e *Envelope) { e.SeatID = "gm" }}, {"zero-version", func(e *Envelope) { e.ExpectedStateVersion = 0 }}, {"type", func(e *Envelope) { e.Type = "arbitrary" }}, {"payload", func(e *Envelope) { e.Payload = checkpoint.Value{Kind: "string", String: "bad"} }}, {"correlation", func(e *Envelope) { e.CorrelationID = "" }}} {
		t.Run(c.name, func(t *testing.T) {
			changed := e
			c.change(&changed)
			if _, err := a.Validate(i, changed); err == nil {
				t.Fatal("envelope reached execution")
			}
		})
	}
	if _, err := a.Validate(Identity{}, e); !errors.Is(err, ErrDenied) {
		t.Fatal(err)
	}
	other, _, _ := testAuthority(t)
	if _, err := other.Validate(i, e); !errors.Is(err, ErrDenied) {
		t.Fatal("identity accepted by unrelated issuer")
	}
	if err := a.Revoke("fixture-client-0123456789abcdef"); err != nil {
		t.Fatal(err)
	}
	if _, err := a.Validate(i, e); !errors.Is(err, ErrDenied) {
		t.Fatal("queued identity remained valid")
	}
}
func TestEnvelopeDecodeRejectsAmbiguousWireData(t *testing.T) {
	_, _, e := testAuthority(t)
	raw, _ := json.Marshal(e)
	for _, bad := range [][]byte{[]byte(`{"command_id":"a","command_id":"b"}`), append(raw[:len(raw)-1], []byte(`,"unknown":true}`)...), []byte(strings.Repeat(" ", 128<<10) + "{}")} {
		if _, err := Decode(bad); err == nil {
			t.Fatal("ambiguous/oversized wire data admitted")
		}
	}
}
