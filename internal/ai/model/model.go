// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package model implements server-owned model routes and current scoped
// configuration. No browser field can issue a qualification or an endpoint.
package model

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/credential"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"time"
)

type Tuple struct{ Model, Endpoint, Adapter, PromptTemplate, ToolMode, TestVersion string }
type EndpointData struct {
	ID, URL, Adapter string
	Models           []string
	AllowLANHTTP     bool
}
type Endpoint = auth.Secret[EndpointData]
type GameEvidence struct {
	GraphHash, TestVersion, EvidenceHash string
	Capabilities                         []string
}

// Qualifications come only from the server's reviewed test registry. The
// execution batch supplies actual provider tests; users select these records.
type CertificationData struct {
	ID, WorkspaceID string
	Tuple           Tuple
	Level           int
	Capabilities    []string
	Games           []GameEvidence
	EvidenceHash    string
	ExpiresAt       time.Time
}
type Certification = auth.Secret[CertificationData]

// Limits are finite configuration caps. B008 owns aggregate reservations,
// accounting and provider execution; this batch never records them as run.
type Limits struct{ Calls, Tokens, CostMicros, LatencyMillis, Tools, Subagents, ContextBytes, LocalComputeMillis uint64 }
type CertificateBinding struct{ ID, Hash string }
type ConfigurationData struct {
	Scope                                         core.Scope
	SeatID, Selection, OwnerKind, OwnerID         string
	ConfigurationID, ConfigurationHash, GraphHash string
	PreparationRevision                           uint64
	Version                                       uint64
	CredentialID                                  string
	CredentialVersion                             uint64
	Primary                                       CertificateBinding
	Tuple                                         Tuple
	Fallbacks                                     []CertificateBinding
	Budget                                        Limits
	Revoked                                       bool
}
type Configuration = auth.Secret[ConfigurationData]
type CallerData struct {
	Credential                    auth.BrowserCredential
	CSRF, IdempotencyKey, Network string
}
type Caller = auth.Secret[CallerData]
type CredentialRequestData struct {
	Scope      core.Scope
	SeatID, ID string
	Lifetime   credential.Lifetime
	ExpiresAt  time.Time
	Key        credential.Key
}
type CredentialRequest = auth.Secret[CredentialRequestData]
type ConfigureRequestData struct {
	Scope                                            core.Scope
	SeatID, Selection, CertificationID, CredentialID string
	FallbackIDs                                      []string
	Budget                                           Limits
	ExpectedVersion                                  uint64
}
type ConfigureRequest = auth.Secret[ConfigureRequestData]
type TargetData struct {
	Scope      core.Scope
	SeatID, ID string
}
type Target = auth.Secret[TargetData]

type Storage interface {
	Bind(core.Transaction) (Transaction, error)
}
type Transaction interface {
	Credential(context.Context, core.Scope, string, string) (credential.Record, error)
	InsertCredential(context.Context, credential.Record) error
	RevokeCredential(context.Context, core.Scope, string, string, uint64) error
	Configuration(context.Context, core.Scope, string, string) (Configuration, error)
	PutConfiguration(context.Context, Configuration, uint64) error
	RevokeConfiguration(context.Context, core.Scope, string, string, uint64) error
}
