// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package ipc defines the narrow, versioned lifecycle protocol between the
// lua-runner process and its caller. It does not define Host API callbacks.
package ipc

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"strings"
	"unicode/utf8"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/vm"
)

const (
	ProtocolVersion = 1
	MaxMessageSize  = 4 << 20
)

var (
	ErrInvalidMessage = errors.New("invalid Lua runner message")
	ErrUnknownVersion = errors.New("unknown Lua runner protocol version")
	ErrUnknownType    = errors.New("unknown Lua runner message type")
)

type MessageType string

const (
	TypeCreate      MessageType = "create"
	TypeEval        MessageType = "eval"
	TypeLoadModule  MessageType = "load_module"
	TypeCheckpoint  MessageType = "checkpoint"
	TypeReconstruct MessageType = "reconstruct"
	TypeDestroy     MessageType = "destroy"
)

type Message interface {
	messageType() MessageType
}

type Create struct {
	SessionID      string `json:"session_id"`
	LuaProfile     string `json:"lua_profile"`
	RuntimeVersion string `json:"runtime_version"`
}

func (Create) messageType() MessageType { return TypeCreate }

type Eval struct {
	SessionID string `json:"session_id"`
	ChunkName string `json:"chunk_name"`
	Source    string `json:"source"`
}

func (Eval) messageType() MessageType { return TypeEval }

type LoadModule struct {
	SessionID string `json:"session_id"`
	Module    string `json:"module"`
	Source    string `json:"source"`
}

func (LoadModule) messageType() MessageType { return TypeLoadModule }

type Checkpoint struct {
	SessionID string             `json:"session_id"`
	Binding   checkpoint.Binding `json:"binding"`
	State     checkpoint.Value   `json:"state"`
}

func (Checkpoint) messageType() MessageType { return TypeCheckpoint }

type Reconstruct struct {
	SessionID      string             `json:"session_id"`
	Binding        checkpoint.Binding `json:"binding"`
	Authoritative  checkpoint.Value   `json:"authoritative"`
	Checkpoint     []byte             `json:"checkpoint"`
	RuntimeProgram Program            `json:"runtime_program"`
	RestoreProgram Program            `json:"restore_program"`
}

func (Reconstruct) messageType() MessageType { return TypeReconstruct }

// Program is an explicit source-only reconstruction input. It contains source
// bytes as JSON text and never a path or implicit package lookup key.
type Program struct {
	Name   string `json:"name"`
	Source string `json:"source"`
}

func (p Program) reconstructionProgram() vm.ReconstructionProgram {
	return vm.ReconstructionProgram{Name: p.Name, Source: p.Source}
}

type Destroy struct {
	SessionID string `json:"session_id"`
}

func (Destroy) messageType() MessageType { return TypeDestroy }

type envelope struct {
	Version   int             `json:"version"`
	Type      MessageType     `json:"type"`
	RequestID string          `json:"request_id"`
	Payload   json.RawMessage `json:"payload"`
}

type Request struct {
	RequestID string
	Message   Message
}

func validIdentity(value string, max int) bool {
	return strings.TrimSpace(value) != "" && len(value) <= max && utf8.ValidString(value) && !strings.ContainsRune(value, '\x00')
}

