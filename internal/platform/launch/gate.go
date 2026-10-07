// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package launch

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
	"math"
	"slices"
	"strings"
	"unicode"
	"unicode/utf8"
)

func label(v string) bool {
	if !utf8.ValidString(v) || len(v) > 256 || strings.TrimSpace(v) == "" {
		return false
	}
	for _, c := range v {
		if unicode.IsControl(c) {
			return false
		}
	}
	return true
}
func labels(xs []string, max int) bool {
	if len(xs) > max {
		return false
	}
	seen := map[string]bool{}
	for _, x := range xs {
		if !label(x) || seen[x] {
			return false
		}
		seen[x] = true
	}
	return true
}
func configuration(v ConfigurationData) bool {
	if !store.ValidID(v.ID) || !store.ValidID(v.WorkspaceID) || v.Factory == nil || len(v.Seats) < 1 || len(v.Seats) > 64 || !labels(v.ContentTags, 64) || !labels(v.SafetyTags, 64) || v.Request.Workspace != v.WorkspaceID || len(v.Request.Dependencies) > 127 {
		return false
	}
	for _, tag := range v.SafetyTags {
		if !store.ValidID(tag) {
			return false
		}
	}
	for _, tag := range v.ContentTags {
		if !slices.Contains(v.SafetyTags, tag) {
			return false
		}
	}
	seen := map[string]bool{}
	required := false
	for _, s := range v.Seats {
		if !store.ValidID(s.ID) || seen[s.ID] || len(s.Modes) < 1 || len(s.Modes) > 2 || !labels(s.ModelCapabilities, 32) {
			return false
		}
		seen[s.ID] = true
		required = required || s.Required
		modes := map[string]bool{}
		for _, m := range s.Modes {
			if (m != "human" && m != "ai") || modes[m] {
				return false
			}
			modes[m] = true
		}
	}
	return required
}
func validPreparation(p PreparationData, c ConfigurationData) bool {
	if p.ConfigurationID != c.ID || p.ConfigurationHash != configurationHash(c) || p.Scope.WorkspaceID != c.WorkspaceID || !store.ValidID(p.Scope.RoomID) || !store.ValidID(p.Scope.GameID) || p.Revision < 1 || p.Revision >= math.MaxInt64 || !checkpoint.IsDigest(p.GraphHash) || len(p.Slots) > len(c.Seats) {
		return false
	}
	seen := map[string]bool{}
	humans := map[string]bool{}
	for _, slot := range p.Slots {
		if seen[slot.ID] {
			return false
		}
		seen[slot.ID] = true
		idx := slices.IndexFunc(c.Seats, func(r SeatRule) bool { return r.ID == slot.ID })
		if idx < 0 || !slices.Contains(c.Seats[idx].Modes, slot.Mode) {
			return false
		}
		if slot.Mode == "human" {
			if !store.ValidID(slot.ParticipantID) || slot.ModelSelection != "" || humans[slot.ParticipantID] {
				return false
			}
			humans[slot.ParticipantID] = true
		} else if slot.ParticipantID != "" || !store.ValidID(slot.ModelSelection) {
			return false
		}
	}
	return true
}
func sessionRequest(c ConfigurationData, s core.Scope) (r installRequest) {
	return installRequest{configuration: c, scope: s}
}

// Retain credentials/attestations in the protected configuration; no caller
// can substitute a graph, installation credential or workspace.
type installRequest struct {
	configuration ConfigurationData
	scope         core.Scope
}

