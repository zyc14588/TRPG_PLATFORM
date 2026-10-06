// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package protocol_test

import (
	"bytes"
	"encoding/json"
	"fmt"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/tests/fuzz/support"
)

func FuzzProtocolEnvelope(f *testing.F) {
	f.Add(support.Protocol())
	f.Add([]byte(`{"command_id":"a","command_id":"b"}`))
	b := data.Binding{Workspace: "fixture", Session: "fixture", GraphHash: checkpoint.Hash([]byte("protocol-graph"))}
	a, err := command.NewFixtureAuthority([]command.FixtureSeat{{Credential: "m1-fuzz-synthetic-gm-credential", Binding: b, Principal: "gm", Seat: "gm", Commands: map[string]func(checkpoint.Value) error{"increment": func(v checkpoint.Value) error {
		if v.Kind != "table" || len(v.Table) != 1 || v.Table["delta"].Kind != "integer" {
			return fmt.Errorf("invalid fixed payload")
		}
		return nil
	}}}})
	if err != nil {
		f.Fatal(err)
	}
	i, err := a.Authenticate("m1-fuzz-synthetic-gm-credential", "fixture", "gm")
	if err != nil {
		f.Fatal(err)
	}
	f.Fuzz(func(t *testing.T, raw []byte) {
		if len(raw) > support.MaxInput {
			return
		}
		before := append([]byte(nil), raw...)
		e, err := command.Decode(raw)
		if !bytes.Equal(raw, before) {
			t.Fatal("decoder mutated caller bytes")
		}
		if err != nil {
			return
		}
		canonical, err := json.Marshal(e)
		if err != nil {
			t.Fatal(err)
		}
		round, err := command.Decode(canonical)
		// JSON omitempty normalizes empty slices/maps to nil. Compare exact
		// canonical wire facts, not unobservable Go collection allocation.
		if err != nil || !bytes.Equal(support.JSON(round), canonical) {
			t.Fatal("accepted envelope failed canonical roundtrip")
		}
		accepted, err := a.Validate(i, e)
		if err == nil && (accepted.SessionID != "fixture" || accepted.SeatID != "gm" || accepted.Type != "increment" || accepted.ExpectedStateVersion == 0) {
			t.Fatal("fixed authority accepted another scope")
		}
	})
}
