//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package fixture

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	fixtures "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
	"strings"
	"testing"
)

func NewWithDependency(t *testing.T) *Harness {
	t.Helper()
	leaf, e := fixtures.Build(fixtures.Files("fixture.example/content", "content", ""))
	need(t, e)
	build := func(runtime install.RuntimeConfig, source string) (*archive.Package, install.PolicyConfig, error) {
		root, pc, e := aiBuild(runtime, source)
		if e != nil {
			return nil, pc, e
		}
		files := map[string][]byte{}
		for _, entry := range root.Entries() {
			files[entry.Path()] = entry.Bytes()
		}
		files[archive.ManifestPath] = append(files[archive.ManifestPath], []byte("\n[[dependencies]]\npackage_id=\"fixture.example/content\"\nversion=\"1.0.0\"\n")...)
		node := leaf.ExactLock().Packages()[0]
		full, e := fixtures.Build(files, node)
		if e != nil {
			return nil, pc, e
		}
		approval := pc.Artifacts[string(root.ArtifactIdentity().Digest())]
		delete(pc.Artifacts, string(root.ArtifactIdentity().Digest()))
		pc.Artifacts[string(full.ArtifactIdentity().Digest())] = approval
		m, e := leaf.Manifest()
		if e != nil {
			return nil, pc, e
		}
		pc.Artifacts[string(leaf.ArtifactIdentity().Digest())] = install.Approval{RightsDigest: install.RightsDigest(*m.Package), Retention: "synthetic", Safety: "ACTIVE"}
		return full, pc, nil
	}
	return newHarness(t, false, build, leaf)
}
func (h *Harness) ChangeConfiguration(t *testing.T, id string) {
	slots := h.humanSlots()
	if id == "ai-required" {
		slots = append(slots, map[string]any{"id": "ai", "mode": "ai", "model_selection": "selected"})
	}
	h.api(t, h.owner, "configure", map[string]any{"configuration_id": id, "slots": slots})
}
func (h *Harness) BreakGraphMapping(t *testing.T, missing bool) {
	configs := []launch.Configuration{}
	for _, c := range h.configs {
		v := c.StorageValue()
		if v.ID == "ai-required" {
			if missing {
				v.Request.Dependencies = nil
			} else {
				v.Request.Root = "sha256:" + strings.Repeat("f", 64)
			}
		}
		configs = append(configs, auth.RoomSecret(v))
	}
	h.configs = configs
	h.recomposeLaunch(t, configs)
	h.composePlayer(t)
	h.Compose(t)
}
func (h *Harness) UnseatHost(t *testing.T) {
	_ = h.sql(t, "UPDATE platform_room.participants SET active=false WHERE workspace_id='"+h.w+"' AND id='"+h.hostPart+"'")
}
func (h *Harness) DisableGuest(t *testing.T) {
	_ = h.sql(t, "UPDATE platform_core.guests SET disabled=true WHERE workspace_id='"+h.w+"' AND room_id='"+h.room+"'")
}
func (h *Harness) AddParticipantMembership(t *testing.T) {
	need(t, h.r.Transact(h.ctx, func(tx auth.Transaction) error {
		return tx.Core().PutMembership(h.ctx, core.Membership{WorkspaceID: h.w, AccountID: h.participant.StorageValue().ID, Role: core.Member})
	}))
}
func (h *Harness) RemoveParticipantMembership(t *testing.T) {
	need(t, h.r.Transact(h.ctx, func(tx auth.Transaction) error {
		return tx.Core().DeleteMembership(h.ctx, h.w, h.participant.StorageValue().ID)
	}))
}
func (h *Harness) DirectCanceled(t *testing.T) error {
	source, e := model.NewPresentationSource(h.Models, nil)
	need(t, e)
	schema, e := ReadProductFile("schemas/platform/platform-player-presentation-api-v1.schema.json")
	need(t, e)
	facade, e := player.NewPresentationService(h.players, source, schema)
	need(t, e)
	ctx, cancel := context.WithCancel(h.ctx)
	cancel()
	_, e = facade.Read(ctx, h.caller(h.owner, ""), h.w, "ai-required")
	return e
}
