// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package ipc

import (
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
)

// Callback envelopes have a direction and both outer and per-command sequence.
// They contain data only; the parent's opaque execution capability is never sent.
type Callback struct {
	Kind     string           `json:"kind"`
	Version  int              `json:"version"`
	ID       uint64           `json:"id"`
	Sequence uint64           `json:"sequence"`
	PID      int              `json:"pid"`
	Profile  string           `json:"profile"`
	Runtime  string           `json:"runtime"`
	Call     profile.HostCall `json:"call"`
}
type CallbackReply struct {
	Kind     string           `json:"kind"`
	Version  int              `json:"version"`
	ID       uint64           `json:"id"`
	Sequence uint64           `json:"sequence"`
	Value    checkpoint.Value `json:"value"`
	Error    string           `json:"error,omitempty"`
}
