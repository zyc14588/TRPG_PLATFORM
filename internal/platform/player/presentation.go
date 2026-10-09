// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"github.com/santhosh-tekuri/jsonschema/v6"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"io"
	"slices"
	"time"
)

const presentationSchemaID = "urn:trpg-platform:platform-player-presentation-api:v1"

// Canonical accepted product Schema; independent of private planning receipts.
const presentationSchemaDigest = "4d34b37b0faf35b5b864a857f68edd38e30b87bbdaf692b4f8240abbcc3b915a"

type Presentation struct {
	WorkspaceID       string                        `json:"workspace_id"`
	ConfigurationID   string                        `json:"configuration_id"`
	GameID            string                        `json:"game_id"`
	ConfigurationHash string                        `json:"configuration_hash"`
	GraphHash         string                        `json:"graph_hash"`
	Packages          []install.PresentationPackage `json:"packages"`
	ModelSelections   []model.PresentationSelection `json:"model_selections"`
}
type PresentationService struct {
	players *Service
	models  *model.PresentationSource
	schemas map[string]*jsonschema.Schema
}

func (*PresentationService) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private player presentation facade>")
}
func (*PresentationService) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }

func compilePresentation(raw []byte) (map[string]*jsonschema.Schema, error) {
	v, e := auth.RoomContractJSON(raw)
	if e != nil {
		return nil, auth.ErrInvalid
	}
	m, ok := v.(map[string]any)
	if !ok || m["$id"] != presentationSchemaID || m["$schema"] != "https://json-schema.org/draft/2020-12/schema" || m["x-status"] != "ACTIVE" || m["x-section-id"] != "SCHEMA-PLATFORM-PLAYER-PRESENTATION-API-V1" {
		return nil, auth.ErrInvalid
	}
	var b bytes.Buffer
	enc := json.NewEncoder(&b)
	enc.SetEscapeHTML(false)
	if enc.Encode(m) != nil {
		return nil, auth.ErrInvalid
	}
	h := sha256.Sum256(bytes.TrimSuffix(b.Bytes(), []byte("\n")))
	if hex.EncodeToString(h[:]) != presentationSchemaDigest {
		return nil, auth.ErrInvalid
	}
	defs, ok := m["$defs"].(map[string]any)
	if !ok || len(defs) != 8 {
		return nil, auth.ErrInvalid
	}
	names := []string{"ID", "Digest", "Label", "Package", "ModelSelection", "Presentation", "PresentationResponse", "ErrorResponse"}
	c := jsonschema.NewCompiler()
	c.DefaultDraft(jsonschema.Draft2020)
	c.AssertFormat()
	c.UseLoader(denyLoader{})
	if c.AddResource(presentationSchemaID, v) != nil {
		return nil, auth.ErrInvalid
	}
	if _, e = c.Compile(presentationSchemaID); e != nil {
		return nil, auth.ErrInvalid
	}
	out := map[string]*jsonschema.Schema{}
	for _, name := range names {
		if _, ok := defs[name]; !ok {
			return nil, auth.ErrInvalid
		}
		s, e := c.Compile(presentationSchemaID + "#/$defs/" + name)
		if e != nil {
			return nil, auth.ErrInvalid
		}
		out[name] = s
	}
	return out, nil
}

func NewPresentationService(players *Service, models *model.PresentationSource, schema []byte) (*PresentationService, error) {
	d := players.state()
	if d == nil || !models.UsesAuthority(d.options.Authority) || !models.UsesLaunch(d.options.Launch) {
		return nil, auth.ErrInvalid
	}
	schemas, e := compilePresentation(schema)
	if e != nil {
		return nil, e
	}
	return &PresentationService{players, models, schemas}, nil
}
func (s *PresentationService) UsesPlayers(p *Service) bool {
	return s != nil && s.players == p && p.state() != nil
}
func (s *PresentationService) Authentication() *auth.Service {
	if s == nil {
		return nil
	}
	return s.players.Authentication()
}

func (s *PresentationService) encode(v Presentation) (auth.Outcome, error) {
	if s == nil || s.schemas["PresentationResponse"] == nil {
		return auth.Outcome{}, auth.ErrUnavailable
	}
	raw, e := json.Marshal(struct {
		Version   int          `json:"schema_version"`
		RequestID string       `json:"request_id"`
		Data      Presentation `json:"data"`
	}{1, "response", v})
	if e != nil || len(raw) > MaxResponseBytes {
		return auth.Outcome{}, auth.ErrUnavailable
	}
	dec := json.NewDecoder(bytes.NewReader(raw))
	dec.UseNumber()
	var value any
	if dec.Decode(&value) != nil || s.schemas["PresentationResponse"].Validate(value) != nil {
		return auth.Outcome{}, auth.ErrUnavailable
	}
	return auth.RoomOutcome(raw), nil
}

func (s *PresentationService) Read(ctx context.Context, caller launch.Caller, w, id string) (auth.Outcome, error) {
	return s.read(ctx, caller, w, id, false)
}

func (s *PresentationService) ReadRoom(ctx context.Context, caller launch.Caller, w, room string) (auth.Outcome, error) {
	return s.read(ctx, caller, w, room, true)
}

func (s *PresentationService) read(ctx context.Context, caller launch.Caller, w, id string, room bool) (auth.Outcome, error) {
	if s == nil || s.players.state() == nil || ctx == nil || ctx.Err() != nil {
		return auth.Outcome{}, auth.ErrUnavailable
	}
	if !store.ValidID(w) || !store.ValidID(id) {
		return auth.Outcome{}, auth.ErrInvalid
	}
	d := s.players.state()
	d.mu.Lock()
	closed := d.closed
	d.mu.Unlock()
	if closed || d.options.Context.Err() != nil {
		return auth.Outcome{}, auth.ErrUnavailable
	}
	g, ok := d.games[w+"/"+id]
	if !room && !ok {
		return auth.Outcome{}, auth.ErrDenied
	}
	ctx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()
	var out auth.Outcome
	e := d.options.Authority.Inspect(ctx, caller.StorageValue().Credential, "", false, func(ctx context.Context, tx auth.Transaction, v auth.SessionData) error {
		var graph launch.PresentationGraph
		var e error
		if room {
			graph, e = d.options.Launch.PlayerRoomPresentationWithin(ctx, tx, v, w, id)
		} else {
			graph, e = d.options.Launch.PlayerPresentationWithin(ctx, tx, v, w, id, g.GameID)
		}
		if e != nil {
			return e
		}
		x := graph.StorageValue()
		var models []model.PresentationSelection
		if room {
			g, ok = d.games[w+"/"+x.ConfigurationID]
			if !ok || g.GameID != x.Scope.GameID {
				return auth.ErrDenied
			}
			models, e = s.models.WithinRoom(ctx, tx, v, graph)
		} else {
			models, e = s.models.Within(ctx, tx, v, graph)
		}
		if e != nil {
			return e
		}
		if ctx.Err() != nil {
			return auth.ErrUnavailable
		}
		// Lists are complete and owned. Never substitute a catalog, cached graph,
		// endpoint alias, or a consent/launch/readiness result for these facts.
		out, e = s.encode(Presentation{w, x.ConfigurationID, g.GameID, x.ConfigurationHash, x.GraphHash, slices.Clone(x.Packages), models})
		return e
	})
	if e != nil || ctx.Err() != nil {
		if ctx.Err() != nil {
			return auth.Outcome{}, auth.ErrUnavailable
		}
		return auth.Outcome{}, SafeError(e)
	}
	return out, nil
}
