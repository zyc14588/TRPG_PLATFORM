// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package model

import (
	"context"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"io"
	"slices"
	"strings"
	"unicode"
	"unicode/utf8"
)

// PresentationRegistry is an optional read-only extension of the existing
// cookie-bound model transaction. Missing enumeration or budget evidence is
// unavailable, rather than an implicit grant or a ready default.
type PresentationRegistry interface {
	PresentationConfigurations(context.Context, string, string) ([]Configuration, error)
	PresentationBudgetReady(context.Context, Configuration) (bool, error)
}
type PresentationSelection struct {
	SelectionID  string   `json:"selection_id"`
	Label        string   `json:"label"`
	SeatIDs      []string `json:"seat_ids"`
	Capabilities []string `json:"capabilities"`
	Ready        bool     `json:"ready"`
}
type PresentationSource struct {
	service *Service
	labels  map[string]string
}

func (*PresentationSource) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private presentation model source>")
}
func (*PresentationSource) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }

func presentationLabel(v string) bool {
	if !utf8.ValidString(v) || utf8.RuneCountInString(v) < 1 || utf8.RuneCountInString(v) > 80 || strings.TrimSpace(v) == "" {
		return false
	}
	for _, r := range v {
		if unicode.IsControl(r) {
			return false
		}
	}
	return true
}

// Labels annotate verified, live rows only. They cannot supply a selection or
// its permissions, certification, compatibility or readiness.
func NewPresentationSource(s *Service, labels map[string]string) (*PresentationSource, error) {
	if s.state() == nil {
		return nil, auth.ErrInvalid
	}
	owned := map[string]string{}
	if len(labels) > 4096 {
		return nil, auth.ErrInvalid
	}
	for key, label := range labels {
		p := strings.Split(key, "/")
		if len(p) != 2 || !scopeLabelID(p[0]) || !scopeLabelID(p[1]) || !presentationLabel(label) {
			return nil, auth.ErrInvalid
		}
		owned[key] = label
	}
	return &PresentationSource{s, owned}, nil
}
func scopeLabelID(v string) bool { return labels([]string{v}, 1) }
func (s *PresentationSource) UsesAuthority(a *auth.RoomAuthority) bool {
	return s != nil && s.service.state() != nil && s.service.state().authority == a
}
func (s *PresentationSource) UsesLaunch(l *launch.Service) bool {
	return s != nil && s.service.state() != nil && l.PlayerPresentationModels(s.service)
}

