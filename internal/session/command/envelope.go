// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package command validates the minimal fixture command boundary. Credentials
// and seat policies are supplied explicitly by the trusted operator composition.
package command

import (
	"context"
	"crypto/sha256"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math"
	"sync"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

var ErrEnvelope = errors.New("SESSION_ENVELOPE_REJECTED")
var ErrDenied = errors.New("SESSION_SEAT_DENIED")

type Envelope struct {
	CommandID            string           `json:"command_id"`
	SessionID            string           `json:"session_id"`
	ExpectedStateVersion uint64           `json:"expected_state_version"`
	SeatID               string           `json:"seat_id"`
	Type                 string           `json:"type"`
	Payload              checkpoint.Value `json:"payload"`
	CorrelationID        string           `json:"correlation_id"`
}
type ViewPolicy struct {
	ViewFields   []string
	EventFields  map[string][]string
	ResultFields []string
	ScalarResult bool
}
type FixtureSeat struct {
	Credential      string
	Binding         data.Binding
	Principal, Seat string
	Commands        map[string]func(checkpoint.Value) error
	Views           ViewPolicy
}
type record struct {
	seat     FixtureSeat
	epoch    uint64
	disabled bool
	current  NativeResolver
	recovery bool
	inputs   NativeInputSource
}
type Authority struct{ data **authorityData }
type authorityData struct {
	mu      sync.RWMutex
	records map[[32]byte]record
}
type Identity struct{ data **identityData }
type identityData struct {
	authority       *Authority
	key             [32]byte
	epoch           uint64
	binding         data.Binding
	principal, seat string
}

func (a *Authority) state() *authorityData {
	if a == nil || a.data == nil {
		return nil
	}
	return *a.data
}
func (i Identity) state() *identityData {
	if i.data == nil {
		return nil
	}
	return *i.data
}
func (Authority) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<session authority>") }
func (Authority) MarshalJSON() ([]byte, error) { return nil, ErrDenied }
func (Identity) Format(f fmt.State, _ rune)    { _, _ = io.WriteString(f, "<session identity>") }
func (Identity) MarshalJSON() ([]byte, error)  { return nil, ErrDenied }
func (i Identity) Binding() data.Binding {
	if i.state() == nil {
		return data.Binding{}
	}
	return i.state().binding
}
func (i Identity) Principal() string {
	if i.state() == nil {
		return ""
	}
	return i.state().principal
}
func (i Identity) Seat() string {
	if i.state() == nil {
		return ""
	}
	return i.state().seat
}
func issued(a *Authority, key [32]byte, r record) Identity {
	v := &identityData{authority: a, key: key, epoch: r.epoch, binding: r.seat.Binding, principal: r.seat.Principal, seat: r.seat.Seat}
	return Identity{data: &v}
}
func NewFixtureAuthority(seats []FixtureSeat) (*Authority, error) {
	if len(seats) < 1 || len(seats) > 256 {
		return nil, ErrDenied
	}
	d := &authorityData{records: map[[32]byte]record{}}
	a := &Authority{data: &d}
	seatsSeen := map[string]bool{}
	for _, s := range seats {
		if len(s.Credential) < 24 || len(s.Credential) > 256 || !store.ValidID(s.Binding.Workspace) || !store.ValidID(s.Binding.Session) || !checkpoint.IsDigest(s.Binding.GraphHash) || !store.ValidID(s.Principal) || !store.ValidID(s.Seat) || len(s.Commands) > 32 {
			return nil, ErrDenied
		}
		seatKey := s.Binding.Workspace + "/" + s.Binding.Session + "/" + s.Seat
		if seatsSeen[seatKey] {
			return nil, ErrDenied
		}
		seatsSeen[seatKey] = true
		key := sha256.Sum256([]byte(s.Credential))
		if _, ok := a.state().records[key]; ok {
			return nil, ErrDenied
		}
		commands := map[string]func(checkpoint.Value) error{}
		for name, f := range s.Commands {
			if !store.ValidID(name) || f == nil {
				return nil, ErrDenied
			}
			commands[name] = f
		}
		views, err := copyPolicy(s.Views)
		if err != nil {
			return nil, err
		}
		s.Commands = commands
		s.Views = views
		s.Credential = ""
		a.state().records[key] = record{seat: s, epoch: 1}
	}
	return a, nil
}
func copyPolicy(p ViewPolicy) (ViewPolicy, error) {
	q := p
	q.ViewFields = append([]string(nil), p.ViewFields...)
	q.ResultFields = append([]string(nil), p.ResultFields...)
	q.EventFields = map[string][]string{}
	if len(p.ViewFields) > 128 || len(p.ResultFields) > 128 || len(p.EventFields) > 64 {
		return q, ErrDenied
	}
	for name, fields := range p.EventFields {
		if name == "" || len(name) > 256 || len(fields) > 128 {
			return q, ErrDenied
		}
		q.EventFields[name] = append([]string(nil), fields...)
	}
	for _, fields := range [][]string{q.ViewFields, q.ResultFields} {
		for _, field := range fields {
			if !store.ValidID(field) {
				return q, ErrDenied
			}
		}
	}
	for _, fields := range q.EventFields {
		for _, field := range fields {
			if !store.ValidID(field) {
				return q, ErrDenied
			}
		}
	}
	return q, nil
}
func (a *Authority) Authenticate(credential, session, seat string) (Identity, error) {
	if a.state() == nil || len(credential) > 256 {
		return Identity{}, ErrDenied
	}
	key := sha256.Sum256([]byte(credential))
	d := a.state()
	d.mu.RLock()
	r, ok := d.records[key]
	d.mu.RUnlock()
	if !ok || r.disabled || r.current != nil || r.seat.Binding.Session != session || r.seat.Seat != seat {
		return Identity{}, ErrDenied
	}
	return issued(a, key, r), nil
}
func (a *Authority) resolve(i Identity) (FixtureSeat, error) {
	r, e := a.resolveCurrent(context.Background(), i)
	return r.seat, e
}
func (a *Authority) Verify(i Identity) error { return a.VerifyContext(context.Background(), i) }
func (a *Authority) VerifyContext(ctx context.Context, i Identity) error {
	_, e := a.resolveCurrent(ctx, i)
	return e
}
func (a *Authority) Policy(i Identity) (ViewPolicy, error) {
	return a.PolicyContext(context.Background(), i)
}
func (a *Authority) PolicyContext(ctx context.Context, i Identity) (ViewPolicy, error) {
	r, e := a.resolveCurrent(ctx, i)
	if e != nil {
		return ViewPolicy{}, e
	}
	return copyPolicy(r.seat.Views)
}
func (a *Authority) Revoke(credential string) error {
	if a.state() == nil || len(credential) > 256 {
		return ErrDenied
	}
	key := sha256.Sum256([]byte(credential))
	a.state().mu.Lock()
	defer a.state().mu.Unlock()
	r, ok := a.state().records[key]
	if !ok {
		return ErrDenied
	}
	r.disabled = true
	r.epoch++
	a.state().records[key] = r
	return nil
}
func Decode(raw []byte) (Envelope, error) {
	var e Envelope
	if checkpoint.StrictDecode(raw, &e, 128<<10) != nil {
		return e, ErrEnvelope
	}
	return e, nil
}
func (a *Authority) Validate(i Identity, e Envelope) (Envelope, error) {
	return a.ValidateContext(context.Background(), i, e)
}
func (a *Authority) ValidateContext(ctx context.Context, i Identity, e Envelope) (Envelope, error) {
	r, err := a.resolveCurrent(ctx, i)
	s := r.seat
	if err != nil {
		return Envelope{}, err
	}
	if !store.ValidID(e.CommandID) || e.SessionID != s.Binding.Session || e.SeatID != s.Seat || !store.ValidID(e.CorrelationID) || !store.ValidID(e.Type) || e.ExpectedStateVersion == 0 || e.ExpectedStateVersion >= math.MaxInt64 || checkpoint.Validate(e.Payload) != nil {
		return Envelope{}, ErrEnvelope
	}
	validate, ok := s.Commands[e.Type]
	if !ok {
		return Envelope{}, ErrDenied
	}
	raw, err := json.Marshal(e)
	if err != nil || len(raw) > 128<<10 {
		return Envelope{}, ErrEnvelope
	}
	var owned Envelope
	if json.Unmarshal(raw, &owned) != nil || validate(owned.Payload) != nil {
		return Envelope{}, ErrEnvelope
	}
	return owned, nil
}
