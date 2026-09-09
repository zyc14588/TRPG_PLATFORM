// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package ipc carries data-only requests over anonymous local pipes. It opens no
// listening sockets, inherits no application environment, and exposes no pointers.
package ipc

import (
	"encoding/binary"
	"io"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
)

const Version = 1
const MaxFrameBytes = 2 << 20

var ErrProtocol = profile.Fail("IPC_PROTOCOL_REJECTED")
var ErrRunner = profile.Fail("RUNNER_FAILED")

type Request struct {
	Version   int               `json:"version"`
	ID        uint64            `json:"id"`
	Operation string            `json:"operation"`
	Config    *profile.Config   `json:"config,omitempty"`
	Source    []byte            `json:"source,omitempty"`
	State     *checkpoint.Value `json:"state,omitempty"`
	Saved     *checkpoint.Value `json:"saved,omitempty"`
}
type Response struct {
	Version int            `json:"version"`
	ID      uint64         `json:"id"`
	Result  profile.Result `json:"result"`
	Error   string         `json:"error,omitempty"`
	Profile string         `json:"profile"`
	Runtime string         `json:"runtime"`
	PID     int            `json:"pid"`
}

func ReadFrame(r io.Reader) ([]byte, error) {
	var header [4]byte
	if _, err := io.ReadFull(r, header[:]); err != nil {
		return nil, err
	}
	n := binary.BigEndian.Uint32(header[:])
	if n == 0 || n > MaxFrameBytes {
		return nil, ErrProtocol
	}
	raw := make([]byte, n)
	if _, err := io.ReadFull(r, raw); err != nil {
		return nil, err
	}
	return raw, nil
}
func WriteFrame(w io.Writer, raw []byte) error {
	if len(raw) == 0 || len(raw) > MaxFrameBytes {
		return ErrProtocol
	}
	var header [4]byte
	binary.BigEndian.PutUint32(header[:], uint32(len(raw)))
	for _, part := range [][]byte{header[:], raw} {
		for len(part) > 0 {
			n, err := w.Write(part)
			if err != nil {
				return err
			}
			if n <= 0 {
				return io.ErrShortWrite
			}
			part = part[n:]
		}
	}
	return nil
}