func validateMessage(message Message) (Message, error) {
	switch value := message.(type) {
	case Create:
		if !validIdentity(value.SessionID, 256) {
			return nil, ErrInvalidMessage
		}
		if _, err := profile.Resolve(value.LuaProfile, value.RuntimeVersion); err != nil {
			return nil, err
		}
		return value, nil
	case Eval:
		if !validIdentity(value.SessionID, 256) || !validIdentity(value.ChunkName, 512) {
			return nil, ErrInvalidMessage
		}
		if err := profile.ValidateSource([]byte(value.Source)); err != nil {
			return nil, err
		}
		return value, nil
	case LoadModule:
		if !validIdentity(value.SessionID, 256) || !validIdentity(value.Module, 256) {
			return nil, ErrInvalidMessage
		}
		if err := profile.ValidateSource([]byte(value.Source)); err != nil {
			return nil, err
		}
		return value, nil
	case Checkpoint:
		if !validIdentity(value.SessionID, 256) || value.Binding.SessionID != value.SessionID {
			return nil, ErrInvalidMessage
		}
		binding, err := checkpoint.NewBinding(value.Binding)
		if err != nil {
			return nil, err
		}
		state, err := checkpoint.Normalize(value.State)
		if err != nil {
			return nil, err
		}
		value.Binding, value.State = binding, state
		return value, nil
	case Reconstruct:
		if !validIdentity(value.SessionID, 256) || value.Binding.SessionID != value.SessionID {
			return nil, ErrInvalidMessage
		}
		binding, err := checkpoint.NewBinding(value.Binding)
		if err != nil {
			return nil, err
		}
		authoritative, err := checkpoint.Normalize(value.Authoritative)
		if err != nil {
			return nil, err
		}
		if _, err := checkpoint.Unmarshal(value.Checkpoint, binding); err != nil {
			return nil, err
		}
		if err := vm.ValidateReconstructionProgram(value.RuntimeProgram.reconstructionProgram()); err != nil {
			return nil, err
		}
		if err := vm.ValidateReconstructionProgram(value.RestoreProgram.reconstructionProgram()); err != nil {
			return nil, err
		}
		value.Binding, value.Authoritative = binding, authoritative
		return value, nil
	case Destroy:
		if !validIdentity(value.SessionID, 256) {
			return nil, ErrInvalidMessage
		}
		return value, nil
	default:
		return nil, ErrUnknownType
	}
}

// EncodeRequest returns deterministic single-object JSON.
func EncodeRequest(requestID string, message Message) ([]byte, error) {
	if !validIdentity(requestID, 256) {
		return nil, ErrInvalidMessage
	}
	validated, err := validateMessage(message)
	if err != nil {
		return nil, err
	}
	payload, err := json.Marshal(validated)
	if err != nil {
		return nil, err
	}
	encoded, err := json.Marshal(envelope{Version: ProtocolVersion, Type: validated.messageType(), RequestID: requestID, Payload: payload})
	if err != nil {
		return nil, err
	}
	if len(encoded) > MaxMessageSize {
		return nil, checkpoint.ErrOversized
	}
	return encoded, nil
}

// DecodeRequest rejects unknown versions, types, fields and trailing JSON.
func DecodeRequest(encoded []byte) (Request, error) {
	if len(encoded) == 0 || len(encoded) > MaxMessageSize || !utf8.Valid(encoded) {
		return Request{}, ErrInvalidMessage
	}
	var env envelope
	if err := strictDecode(encoded, &env); err != nil {
		return Request{}, err
	}
	if env.Version != ProtocolVersion {
		return Request{}, fmt.Errorf("%w: %d", ErrUnknownVersion, env.Version)
	}
	if !validIdentity(env.RequestID, 256) || len(env.Payload) == 0 {
		return Request{}, ErrInvalidMessage
	}
	var message Message
	switch env.Type {
	case TypeCreate:
		var payload Create
		if err := strictDecode(env.Payload, &payload); err != nil {
			return Request{}, err
		}
		message = payload
	case TypeEval:
		var payload Eval
		if err := strictDecode(env.Payload, &payload); err != nil {
			return Request{}, err
		}
		message = payload
	case TypeLoadModule:
		var payload LoadModule
		if err := strictDecode(env.Payload, &payload); err != nil {
			return Request{}, err
		}
		message = payload
	case TypeCheckpoint:
		var payload Checkpoint
		if err := strictDecode(env.Payload, &payload); err != nil {
			return Request{}, err
		}
		message = payload
	case TypeReconstruct:
		var payload Reconstruct
		if err := strictDecode(env.Payload, &payload); err != nil {
			return Request{}, err
		}
		message = payload
	case TypeDestroy:
		var payload Destroy
		if err := strictDecode(env.Payload, &payload); err != nil {
			return Request{}, err
		}
		message = payload
	default:
		return Request{}, fmt.Errorf("%w: %q", ErrUnknownType, env.Type)
	}
	validated, err := validateMessage(message)
	if err != nil {
		return Request{}, err
	}
	return Request{RequestID: env.RequestID, Message: validated}, nil
}

func strictDecode(encoded []byte, target any) error {
	if err := validateStrictJSON(encoded); err != nil {
		return err
	}
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(target); err != nil {
		return fmt.Errorf("%w: %v", ErrInvalidMessage, err)
	}
	var extra any
	if err := decoder.Decode(&extra); !errors.Is(err, io.EOF) {
		return ErrInvalidMessage
	}
	return nil
}
