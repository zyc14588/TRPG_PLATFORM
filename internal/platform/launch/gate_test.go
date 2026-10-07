// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package launch

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
	"strings"
	"testing"
	"time"
)

type gateCore struct {
	core.Transaction
	disabled bool
	guest    core.Guest
	now      time.Time
}

func (c *gateCore) Account(_ context.Context, id string) (core.Account, error) {
	return core.Account{ID: id, Disabled: c.disabled}, nil
}
func (c *gateCore) Guest(_ context.Context, s core.Scope, id string) (core.Guest, error) {
	if c.guest.Scope != s || c.guest.ID != id {
		return core.Guest{}, auth.ErrDenied
	}
	return c.guest, nil
}
func (c *gateCore) Now(context.Context) (time.Time, error) { return c.now, nil }

type gateRoom struct {
	room.Transaction
	parts []room.Participant
}

func (r *gateRoom) Participants(context.Context, core.Scope) ([]room.Participant, error) {
	return r.parts, nil
}

type gateModels struct {
	calls    int
	allowed  bool
	received ModelRequirementData
}

func (m *gateModels) Check(_ context.Context, _ core.Transaction, v ModelRequirement, _ []Acknowledgment) error {
	m.calls++
	m.received = v.StorageValue()
	if !m.allowed {
		return auth.ErrDenied
	}
	return nil
}
func gateFixture() (ConfigurationData, PreparationData, []Acknowledgment, *gateCore, *gateRoom) {
	scope := core.Scope{WorkspaceID: "workspace", RoomID: "private-room", GameID: "game"}
	hash := "sha256:" + strings.Repeat("a", 64)
	c := ConfigurationData{ID: "minimal", WorkspaceID: scope.WorkspaceID, Seats: []SeatRule{{ID: "gm", Required: true, Modes: []string{"human"}}}, SafetyTags: []string{"violence", "gore"}}
	p := PreparationData{Scope: scope, ConfigurationID: c.ID, ConfigurationHash: configurationHash(c), GraphHash: hash, Revision: 1, Slots: []Slot{{ID: "gm", Mode: "human", ParticipantID: "participant"}}}
	a := auth.RoomSecret(AcknowledgmentData{Scope: scope, ParticipantID: "participant", ConfigurationHash: p.ConfigurationHash, GraphHash: hash, Revision: 1, Consent: true, Ready: true, SafetyConfirmed: true})
	tx := &gateCore{now: time.Unix(1000, 0)}
	rt := &gateRoom{parts: []room.Participant{auth.RoomSecret(room.ParticipantData{Scope: scope, ID: "participant", AccountID: "account", Active: true, Host: true})}}
	return c, p, []Acknowledgment{a}, tx, rt
}
func TestReadinessHumanLaunchRequiresCurrentAllGates(t *testing.T) {
	c, p, a, tx, rt := gateFixture()
	seat, e := checkReadiness(context.Background(), tx, rt, p, c, a, nil)
	if e != nil || seat != "gm" {
		t.Fatal("valid human preparation rejected")
	}
}
func TestReadinessRejectsEachMissingOrStaleGate(t *testing.T) {
	cases := []struct {
		name   string
		change func(*ConfigurationData, *PreparationData, *[]Acknowledgment, *gateCore, *gateRoom)
	}{
		{"missing-required-seat", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			p.Slots = nil
		}},
		{"unseated-active-participant", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			r.parts = append(r.parts, auth.RoomSecret(room.ParticipantData{Scope: p.Scope, ID: "second", AccountID: "another", Active: true}))
		}},
		{"missing-consent", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			v := (*a)[0].StorageValue()
			v.Consent = false
			(*a)[0] = auth.RoomSecret(v)
		}},
		{"not-ready", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			v := (*a)[0].StorageValue()
			v.Ready = false
			(*a)[0] = auth.RoomSecret(v)
		}},
		{"safety-unconfirmed", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			v := (*a)[0].StorageValue()
			v.SafetyConfirmed = false
			(*a)[0] = auth.RoomSecret(v)
		}},
		{"missing-acknowledgment", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			*a = nil
		}},
		{"old-revision", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			p.Revision++
		}},
		{"old-package-graph", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			p.GraphHash = "sha256:" + strings.Repeat("b", 64)
		}},
		{"configuration-changed-after-consent", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			c.ContentTags = []string{"violence"}
		}},
		{"cross-workspace-acknowledgment", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			v := (*a)[0].StorageValue()
			v.Scope.WorkspaceID = "other"
			(*a)[0] = auth.RoomSecret(v)
		}},
		{"cross-game-participant", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			v := r.parts[0].StorageValue()
			v.Scope.GameID = "other"
			r.parts[0] = auth.RoomSecret(v)
		}},
		{"disabled-account", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			tx.disabled = true
		}},
		{"inactive-participant", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			v := r.parts[0].StorageValue()
			v.Active = false
			r.parts[0] = auth.RoomSecret(v)
		}},
		{"expired-guest", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			v := r.parts[0].StorageValue()
			v.GuestID = "guest"
			v.AccountID = ""
			r.parts[0] = auth.RoomSecret(v)
			tx.guest = core.Guest{Scope: p.Scope, ID: "guest", ExpiresAt: tx.now}
		}},
		{"guest-claim-mismatch", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			v := r.parts[0].StorageValue()
			v.GuestID = "guest"
			r.parts[0] = auth.RoomSecret(v)
			tx.guest = core.Guest{Scope: p.Scope, ID: "guest", ClaimedAccountID: "other", ExpiresAt: tx.now.Add(time.Hour)}
		}},
		{"duplicate-seat", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			p.Slots = append(p.Slots, p.Slots[0])
		}},
		{"unknown-seat", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			p.Slots[0].ID = "unregistered"
		}},
		{"unknown-safety-boundary", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			v := (*a)[0].StorageValue()
			v.Boundaries = []string{"unrecognized"}
			(*a)[0] = auth.RoomSecret(v)
		}},
		{"duplicate-acknowledgment", func(c *ConfigurationData, p *PreparationData, a *[]Acknowledgment, tx *gateCore, r *gateRoom) {
			*a = append(*a, (*a)[0])
		}},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			c, p, a, tx, rt := gateFixture()
			test.change(&c, &p, &a, tx, rt)
			if _, e := checkReadiness(context.Background(), tx, rt, p, c, a, nil); e == nil {
				t.Fatal("invalid preparation opened the launch gate")
			}
		})
	}
}
func TestContentBoundaryBlocksLabelEvenWithReadyConsent(t *testing.T) {
	c, p, a, tx, rt := gateFixture()
	c.ContentTags = []string{"violence"}
	p.ConfigurationHash = configurationHash(c)
	v := a[0].StorageValue()
	v.ConfigurationHash = p.ConfigurationHash
	v.Boundaries = []string{"violence"}
	a[0] = auth.RoomSecret(v)
	if _, e := checkReadiness(context.Background(), tx, rt, p, c, a, nil); e == nil {
		t.Fatal("unsafe content admitted despite a private boundary")
	}
}
func TestAIRequiresCurrentBoundModelEvidenceAfterHumanConsent(t *testing.T) {
	c, p, a, tx, rt := gateFixture()
	c.Seats = append(c.Seats, SeatRule{ID: "ai", Required: true, Modes: []string{"ai"}, ModelCapabilities: []string{"structured-actions"}})
	p.ConfigurationHash = configurationHash(c)
	v := a[0].StorageValue()
	v.ConfigurationHash = p.ConfigurationHash
	a[0] = auth.RoomSecret(v)
	p.Slots = append(p.Slots, Slot{ID: "ai", Mode: "ai", ModelSelection: "server-selection"})
	if _, e := checkReadiness(context.Background(), tx, rt, p, c, a, nil); e == nil {
		t.Fatal("missing model gateway admitted AI seat")
	}
	m := &gateModels{}
	if _, e := checkReadiness(context.Background(), tx, rt, p, c, a, m); e == nil || m.calls != 1 {
		t.Fatal("denied model capability admitted")
	}
	m.allowed = true
	if _, e := checkReadiness(context.Background(), tx, rt, p, c, a, m); e != nil {
		t.Fatal("verified model capability rejected")
	}
	if m.received.Scope != p.Scope || m.received.GraphHash != p.GraphHash || m.received.ConfigurationHash != p.ConfigurationHash || m.received.Revision != p.Revision || m.received.SeatID != "ai" || len(m.received.Capabilities) != 1 {
		t.Fatal("model capability proof not bound to current game preparation")
	}
	v = a[0].StorageValue()
	v.Consent = false
	a[0] = auth.RoomSecret(v)
	before := m.calls
	if _, e := checkReadiness(context.Background(), tx, rt, p, c, a, m); e == nil || m.calls != before {
		t.Fatal("model processing ran before human consent")
	}
}
