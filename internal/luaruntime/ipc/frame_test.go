// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package ipc

import (
	"bytes"
	"encoding/binary"
	"testing"
)

func TestBoundedFramingRejectsTruncationAndOversize(t *testing.T) {
	var out bytes.Buffer
	if err := WriteFrame(&out, []byte(`{"version":1}`)); err != nil {
		t.Fatal(err)
	}
	got, err := ReadFrame(&out)
	if err != nil || string(got) != `{"version":1}` {
		t.Fatalf("%s %v", got, err)
	}
	for _, raw := range [][]byte{{0, 0}, {0, 0, 0, 4, 'x'}, {0, 0, 0, 0}} {
		if _, err := ReadFrame(bytes.NewReader(raw)); err == nil {
			t.Fatal("bad frame accepted")
		}
	}
	var header [4]byte
	binary.BigEndian.PutUint32(header[:], MaxFrameBytes+1)
	if _, err := ReadFrame(bytes.NewReader(header[:])); err == nil {
		t.Fatal("oversize accepted")
	}
}
