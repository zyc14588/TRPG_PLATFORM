// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player

import (
	"strconv"
	"strings"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	platformsession "github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

type Slot struct {
	ID             string `json:"id"`
	Mode           string `json:"mode"`
	ParticipantID  string `json:"participant_id,omitempty"`
	ModelSelection string `json:"model_selection,omitempty"`
}
type Seat struct {
	ID       string   `json:"id"`
	Required bool     `json:"required"`
	Modes    []string `json:"modes"`
}
type Game struct {
	GameID          string   `json:"game_id"`
	ConfigurationID string   `json:"configuration_id"`
	Title           string   `json:"title"`
	ContentTags     []string `json:"content_tags"`
	SafetyTags      []string `json:"safety_tags"`
	Seats           []Seat   `json:"seats"`
}
type OwnConsent struct {
	Revision        string   `json:"revision"`
	Consent         bool     `json:"consent"`
	Ready           bool     `json:"ready"`
	SafetyConfirmed bool     `json:"safety_confirmed"`
	Boundaries      []string `json:"boundaries"`
}
type Readiness struct {
	Ready   bool     `json:"ready"`
	Reasons []string `json:"reasons"`
}
type Lobby struct {
	Revision          string     `json:"revision"`
	ConfigurationID   *string    `json:"configuration_id"`
	ConfigurationHash *string    `json:"configuration_hash"`
	GraphHash         *string    `json:"graph_hash"`
	Slots             []Slot     `json:"slots"`
	OwnConsent        OwnConsent `json:"own_consent"`
	Readiness         Readiness  `json:"readiness"`
	CanManage         bool       `json:"can_manage"`
}
type ControlDTO struct {
	Revision          string `json:"revision"`
	Paused            bool   `json:"paused"`
	Reason            string `json:"reason"`
	OwnResumeRequired bool   `json:"own_resume_required"`
}
type ConnectionDTO struct {
	ID        string     `json:"connection_id"`
	SessionID string     `json:"session_id"`
	SeatID    string     `json:"seat_id"`
	Expires   int        `json:"expires_in_seconds"`
	Control   ControlDTO `json:"control"`
}
type Event struct {
	Sequence  string           `json:"sequence"`
	Version   string           `json:"version"`
	CommandID string           `json:"command_id"`
	Type      string           `json:"type"`
	Data      checkpoint.Value `json:"data"`
}
type Snapshot struct {
	SessionID string           `json:"session_id"`
	SeatID    string           `json:"seat_id"`
	Version   string           `json:"state_version"`
	Cursor    string           `json:"event_cursor"`
	View      checkpoint.Value `json:"view"`
	Events    []Event          `json:"events"`
	More      bool             `json:"more"`
	Ended     bool             `json:"ended"`
	Control   ControlDTO       `json:"control"`
}
type Result struct {
	CommandID string           `json:"command_id"`
	Version   string           `json:"state_version"`
	Cursor    string           `json:"event_cursor"`
	Replayed  bool             `json:"replayed"`
	Result    checkpoint.Value `json:"result"`
	Control   ControlDTO       `json:"control"`
}
type Export struct {
	FormatVersion int                        `json:"format_version"`
	Kind          platformsession.ExportKind `json:"kind"`
	SessionID     string                     `json:"session_id"`
	SeatID        string                     `json:"seat_id"`
	Version       string                     `json:"state_version"`
	Cursor        string                     `json:"event_cursor"`
	NextCursor    string                     `json:"next_cursor"`
	More          bool                       `json:"more"`
	Ended         bool                       `json:"ended"`
	View          checkpoint.Value           `json:"view"`
	Events        []Event                    `json:"events"`
}
type RecoveryPoint struct {
	SessionID string `json:"session_id"`
	Version   string `json:"state_version"`
	Cursor    string `json:"event_cursor"`
}

func decimal(v uint64) string { return strconv.FormatUint(v, 10) }
func lobbyDTO(v launch.PlayerLobby) Lobby {
	x := v.StorageValue()
	p := x.Preparation
	a := x.Own
	l := Lobby{Revision: decimal(p.Revision), Slots: []Slot{}, OwnConsent: OwnConsent{Revision: decimal(a.Revision), Consent: a.Consent, Ready: a.Ready, SafetyConfirmed: a.SafetyConfirmed, Boundaries: append([]string{}, a.Boundaries...)}, Readiness: Readiness{Ready: x.Ready, Reasons: []string{}}, CanManage: x.CanManage}
	if x.Configured {
		l.ConfigurationID = &p.ConfigurationID
		l.ConfigurationHash = &p.ConfigurationHash
		l.GraphHash = &p.GraphHash
	}
	for _, s := range p.Slots {
		l.Slots = append(l.Slots, Slot{s.ID, s.Mode, s.ParticipantID, s.ModelSelection})
	}
	if !x.Ready {
		if !x.Configured {
			l.Readiness.Reasons = append(l.Readiness.Reasons, "not_configured")
		} else {
			l.Readiness.Reasons = append(l.Readiness.Reasons, "participant_not_ready")
		}
	}
	return l
}
func controlDTO(s State, principal string) ControlDTO {
	v := s.StorageValue()
	return ControlDTO{decimal(v.Revision), v.Paused, v.Reason, v.Paused && needed(v, principal) && !contains(v.Resumed, principal)}
}
func contains(ss []string, s string) bool {
	for _, x := range ss {
		if x == s {
			return true
		}
	}
	return false
}
func eventDTO(events []data.JournalEvent) ([]Event, error) {
	out := []Event{}
	if len(events) > 128 {
		return nil, auth.ErrUnavailable
	}
	for _, e := range events {
		if _, x := counter(decimal(e.Sequence)); x != nil {
			return nil, auth.ErrUnavailable
		}
		if _, x := counter(decimal(e.Version)); x != nil || checkpoint.Validate(e.Event.Payload) != nil {
			return nil, auth.ErrUnavailable
		}
		// Filtering uses the original qualified type. The public DTO presents
		// its local identifier; the immutable native event remains qualified.
		typ := e.Event.Type
		if i := strings.LastIndexByte(typ, '/'); i >= 0 {
			typ = typ[i+1:]
		}
		if !store.ValidID(typ) {
			return nil, auth.ErrUnavailable
		}
		out = append(out, Event{decimal(e.Sequence), decimal(e.Version), e.CommandID, typ, e.Event.Payload})
	}
	return out, nil
}
