// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package auth

import (
	"context"
	"crypto/hmac"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"io"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

// RoomAuthority shares the existing authentication service, session checks,
// limiter and encrypted receipts. Only trusted server composition constructs it.
type RoomAuthority struct{ data **roomAuthorityData }
type roomAuthorityData struct {
	service           *Service
	invitationKeyHash [32]byte
}

func NewRoomAuthority(repo Repository, cookieKey, replayKey, invitationKey, schema []byte, verifier AdmissionVerifier, claims RoomClaimGuard) (*RoomAuthority, error) {
	if len(invitationKey) != 32 || hmac.Equal(invitationKey, cookieKey) || hmac.Equal(invitationKey, replayKey) || verifier == nil || claims == nil {
		return nil, ErrInvalid
	}
	wrapped, e := newRoomRepository(repo, claims)
	if e != nil {
		return nil, e
	}
	s, e := NewService(wrapped, cookieKey, replayKey, schema, verifier)
	if e != nil {
		return nil, e
	}
	d := &roomAuthorityData{service: s, invitationKeyHash: sha256.Sum256(invitationKey)}
	return &RoomAuthority{data: &d}, nil
}
func (a *RoomAuthority) state() *roomAuthorityData {
	if a == nil || a.data == nil {
		return nil
	}
	return *a.data
}
func (*RoomAuthority) String() string   { return "<room authentication authority>" }
func (*RoomAuthority) GoString() string { return "<room authentication authority>" }
func (*RoomAuthority) Format(s fmt.State, _ rune) {
	_, _ = io.WriteString(s, "<room authentication authority>")
}
func (*RoomAuthority) MarshalJSON() ([]byte, error) { return nil, ErrDenied }
func (a *RoomAuthority) Authentication() *Service {
	if a.state() == nil {
		return nil
	}
	return a.state().service
}
func (a *RoomAuthority) InvitationKeyMatches(key []byte) bool {
	if a.state() == nil || len(key) != 32 {
		return false
	}
	h := sha256.Sum256(key)
	return hmac.Equal(h[:], a.state().invitationKeyHash[:])
}

// These are explicit trusted decoding/transport seams. Request and outcome
// handles retain the same double-indirection privacy boundary as authentication.
func RoomJSON(body []byte) (any, error) {
	if len(body) > 16384 {
		return nil, ErrInvalid
	}
	return strictJSON(body)
}
func RoomContractJSON(body []byte) (any, error) {
	if len(body) > 65536 {
		return nil, ErrInvalid
	}
	return strictJSON(body)
}
func RoomSecret[T any](v T) Secret[T] { return protect(v) }
func RoomOutcome(body []byte) Outcome {
	return protect(OutcomeData{Body: append([]byte(nil), body...)})
}

type RoomCommandData struct {
	Action, TargetKey string
	Canonical         []byte
	Write             bool
}
type RoomCommand = Secret[RoomCommandData]
type RoomCallbacks struct {
	Apply  func(context.Context, Transaction, SessionData) (Outcome, error)
	Replay func(context.Context, Transaction, SessionData, Outcome) error
}

func (a *RoomAuthority) Do(ctx context.Context, cookie BrowserCredential, csrf, key, network string, command RoomCommand, calls RoomCallbacks) (Outcome, error) {
	if a.state() == nil || ctx == nil || ctx.Err() != nil || calls.Apply == nil || calls.Replay == nil {
		return Outcome{}, ErrUnavailable
	}
	s := a.state().service
	r := command.StorageValue()
	if !wireID.MatchString(r.Action) || len(r.Action) > 40 || len(r.TargetKey) > 768 || len(r.Canonical) > 20000 || len(network) > 256 {
		return Outcome{}, ErrInvalid
	}
	if r.Write && (!idemID.MatchString(key) || !tokenID.MatchString(csrf)) {
		return Outcome{}, ErrInvalid
	}
	// Hash full target identities rather than truncating them to the existing
	// receipt endpoint bound. The request digest also binds the whole operation.
	ep := "room-v1/" + r.Action + "/" + s.mac("room-target-v1\x00"+r.TargetKey)
	digest := s.mac("room-request-v1\x00" + ep + "\x00" + string(r.Canonical))
	var out Outcome
	e := s.transact(ctx, func(tx Transaction) error {
		v, e := s.findSession(ctx, tx, cookie)
		if e != nil {
			return e
		}
		if e = s.live(ctx, tx, v, false, ""); e != nil {
			return e
		}
		if r.Write && !hmac.Equal([]byte(csrf), []byte(s.csrf(v))) {
			return ErrDenied
		}
		identity := v.Kind + "/" + v.AccountID
		if v.Kind == "preauth" {
			identity = "anonymous/" + network
		}
		if v.Kind == "guest" {
			identity = "guest/" + v.Scope.WorkspaceID + "/" + v.Scope.RoomID + "/" + v.Scope.GameID + "/" + v.GuestID
		}
		if e = s.limit(s.mac("room-rate-v1\x00" + identity + "\x00" + r.Action)); e != nil {
			return e
		}
		now, e := tx.Core().Now(ctx)
		if e != nil {
			return SafeError(e)
		}
		now = sessionTime(now)
		if r.Write {
			receipt, e := tx.Receipt(ctx, v.Hash, ep, key)
			if e == nil {
				saved := receipt.StorageValue()
				if saved.RequestDigest != digest || !now.Before(saved.ExpiresAt) {
					return ErrConflict
				}
				out, e = s.openReceipt(saved)
				if e != nil {
					return e
				}
				if out.StorageValue().Cookie.StorageValue() != "" {
					return ErrDenied
				}
				return SafeError(calls.Replay(ctx, tx, v, out))
			}
			if e != ErrDenied && e != core.ErrDenied {
				return SafeError(e)
			}
		}
		v.LastSeen = now
		if e = tx.PutSession(ctx, StoredSession(v)); e != nil {
			return SafeError(e)
		}
		out, e = calls.Apply(ctx, tx, v)
		if e != nil {
			return SafeError(e)
		}
		if len(out.StorageValue().Body) == 0 || out.StorageValue().Cookie.StorageValue() != "" {
			return ErrUnavailable
		}
		if !r.Write {
			return nil
		}
		expires := now.Add(5 * time.Minute)
		if v.ExpiresAt.Before(expires) {
			expires = v.ExpiresAt
		}
		receipt, e := s.sealReceipt(ReceiptData{OwnerHash: v.Hash, Endpoint: ep, Key: key, RequestDigest: digest, ExpiresAt: expires}, out)
		if e != nil {
			return e
		}
		return SafeError(tx.PutReceipt(ctx, receipt))
	})
	if e != nil {
		return Outcome{}, e
	}
	return out, nil
}

// CanonicalRoomFields freezes a fresh, strictly decoded object. Callers still
// validate its approved Schema and normalize its specified integer fields.
func CanonicalRoomFields(fields map[string]any) ([]byte, error) {
	b, e := json.Marshal(fields)
	if e != nil {
		return nil, ErrInvalid
	}
	return b, nil
}