func (r installRequest) authenticate(ctx context.Context) (string, error) {
	q := r.configuration.Request
	q.Workspace = r.scope.WorkspaceID
	q.Session = r.scope.GameID
	v, e := r.configuration.Factory.Authenticate(ctx, q)
	if e != nil {
		return "", auth.ErrDenied
	}
	if v.Graph == nil || v.Graph.Root() == nil {
		return "", auth.ErrDenied
	}
	m, e := v.Graph.Root().Manifest()
	if e != nil || m.Package == nil || m.Package.PackageKind != model.PackageKindGameSystem {
		return "", auth.ErrDenied
	}
	if v.Binding.Workspace != r.scope.WorkspaceID || v.Binding.Session != r.scope.GameID || !checkpoint.IsDigest(v.Binding.GraphHash) {
		return "", auth.ErrDenied
	}
	return v.Binding.GraphHash, nil
}
func active(ctx context.Context, tx core.Transaction, p room.ParticipantData, scope core.Scope) error {
	if !p.Active || p.Scope != scope || !store.ValidID(p.ID) {
		return auth.ErrDenied
	}
	if p.AccountID != "" {
		a, e := tx.Account(ctx, p.AccountID)
		if e != nil || a.Disabled {
			return auth.ErrDenied
		}
	}
	if p.GuestID != "" {
		g, e := tx.Guest(ctx, scope, p.GuestID)
		now, n := tx.Now(ctx)
		if e != nil || n != nil || g.Disabled || !now.Before(g.ExpiresAt) || g.ClaimedAccountID != p.AccountID {
			return auth.ErrDenied
		}
	} else if p.AccountID == "" {
		return auth.ErrDenied
	}
	return nil
}
func participant(ctx context.Context, tx core.Transaction, rt room.Transaction, r room.RoomData, v auth.SessionData) (room.ParticipantData, error) {
	var p room.Participant
	var e error
	if v.Kind == "account" {
		p, e = rt.AccountParticipant(ctx, r.Scope, v.AccountID)
	} else if v.Kind == "guest" && v.Scope == r.Scope {
		p, e = rt.GuestParticipant(ctx, r.Scope, v.GuestID)
	} else {
		return room.ParticipantData{}, auth.ErrDenied
	}
	if e != nil {
		return room.ParticipantData{}, auth.ErrDenied
	}
	d := p.StorageValue()
	if e = active(ctx, tx, d, r.Scope); e != nil {
		return room.ParticipantData{}, e
	}
	return d, nil
}
func manager(ctx context.Context, tx core.Transaction, rt room.Transaction, r room.RoomData, v auth.SessionData) error {
	if v.Kind != "account" {
		return auth.ErrDenied
	}
	a, e := tx.Account(ctx, v.AccountID)
	if e != nil || a.Disabled {
		return auth.ErrDenied
	}
	m, e := tx.Membership(ctx, r.Scope.WorkspaceID, v.AccountID)
	if e != nil {
		return auth.ErrDenied
	}
	if (v.AccountID == r.Owner && m.Role == core.Owner) || m.Role == core.Admin {
		return nil
	}
	if ok, e := rt.Manager(ctx, r.Scope, v.AccountID); e == nil && ok {
		return nil
	}
	return auth.ErrDenied
}
func checkReadiness(ctx context.Context, tx core.Transaction, rt room.Transaction, p PreparationData, c ConfigurationData, acks []Acknowledgment, models ModelChecker) (string, error) {
	if !validPreparation(p, c) || len(acks) > 64 {
		return "", auth.ErrDenied
	}
	for _, rule := range c.Seats {
		if rule.Required && !slices.ContainsFunc(p.Slots, func(s Slot) bool { return s.ID == rule.ID }) {
			return "", auth.ErrDenied
		}
	}
	ps, e := rt.Participants(ctx, p.Scope)
	if e != nil || len(ps) < 1 || len(ps) > 64 {
		return "", auth.ErrDenied
	}
	participants := map[string]room.ParticipantData{}
	for _, v := range ps {
		d := v.StorageValue()
		if e = active(ctx, tx, d, p.Scope); e != nil {
			return "", e
		}
		if _, exists := participants[d.ID]; exists {
			return "", auth.ErrDenied
		}
		participants[d.ID] = d
	}
	humans := map[string]string{}
	bootstrap := ""
	for _, slot := range p.Slots {
		if slot.Mode == "human" {
			if _, ok := participants[slot.ParticipantID]; !ok {
				return "", auth.ErrDenied
			}
			humans[slot.ParticipantID] = slot.ID
			if bootstrap == "" {
				bootstrap = slot.ID
			}
			continue
		}
		if models == nil {
			return "", auth.ErrDenied
		}
	}
	// An admitted but unseated player cannot become a V1 observer through launch.
	if len(humans) != len(participants) || bootstrap == "" {
		return "", auth.ErrDenied
	}
	seen := map[string]bool{}
	for _, v := range acks {
		a := v.StorageValue()
		if a.Scope != p.Scope || a.Revision != p.Revision || a.ConfigurationHash != p.ConfigurationHash || a.GraphHash != p.GraphHash || seen[a.ParticipantID] || !a.Consent || !a.Ready || !a.SafetyConfirmed || !labels(a.Boundaries, 64) {
			return "", auth.ErrDenied
		}
		if _, ok := humans[a.ParticipantID]; !ok {
			return "", auth.ErrDenied
		}
		seen[a.ParticipantID] = true
		for _, boundary := range a.Boundaries {
			if !slices.Contains(c.SafetyTags, boundary) || slices.Contains(c.ContentTags, boundary) {
				return "", auth.ErrDenied
			}
		}
	}
	if len(seen) != len(humans) {
		return "", auth.ErrDenied
	}
	for _, slot := range p.Slots {
		if slot.Mode != "ai" {
			continue
		}
		idx := slices.IndexFunc(c.Seats, func(x SeatRule) bool { return x.ID == slot.ID })
		proof := auth.RoomSecret(ModelRequirementData{Scope: p.Scope, ConfigurationID: c.ID, ConfigurationHash: p.ConfigurationHash, GraphHash: p.GraphHash, SeatID: slot.ID, Selection: slot.ModelSelection, Revision: p.Revision, Capabilities: append([]string(nil), c.Seats[idx].ModelCapabilities...)})
		if e = models.Check(ctx, tx, proof, acks); e != nil {
			return "", auth.SafeError(e)
		}
	}
	return bootstrap, nil
}
func canonical(v any) ([]byte, error) {
	b, e := json.Marshal(v)
	if e != nil || len(b) > 16384 {
		return nil, auth.ErrInvalid
	}
	if _, e = auth.RoomJSON(b); e != nil {
		return nil, e
	}
	return b, nil
}
func configurationHash(c ConfigurationData) string {
	// This binds all server metadata shown during preparation even across a
	// restart/reconfiguration with the same ID. Credential and private keys are
	// excluded; exact artifacts/current trust remain independently authenticated.
	d := struct {
		ID, Workspace, Root string
		Dependencies        []string
		Seats               []SeatRule
		Content, Safety     []string
	}{c.ID, c.WorkspaceID, c.Request.Root, c.Request.Dependencies, c.Seats, c.ContentTags, c.SafetyTags}
	raw, _ := json.Marshal(d)
	sum := sha256.Sum256(append([]byte("platform-launch-configuration/v1\x00"), raw...))
	return "sha256:" + hex.EncodeToString(sum[:])
}
