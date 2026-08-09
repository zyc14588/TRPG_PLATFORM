// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package ipc

import (
	"bytes"
	"context"
	"errors"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
)

const ipcHash = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"

func ipcBinding(sessionID string) checkpoint.Binding {
	return checkpoint.Binding{
		SessionID:      sessionID,
		StateVersion:   1,
		PackageHashes:  []checkpoint.PackageHash{{PackageID: "pkg.test", SHA256: ipcHash}},
		DependencyLock: `{"packages":["pkg.test"]}`,
		LuaProfile:     profile.ProductionID,
		RuntimeVersion: profile.RuntimeIdentity,
	}
}

func TestRequestCodecIsDeterministicAndTyped(t *testing.T) {
	message := Create{SessionID: "session-a", LuaProfile: profile.ProductionID, RuntimeVersion: profile.RuntimeIdentity}
	first, err := EncodeRequest("request-1", message)
	if err != nil {
		t.Fatal(err)
	}
	second, err := EncodeRequest("request-1", message)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(first, second) {
		t.Fatal("request encoding is nondeterministic")
	}
	decoded, err := DecodeRequest(first)
	if err != nil {
		t.Fatal(err)
	}
	if _, ok := decoded.Message.(Create); !ok || decoded.RequestID != "request-1" {
		t.Fatalf("decoded request = %#v", decoded)
	}
}

func TestRequestCodecFailsClosed(t *testing.T) {
	tests := []struct {
		name string
		data string
		want error
	}{
		{name: "unknown version", data: `{"version":2,"type":"destroy","request_id":"r","payload":{"session_id":"s"}}`, want: ErrUnknownVersion},
		{name: "unknown type", data: `{"version":1,"type":"host_callback","request_id":"r","payload":{}}`, want: ErrUnknownType},
		{name: "unknown envelope field", data: `{"version":1,"type":"destroy","request_id":"r","payload":{"session_id":"s"},"extra":true}`, want: ErrInvalidMessage},
		{name: "unknown payload field", data: `{"version":1,"type":"destroy","request_id":"r","payload":{"session_id":"s","extra":true}}`, want: ErrInvalidMessage},
		{name: "trailing json", data: `{"version":1,"type":"destroy","request_id":"r","payload":{"session_id":"s"}} {}`, want: ErrInvalidMessage},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if _, err := DecodeRequest([]byte(tt.data)); !errors.Is(err, tt.want) {
				t.Fatalf("DecodeRequest error = %v, want %v", err, tt.want)
			}
		})
	}
}

func TestRequestCodecRejectsBytecode(t *testing.T) {
	message := Eval{SessionID: "session-a", ChunkName: "binary", Source: string([]byte{6, 0, 4, 1})}
	if _, err := EncodeRequest("request-binary", message); !errors.Is(err, profile.ErrBytecode) {
		t.Fatalf("EncodeRequest bytecode error = %v", err)
	}
}

func TestRequestCodecRejectsInvalidUTF8WithoutReplacement(t *testing.T) {
	prefix := []byte(`{"version":1,"type":"eval","request_id":"r","payload":{"session_id":"s","chunk_name":"c","source":"`)
	encoded := append(prefix, 0xff)
	encoded = append(encoded, []byte(`"}}`)...)
	if _, err := DecodeRequest(encoded); !errors.Is(err, ErrInvalidMessage) {
		t.Fatalf("invalid UTF-8 error = %v", err)
	}
}

func TestServerLifecycleAndIsolation(t *testing.T) {
	server := NewServer()
	defer server.Close()
	ctx := context.Background()
	create := func(id string) {
		t.Helper()
		response := server.Handle(ctx, Request{RequestID: "create-" + id, Message: Create{SessionID: id, LuaProfile: profile.ProductionID, RuntimeVersion: profile.RuntimeIdentity}})
		if response.Result != ResultPass {
			t.Fatalf("create %s: %#v", id, response)
		}
	}
	create("session-a")
	create("session-b")
	set := server.Handle(ctx, Request{RequestID: "set", Message: Eval{SessionID: "session-a", ChunkName: "set", Source: "marker = 9; return marker"}})
	if set.Result != ResultPass || set.Value == nil || set.Value.Integer != 9 {
		t.Fatalf("set response = %#v", set)
	}
	get := server.Handle(ctx, Request{RequestID: "get", Message: Eval{SessionID: "session-b", ChunkName: "get", Source: "return marker == nil"}})
	if get.Result != ResultPass || get.Value == nil || !get.Value.Boolean {
		t.Fatalf("cross-session response = %#v", get)
	}

	state := checkpoint.Table(checkpoint.Entry("turn", checkpoint.Int(3)))
	cp := server.Handle(ctx, Request{RequestID: "checkpoint", Message: Checkpoint{SessionID: "session-a", Binding: ipcBinding("session-a"), State: state}})
	if cp.Result != ResultPass || len(cp.Checkpoint) == 0 {
		t.Fatalf("checkpoint response = %#v", cp)
	}
	reconstructed := server.Handle(ctx, Request{RequestID: "reconstruct", Message: Reconstruct{SessionID: "session-a", Binding: ipcBinding("session-a"), Authoritative: checkpoint.Table(checkpoint.Entry("version", checkpoint.Int(1))), Checkpoint: cp.Checkpoint}})
	if reconstructed.Result != ResultPass {
		t.Fatalf("reconstruct response = %#v", reconstructed)
	}
	clean := server.Handle(ctx, Request{RequestID: "clean", Message: Eval{SessionID: "session-a", ChunkName: "clean", Source: "return marker == nil"}})
	if clean.Result != ResultPass || clean.Value == nil || !clean.Value.Boolean {
		t.Fatalf("reconstructed VM retained pollution: %#v", clean)
	}
	destroyed := server.Handle(ctx, Request{RequestID: "destroy", Message: Destroy{SessionID: "session-a"}})
	if destroyed.Result != ResultPass {
		t.Fatalf("destroy response = %#v", destroyed)
	}
}

func TestServeRejectsUnknownMessageWithoutSideEffects(t *testing.T) {
	input := strings.NewReader(`{"version":1,"type":"host_callback","request_id":"bad","payload":{}}` + "\n")
	var output bytes.Buffer
	if err := Serve(context.Background(), input, &output); err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(output.String(), `"result":"FAIL"`) || !strings.Contains(output.String(), `"error_code":"INVALID_MESSAGE"`) {
		t.Fatalf("response = %s", output.String())
	}
}
