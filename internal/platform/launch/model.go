// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package launch checks private lobby preparation before authoritative game
// creation. These typed seams are internal server composition, not a wire API.
package launch

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/persistence"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

type SeatRule struct {
	ID                string
	Required          bool
	Modes             []string
	ModelCapabilities []string
}

// Configuration is supplied only by trusted composition for an installed
// artifact. It cannot be supplied by a browser, package payload or Lua.
type ConfigurationData struct {
	ID, WorkspaceID string
	Factory         *install.SessionFactory
	Request         install.SessionRequest
	Seats           []SeatRule
	ContentTags     []string
	SafetyTags      []string
}
type Configuration = auth.Secret[ConfigurationData]
type Slot struct{ ID, Mode, ParticipantID, ModelSelection string }
type PreparationData struct {
	Scope                                         core.Scope
	ConfigurationID, ConfigurationHash, GraphHash string
	Revision                                      uint64
	Slots                                         []Slot
}
type Preparation = auth.Secret[PreparationData]
type AcknowledgmentData struct {
	Scope                                       core.Scope
	ParticipantID, ConfigurationHash, GraphHash string
	Revision                                    uint64
	Consent, Ready, SafetyConfirmed             bool
	Boundaries                                  []string
}
type Acknowledgment = auth.Secret[AcknowledgmentData]
type SessionData struct {
	Scope                              core.Scope
	ConfigurationID, ConfigurationHash string
	Revision                           uint64
	Binding                            data.Binding
}
type Session = auth.Secret[SessionData]
type ConfigureData struct {
	WorkspaceID, RoomID, ConfigurationID string
	Slots                                []Slot
}
type ConfigureRequest = auth.Secret[ConfigureData]
type AcknowledgeData struct {
	WorkspaceID, RoomID             string
	Revision                        uint64
	Consent, Ready, SafetyConfirmed bool
	Boundaries                      []string
}
type AcknowledgeRequest = auth.Secret[AcknowledgeData]
type LaunchData struct {
	WorkspaceID, RoomID string
	Revision            uint64
}
type LaunchRequest = auth.Secret[LaunchData]
type CallerData struct {
	Credential                    auth.BrowserCredential
	CSRF, IdempotencyKey, Network string
}
type Caller = auth.Secret[CallerData]

type ModelRequirementData struct {
	Scope                                                            core.Scope
	ConfigurationID, ConfigurationHash, GraphHash, SeatID, Selection string
	Revision                                                         uint64
	Capabilities                                                     []string
}
type ModelRequirement = auth.Secret[ModelRequirementData]

// This must consult server-owned, current capability and safety evidence.
// An absent provider fails closed whenever an AI seat is occupied. It is not
// an external dispatcher and cannot perform network work in the transaction.
type ModelChecker interface {
	Check(context.Context, core.Transaction, ModelRequirement, []Acknowledgment) error
}
type Storage interface {
	Bind(core.Transaction) (Transaction, error)
	RuntimeRepository() persistence.Repository
	SessionRepository() install.SessionRepository
}
type Transaction interface {
	Preparation(context.Context, core.Scope) (Preparation, error)
	PutPreparation(context.Context, Preparation) error
	Acknowledgments(context.Context, core.Scope) ([]Acknowledgment, error)
	PutAcknowledgment(context.Context, Acknowledgment) error
	Session(context.Context, core.Scope) (Session, error)
	PutSession(context.Context, Session) error
	SessionRepository(core.Scope, data.Binding) (install.SessionRepository, error)
}
