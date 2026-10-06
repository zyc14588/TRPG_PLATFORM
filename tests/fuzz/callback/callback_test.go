// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package callback_test

import (
	"bytes"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/ipc"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/tests/fuzz/support"
)

// This parameter target covers the production wire DTO and package-bound
// argument schema. Origin/token/phase authorization is exercised by the actual
// Host/VM/security M1 gates; this pure target does not claim that authority.
func FuzzHostCallbackParameters(f *testing.F) {
	f.Add(support.Callback())
	f.Add([]byte(`{"version":1,"version":2}`))
	p := support.Pair().Old.Root
	seed := checkpoint.Object(map[string]checkpoint.Value{"key": checkpoint.Text("one"), "delta": checkpoint.Int(7)})
	schema := support.Schema(p, "schemas/named.schema.json", seed)
	f.Fuzz(func(t *testing.T, raw []byte) {
		if len(raw) > support.MaxInput {
			return
		}
		before := append([]byte(nil), raw...)
		var c ipc.Callback
		if checkpoint.StrictDecode(raw, &c, support.MaxInput) != nil {
			return
		}
		if !bytes.Equal(before, raw) {
			t.Fatal("callback decoder mutated input")
		}
		if c.Kind != "callback" || c.Version != ipc.Version || c.Profile != profile.ID || c.Runtime != profile.RuntimeVersion || !profile.KnownHostOperation(c.Call.Capability, c.Call.Operation) {
			return
		}
		for _, v := range c.Call.Arguments {
			if checkpoint.Validate(v) != nil {
				return
			}
		}
		if len(c.Call.Arguments) != 2 || c.Call.Capability != "host.db" || c.Call.Operation != "named" {
			return
		}
		v := c.Call.Arguments[1]
		digest := checkpoint.Hash(support.JSON(v))
		first := schema.Validate(v)
		second := schema.Validate(v)
		if (first == nil) != (second == nil) || checkpoint.Hash(support.JSON(v)) != digest {
			t.Fatal("bound callback parameter validation changed facts")
		}
	})
}
