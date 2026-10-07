// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package core supplies internal relational identity and workspace services.
// Its identity issuance seams accept identities already verified by trusted
// server adapters; they are not login endpoints or credential verifiers.
package core

import (
	"context"
	"errors"
	"fmt"
	"io"
	"regexp"
	"strings"
	"time"
	"unicode"
	"unicode/utf8"
)

var (
	ErrDenied         = errors.New("platform authorization denied")
	ErrConflict       = errors.New("platform relation conflicts")
	ErrUnavailable    = errors.New("platform storage unavailable")
	ErrOutcomeUnknown = errors.New("platform commit outcome unknown")
)

type Role string

const (
	Owner  Role = "owner"
	Admin  Role = "admin"
	Member Role = "member"
)

type Permission uint8

const (
	ReadWorkspace Permission = iota + 1
	ManageWorkspace
	OwnCampaign
	UploadPackage
	RetainCredential
	Participate
	ControlSeat
	ReadPrivateView
)

type Account struct {
	ID, DisplayName string
	Disabled        bool
}

type Workspace struct {
	ID, Name, OwnerID string
}

type Membership struct {
	WorkspaceID, AccountID string
	Role                   Role
}

// Scope is the complete single-game participation boundary. A matching room
// name in another workspace, or another game in the same room, grants nothing.
type Scope struct {
	WorkspaceID, RoomID, GameID string
}

type Guest struct {
	Scope
	ID, ClaimedAccountID string
	ExpiresAt            time.Time
	Disabled             bool
}

type Participation struct {
	Scope
	GuestID, AccountID string
}

// Actor has no exported identity or permission fields. The double indirection
// also prevents fmt's invalid-verb fallback from traversing private identity
// data. Every use rechecks authoritative rows; the handle caches no role.
type Actor struct{ data **actorData }

type actorData struct {
	issuer *issuerKey
	id     string
	scope  Scope
	guest  bool
}

type issuerKey struct{ marker byte }

func (Actor) String() string               { return "<platform actor>" }
func (Actor) GoString() string             { return "<platform actor>" }
func (Actor) Format(s fmt.State, _ rune)   { _, _ = io.WriteString(s, "<platform actor>") }
func (Actor) MarshalJSON() ([]byte, error) { return nil, ErrDenied }

// Repository and Transaction are trusted storage/composition seams. Application
// callers use Service rather than these methods. No arbitrary SQL is accepted.
type Repository interface {
	Transact(context.Context, func(Transaction) error) error
}

type Transaction interface {
	Now(context.Context) (time.Time, error)
	Account(context.Context, string) (Account, error)
	InsertAccount(context.Context, Account) error
	DisableAccount(context.Context, string) error
	Workspace(context.Context, string) (Workspace, error)
	InsertWorkspace(context.Context, Workspace) error
	Membership(context.Context, string, string) (Membership, error)
	PutMembership(context.Context, Membership) error
	DeleteMembership(context.Context, string, string) error
	Guest(context.Context, Scope, string) (Guest, error)
	InsertGuest(context.Context, Guest) error
	ClaimGuest(context.Context, Scope, string, string) error
	DisableGuest(context.Context, Scope, string) error
	Participation(context.Context, Scope, string) (Participation, error)
}

var identifier = regexp.MustCompile(`^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,127}$`)

func validID(s string) bool { return identifier.MatchString(s) }

func validScope(s Scope) bool {
	return validID(s.WorkspaceID) && validID(s.RoomID) && validID(s.GameID)
}

func validLabel(s string) bool {
	if !utf8.ValidString(s) || strings.TrimSpace(s) == "" || utf8.RuneCountInString(s) > 128 {
		return false
	}
	for _, r := range s {
		if unicode.IsControl(r) {
			return false
		}
	}
	return true
}

func workspaceAllowed(role Role, permission Permission) bool {
	if role != Owner && role != Admin && role != Member {
		return false
	}
	switch permission {
	case ReadWorkspace:
		return true
	case ManageWorkspace, OwnCampaign, UploadPackage, RetainCredential:
		return role == Owner || role == Admin
	default:
		// Membership never grants participation, a seat, or a private view.
		return false
	}
}

// SafeError prevents storage callbacks from exporting driver diagnostics or
// arbitrary context causes through the application boundary.
func SafeError(err error) error {
	switch err {
	case nil, ErrDenied, ErrConflict, ErrUnavailable, ErrOutcomeUnknown:
		return err
	default:
		return ErrUnavailable
	}
}
