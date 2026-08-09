// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package ipc

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"sync"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/vm"
)

const (
	ResultPass = "PASS"
	ResultFail = "FAIL"
)

type Response struct {
	Version    int               `json:"version"`
	RequestID  string            `json:"request_id"`
	Result     string            `json:"result"`
	ErrorCode  string            `json:"error_code,omitempty"`
	Value      *checkpoint.Value `json:"value,omitempty"`
	Checkpoint []byte            `json:"checkpoint,omitempty"`
}

func pass(requestID string) Response {
	return Response{Version: ProtocolVersion, RequestID: requestID, Result: ResultPass}
}

func fail(requestID, code string) Response {
	return Response{Version: ProtocolVersion, RequestID: requestID, Result: ResultFail, ErrorCode: code}
}

func errorCode(err error) string {
	switch {
	case errors.Is(err, vm.ErrDestroyed):
		return "VM_DESTROYED"
	case errors.Is(err, vm.ErrPoisoned):
		return "VM_POISONED"
	case errors.Is(err, vm.ErrBusy):
		return "VM_BUSY"
	case errors.Is(err, checkpoint.ErrIncompatibleBinding), errors.Is(err, vm.ErrBindingMismatch):
		return "CHECKPOINT_INCOMPATIBLE"
	case errors.Is(err, checkpoint.ErrCorrupt):
		return "CHECKPOINT_CORRUPT"
	case errors.Is(err, checkpoint.ErrUnsupportedValue), errors.Is(err, checkpoint.ErrInvalidValue):
		return "CHECKPOINT_VALUE_REJECTED"
	case errors.Is(err, profile.ErrBytecode), errors.Is(err, profile.ErrInvalidUTF8):
		return "SOURCE_REJECTED"
	case errors.Is(err, context.Canceled), errors.Is(err, context.DeadlineExceeded):
		return "CANCELED"
	default:
		return "RUNTIME_REJECTED"
	}
}

type Server struct {
	mu  sync.RWMutex
	vms map[string]*vm.VM
}

func NewServer() *Server {
	return &Server{vms: make(map[string]*vm.VM)}
}

func (s *Server) lookup(sessionID string) (*vm.VM, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	runtime, ok := s.vms[sessionID]
	return runtime, ok
}

// Handle performs only VM lifecycle operations; there are no Host API or
// Session business commands in this protocol.
func (s *Server) Handle(ctx context.Context, request Request) Response {
	switch message := request.Message.(type) {
	case Create:
		created, err := vm.New(vm.Config{SessionID: message.SessionID, ProfileID: message.LuaProfile, RuntimeVersion: message.RuntimeVersion, Stdout: io.Discard})
		if err != nil {
			return fail(request.RequestID, errorCode(err))
		}
		s.mu.Lock()
		if _, exists := s.vms[message.SessionID]; exists {
			s.mu.Unlock()
			_ = created.Destroy(context.Background())
			return fail(request.RequestID, "VM_EXISTS")
		}
		s.vms[message.SessionID] = created
		s.mu.Unlock()
		return pass(request.RequestID)
	case Eval:
		runtime, ok := s.lookup(message.SessionID)
		if !ok {
			return fail(request.RequestID, "VM_NOT_FOUND")
		}
		value, err := runtime.Eval(ctx, message.ChunkName, []byte(message.Source))
		if err != nil {
			return fail(request.RequestID, errorCode(err))
		}
		response := pass(request.RequestID)
		response.Value = &value
		return response
	case LoadModule:
		runtime, ok := s.lookup(message.SessionID)
		if !ok {
			return fail(request.RequestID, "VM_NOT_FOUND")
		}
		if err := runtime.LoadModule(ctx, message.Module, []byte(message.Source)); err != nil {
			return fail(request.RequestID, errorCode(err))
		}
		return pass(request.RequestID)
	case Checkpoint:
		runtime, ok := s.lookup(message.SessionID)
		if !ok {
			return fail(request.RequestID, "VM_NOT_FOUND")
		}
		encoded, err := runtime.CreateCheckpoint(ctx, message.Binding, message.State)
		if err != nil {
			return fail(request.RequestID, errorCode(err))
		}
		response := pass(request.RequestID)
		response.Checkpoint = encoded
		return response
	case Reconstruct:
		config := vm.Config{SessionID: message.SessionID, ProfileID: message.Binding.LuaProfile, RuntimeVersion: message.Binding.RuntimeVersion, Stdout: io.Discard}
		replacement, err := vm.Reconstruct(ctx, config, message.Authoritative, message.Checkpoint, message.Binding)
		if err != nil {
			return fail(request.RequestID, errorCode(err))
		}
		s.mu.Lock()
		old := s.vms[message.SessionID]
		if old != nil {
			if err := old.Destroy(ctx); err != nil {
				s.mu.Unlock()
				_ = replacement.Destroy(context.Background())
				return fail(request.RequestID, errorCode(err))
			}
		}
		s.vms[message.SessionID] = replacement
		s.mu.Unlock()
		return pass(request.RequestID)
	case Destroy:
		s.mu.Lock()
		runtime, ok := s.vms[message.SessionID]
		if !ok {
			s.mu.Unlock()
			return fail(request.RequestID, "VM_NOT_FOUND")
		}
		if err := runtime.Destroy(ctx); err != nil {
			s.mu.Unlock()
			return fail(request.RequestID, errorCode(err))
		}
		delete(s.vms, message.SessionID)
		s.mu.Unlock()
		return pass(request.RequestID)
	default:
		return fail(request.RequestID, "UNKNOWN_MESSAGE")
	}
}

func (s *Server) Close() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	var joined error
	for id, runtime := range s.vms {
		if err := runtime.Destroy(context.Background()); err != nil && !errors.Is(err, vm.ErrDestroyed) {
			joined = errors.Join(joined, fmt.Errorf("destroy %s: %w", id, err))
		}
		delete(s.vms, id)
	}
	return joined
}

// Serve processes newline-delimited deterministic JSON on stdin/stdout.
func Serve(ctx context.Context, input io.Reader, output io.Writer) error {
	server := NewServer()
	defer server.Close()
	scanner := bufio.NewScanner(input)
	scanner.Buffer(make([]byte, 64*1024), MaxMessageSize)
	encoder := json.NewEncoder(output)
	encoder.SetEscapeHTML(false)
	for scanner.Scan() {
		if err := ctx.Err(); err != nil {
			return err
		}
		request, err := DecodeRequest(scanner.Bytes())
		response := fail("", "INVALID_MESSAGE")
		if err == nil {
			response = server.Handle(ctx, request)
		}
		if err := encoder.Encode(response); err != nil {
			return err
		}
	}
	if err := scanner.Err(); err != nil {
		return err
	}
	return ctx.Err()
}