// Within shares the actual read's admission transaction. Every candidate is
// checked against current room participation/AI-seat control, preparation,
// certificates, credential ownership/lifetime, and aggregate budget evidence.
// Guests and unseated administrators receive no private AI-seat inventory.
func (s *PresentationSource) Within(ctx context.Context, tx auth.Transaction, caller auth.SessionData, graph launch.PresentationGraph) ([]PresentationSelection, error) {
	if s == nil || s.service.state() == nil || ctx == nil || ctx.Err() != nil || tx == nil {
		return nil, auth.ErrUnavailable
	}
	bound, e := auth.RoomAdmissionSession(tx.Core())
	if e != nil || bound.StorageValue().Hash != caller.Hash {
		return nil, auth.ErrDenied
	}
	g := graph.StorageValue()
	if !scopeLabelID(g.WorkspaceID) || !scopeLabelID(g.ConfigurationID) || !graphHash(g.ConfigurationHash) || !graphHash(g.GraphHash) || len(g.Seats) < 1 || len(g.Seats) > 64 {
		return nil, auth.ErrDenied
	}
	mt, e := s.service.state().storage.Bind(tx.Core())
	if e != nil {
		return nil, auth.SafeError(e)
	}
	registry, ok := mt.(PresentationRegistry)
	if !ok {
		return nil, auth.ErrUnavailable
	}
	rows, e := registry.PresentationConfigurations(ctx, g.WorkspaceID, g.ConfigurationID)
	if e != nil {
		return nil, auth.SafeError(e)
	}
	if len(rows) > 64 {
		return nil, auth.ErrUnavailable
	}
	byID := map[string]PresentationSelection{}
	for _, row := range rows {
		if ctx.Err() != nil {
			return nil, auth.ErrUnavailable
		}
		c := row.StorageValue()
		if c.Scope.WorkspaceID != g.WorkspaceID || c.ConfigurationID != g.ConfigurationID {
			return nil, auth.ErrDenied
		}
		if c.Revoked || c.ConfigurationHash != g.ConfigurationHash || c.GraphHash != g.GraphHash {
			continue
		}
		idx := slices.IndexFunc(g.Seats, func(r launch.SeatRule) bool { return r.ID == c.SeatID })
		if idx < 0 || !slices.Contains(g.Seats[idx].Modes, "ai") {
			continue
		}
		f, e := s.service.frame(ctx, tx.Core(), c.Scope, c.SeatID)
		if e != nil {
			if e == auth.ErrDenied {
				continue
			}
			return nil, auth.SafeError(e)
		}
		if _, e = f.access(ctx, tx.Core(), caller); e != nil {
			if e == auth.ErrDenied {
				continue
			}
			return nil, auth.SafeError(e)
		}
		if e = s.service.configurationCurrent(ctx, tx.Core(), f, c, g.Seats[idx].ModelCapabilities); e != nil {
			if e == auth.ErrDenied {
				continue
			}
			return nil, auth.SafeError(e)
		}
		cert, e := s.service.certificate(g.WorkspaceID, c.Primary)
		if e != nil {
			continue
		}
		// Expose only capabilities certified for this exact graph, not the raw
		// registry report or capabilities for other game graphs.
		caps := []string{}
		for _, game := range cert.Games {
			if game.GraphHash == g.GraphHash {
				for _, cap := range game.Capabilities {
					if slices.Contains(cert.Capabilities, cap) {
						caps = append(caps, cap)
					}
				}
			}
		}
		slices.Sort(caps)
		if !labels(caps, 64) || !scopeLabelID(c.Selection) {
			return nil, auth.ErrDenied
		}
		ready, e := registry.PresentationBudgetReady(ctx, row)
		if e != nil {
			return nil, auth.SafeError(e)
		}
		acks, e := f.lt.Acknowledgments(ctx, c.Scope)
		if e != nil {
			return nil, auth.SafeError(e)
		}
		proof := auth.RoomSecret(launch.ModelRequirementData{Scope: c.Scope, ConfigurationID: c.ConfigurationID, ConfigurationHash: c.ConfigurationHash, GraphHash: c.GraphHash, SeatID: c.SeatID, Selection: c.Selection, Revision: c.PreparationRevision, Capabilities: slices.Clone(g.Seats[idx].ModelCapabilities)})
		if e = s.service.Check(ctx, tx.Core(), proof, acks); e != nil {
			if auth.SafeError(e) != auth.ErrDenied {
				return nil, auth.SafeError(e)
			}
			ready = false
		}
		label := s.labels[g.WorkspaceID+"/"+c.Selection]
		if label == "" {
			label = c.Selection
		}
		if !presentationLabel(label) {
			return nil, auth.ErrUnavailable
		}
		old, exists := byID[c.Selection]
		if exists {
			if !slices.Equal(old.Capabilities, caps) || old.Label != label {
				return nil, auth.ErrDenied
			}
			if !slices.Contains(old.SeatIDs, c.SeatID) {
				old.SeatIDs = append(old.SeatIDs, c.SeatID)
			}
			old.Ready = old.Ready && ready
			byID[c.Selection] = old
		} else {
			byID[c.Selection] = PresentationSelection{c.Selection, label, []string{c.SeatID}, caps, ready}
		}
	}
	out := make([]PresentationSelection, 0, len(byID))
	for _, v := range byID {
		slices.Sort(v.SeatIDs)
		if len(v.SeatIDs) > 64 {
			return nil, auth.ErrUnavailable
		}
		out = append(out, v)
	}
	slices.SortFunc(out, func(a, b PresentationSelection) int { return strings.Compare(a.SelectionID, b.SelectionID) })
	return out, nil
}
