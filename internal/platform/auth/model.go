// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package auth supplies the approved local authentication boundary. Storage,
// admission and operator methods are trusted server composition seams.
package auth

import (
	"context"
	"errors"
	"fmt"
	"io"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

var (
	ErrInvalid         = errors.New("INVALID_REQUEST")
	ErrUnauthenticated = errors.New("UNAUTHENTICATED")
	ErrDenied          = errors.New("DENIED")
	ErrConflict        = errors.New("CONFLICT")
	ErrClaimRequired   = errors.New("CLAIM_REQUIRED")
	ErrRateLimited     = errors.New("RATE_LIMITED")
	ErrUnavailable     = errors.New("UNAVAILABLE")
	ErrOutcomeUnknown  = errors.New("OUTCOME_UNKNOWN")
)

func SafeError(e error) error {
	switch e {
	case nil, ErrInvalid, ErrUnauthenticated, ErrDenied, ErrConflict, ErrClaimRequired, ErrRateLimited, ErrUnavailable, ErrOutcomeUnknown:
		return e
	case core.ErrDenied:
		return ErrDenied
	case core.ErrConflict:
		return ErrConflict
	case core.ErrOutcomeUnknown:
		return ErrOutcomeUnknown
	default:
		return ErrUnavailable
	}
}

// Secret uses double indirection so fmt's invalid-verb fallback cannot descend
// into secrets. StorageValue is an explicit trusted storage/HTTP boundary;
// ordinary formatting and JSON exports are always denied.
type Secret[T any] struct{ data **T }

func protect[T any](v T) Secret[T]             { p := &v; return Secret[T]{data: &p} }
func (Secret[T]) String() string               { return "<platform authentication>" }
func (Secret[T]) GoString() string             { return "<platform authentication>" }
func (Secret[T]) Format(s fmt.State, _ rune)   { _, _ = io.WriteString(s, "<platform authentication>") }
func (Secret[T]) MarshalJSON() ([]byte, error) { return nil, ErrDenied }
func (s Secret[T]) StorageValue() T {
	if s.data == nil || *s.data == nil {
		var zero T
		return zero
	}
	return **s.data
}

type Password = Secret[[]byte]

func StoredPassword(data []byte) (Password, error) {
	if len(data) != 48 {
		return Password{}, ErrDenied
	}
	return protect(append([]byte(nil), data...)), nil
}

type BrowserCredential = Secret[string]

func BrowserCookie(value string) BrowserCredential { return protect(value) }

type SessionData struct {
	Hash                string
	Kind                string
	AccountID, GuestID  string
	Scope               core.Scope
	ExpiresAt, LastSeen time.Time
	Retired, Revoked    bool
	SuccessorHash       string
}
type Session = Secret[SessionData]

func StoredSession(v SessionData) Session { return protect(v) }

type ReceiptData struct {
	OwnerHash, Endpoint, Key, RequestDigest string
	ExpiresAt                               time.Time
	Ciphertext                              []byte
}
type Receipt = Secret[ReceiptData]

func StoredReceipt(v ReceiptData) Receipt {
	v.Ciphertext = append([]byte(nil), v.Ciphertext...)
	return protect(v)
}

type OutcomeData struct {
	Body       []byte
	Cookie     BrowserCredential
	CookieKind string
	ExpiresAt  time.Time
}
type Outcome = Secret[OutcomeData]

type Transaction interface {
	Core() core.Transaction
	Session(context.Context, string) (Session, error)
	PutSession(context.Context, Session) error
	Receipt(context.Context, string, string, string) (Receipt, error)
	PutReceipt(context.Context, Receipt) error
	Credential(context.Context, string) (string, Password, error)
	PutCredential(context.Context, string, string, Password) error
	ConsumeGrant(context.Context, string) error
	AccountCount(context.Context) (int, error)
	PutGrant(context.Context, string, time.Time) error
}
type Repository interface {
	Transact(context.Context, func(Transaction) error) error
}

// AdmissionVerifier must validate an invitation and provision its restricted
// guest using this transaction. No client supplies the returned scope. A nil
// verifier denies exchange. The later room service owns invitation semantics.
type AdmissionVerifier interface {
	Verify(context.Context, core.Transaction, BrowserCredential) (core.Guest, error)
}

type RequestData struct {
	Action, WorkspaceID, AccountID string
	Fields                         map[string]any
}
type Request = Secret[RequestData]

type singleTransaction struct{ tx core.Transaction }

func (r singleTransaction) Transact(ctx context.Context, f func(core.Transaction) error) error {
	return f(r.tx)
}
