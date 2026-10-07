// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package room implements the private lobby and invitation admission boundary.
package room

import (
	"context"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

type RoomData struct {
	Scope                  core.Scope
	ID, Name, Owner, State string
}
type InvitationData struct {
	Scope                     core.Scope
	ID, LinkHash, CodeHash    string
	ApprovalRequired, Revoked bool
	ExpiresAt                 time.Time
	MaxUses, Uses             int
}
type AdmissionData struct {
	Scope                                                core.Scope
	ID, InvitationID, Mode, Status, Name                 string
	OwnerAccount, OwnerSession, ParticipantID, TokenHash string
	ExpiresAt, TokenExpiresAt                            time.Time
	TokenUsed                                            bool
}
type ParticipantData struct {
	Scope                        core.Scope
	ID, AccountID, GuestID, Name string
	Active, Host                 bool
}
type StoredRoom = auth.Secret[RoomData]
type Invitation = auth.Secret[InvitationData]
type Admission = auth.Secret[AdmissionData]
type Participant = auth.Secret[ParticipantData]

// Storage can bind only an existing transaction. It cannot open a second
// transaction while authentication, admission and encrypted receipts commit.
type Storage interface {
	Bind(core.Transaction) (Transaction, error)
}
type Transaction interface {
	Room(context.Context, string, string) (StoredRoom, error)
	InsertRoom(context.Context, StoredRoom) error
	CloseRoom(context.Context, core.Scope) error
	Invitation(context.Context, core.Scope, string) (Invitation, error)
	InvitationBySecret(context.Context, string, string) (Invitation, error)
	PutInvitation(context.Context, Invitation) error
	Admission(context.Context, string) (Admission, error)
	AdmissionByToken(context.Context, string) (Admission, error)
	ActorAdmission(context.Context, core.Scope, string, string, string) (Admission, error)
	PutAdmission(context.Context, Admission) error
	Pending(context.Context, core.Scope, time.Time) ([]Admission, error)
	Participant(context.Context, core.Scope, string) (Participant, error)
	AccountParticipant(context.Context, core.Scope, string) (Participant, error)
	GuestParticipant(context.Context, core.Scope, string) (Participant, error)
	Participants(context.Context, core.Scope) ([]Participant, error)
	PutParticipant(context.Context, Participant) error
	Manager(context.Context, core.Scope, string) (bool, error)
	SetManager(context.Context, core.Scope, string, bool) error
}

type RequestData struct {
	Action, WorkspaceID, RoomID, ResourceID string
	Fields                                  map[string]any
}
type Request = auth.Secret[RequestData]

// A lobby never supplies a game-private view or a seat-control capability.
func AuthorizePrivateView() error { return auth.ErrDenied }
func AuthorizeSeatControl() error { return auth.ErrDenied }
