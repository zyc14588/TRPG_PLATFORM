// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package checkpoint

import "fmt"

// Format keeps arbitrary Lua values out of ordinary Go diagnostics, including
// nested receipt and inspection structs. Authorized persistence and filtered
// transport use the explicit JSON codec, whose bytes remain unchanged.
// Do not traverse the value: even invalid, cyclic, or oversized values must
// produce bounded output without rendering keys, payloads, or identities.
func (Value) Format(out fmt.State, _ rune) {
	_, _ = out.Write([]byte("<checkpoint.Value:redacted>"))
}
