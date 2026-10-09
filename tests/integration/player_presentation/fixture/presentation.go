//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package fixture

import (
	"context"
	"crypto/rand"
	"encoding/json"
	"errors"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/credential"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/httpapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
	platformsession "github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
	"io"
	"log"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"sync/atomic"
	"testing"
	"time"
)

var productRoot string

func findProductRoot() (string, error) {
	d, e := os.Getwd()
	if e != nil {
		return "", e
	}
	for i := 0; i < 8; i++ {
		if b, e := os.ReadFile(filepath.Join(d, "go.mod")); e == nil && strings.HasPrefix(string(b), "module github.com/zyc14588/TRPG_PLATFORM") {
			return d, nil
		}
		d = filepath.Dir(d)
	}
	return "", errors.New("product root missing")
}
func ReadProductFile(name string) ([]byte, error) {
	return os.ReadFile(filepath.Join(productRoot, name))
}

type Harness struct {
	*playerFixture
	Models        *model.Service
	ProviderCalls atomic.Uint64
	certificate   model.Certification
	modelOptions  model.Options
}

func Limits() model.Limits {
	return model.Limits{Calls: 8, Tokens: 8192, CostMicros: 10000, LatencyMillis: 4000, Tools: 8, Subagents: 1, ContextBytes: 16384, LocalComputeMillis: 4000}
}
func New(t *testing.T, guest bool) *Harness { return newHarness(t, guest, aiBuild) }
func newHarness(t *testing.T, guest bool, build func(install.RuntimeConfig, string) (*archive.Package, install.PolicyConfig, error), dependencies ...*archive.Package) *Harness {
	t.Helper()
	l := newLaunchFixtureWithBuilder(t, nil, build, dependencies...)
	n := playerFixtureWithLaunch(t, l, guest)
	h := &Harness{playerFixture: n}
	n.api(t, n.owner, "configure", map[string]any{"configuration_id": "ai-required", "slots": append(n.humanSlots(), map[string]any{"id": "ai", "mode": "ai", "model_selection": "selected"})})
	pre, e := l.service.PlayerLobby(l.ctx, l.caller(l.owner, ""), l.w, n.room)
	need(t, e)
	p := pre.StorageValue().Preparation
	mt, e := postgres.NewPlatformModelStorage(l.r)
	need(t, e)
	need(t, mt.Bootstrap(l.ctx))
	bs, e := postgres.NewPlatformBudgetStorage(l.r)
	need(t, e)
	need(t, bs.Bootstrap(l.ctx))
	master := filepath.Join(t.TempDir(), "master")
	raw := make([]byte, 32)
	_, e = rand.Read(raw)
	need(t, e)
	need(t, os.WriteFile(master, raw, 0400))
	clear(raw)
	vault, e := credential.New(master)
	need(t, e)
	t.Cleanup(vault.Close)
	provider := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		h.ProviderCalls.Add(1)
		http.Error(w, "unexpected dispatch", 503)
	}))
	t.Cleanup(provider.Close)
	endpoint := provider.URL + "/v1"
	cert := model.NewCertification(model.CertificationData{ID: "certified", WorkspaceID: l.w, Tuple: model.Tuple{Model: "fixture:small", Endpoint: endpoint, Adapter: "openai-compatible", PromptTemplate: "safe-v1", ToolMode: "structured", TestVersion: "fixture-v1"}, Level: 3, Capabilities: []string{"structured-actions", "ai-player"}, Games: []model.GameEvidence{{GraphHash: p.GraphHash, TestVersion: "fixture-v1", EvidenceHash: strings.Repeat("a", 64), Capabilities: []string{"structured-actions", "ai-player"}}}, EvidenceHash: strings.Repeat("b", 64), ExpiresAt: time.Now().Add(time.Hour)})
	h.certificate = cert
	h.modelOptions = model.Options{Authority: l.authority, Rooms: l.store, Launches: l.storage, Storage: mt, Vault: vault, Endpoints: []model.Endpoint{model.NewEndpoint(model.EndpointData{ID: "local-only", URL: endpoint, Adapter: "openai-compatible", Models: []string{"fixture:small"}, AllowLANHTTP: true})}, Certifications: []model.Certification{cert}, Defaults: map[string]string{l.w: "certified"}, WorkspaceLimits: map[string]model.Limits{l.w: Limits()}}
	h.Models, e = model.New(h.modelOptions)
	need(t, e)
	v := n.owner.StorageValue()
	caller := auth.RoomSecret(model.CallerData{Credential: v.Cookie, CSRF: v.CSRF, IdempotencyKey: "key-" + token(t)})
	key, e := credential.NewKey([]byte("owned-B014-synthetic-private-provider-key"))
	need(t, e)
	need(t, h.Models.StoreCredential(l.ctx, caller, auth.RoomSecret(model.CredentialRequestData{Scope: p.Scope, SeatID: "ai", ID: "byok", Lifetime: credential.Retained, Key: key})))
	key.Close()
	c := caller.StorageValue()
	c.IdempotencyKey = "configure-" + token(t)
	_, e = h.Models.Configure(l.ctx, auth.RoomSecret(c), model.NewConfigureRequest(model.ConfigureRequestData{Scope: p.Scope, SeatID: "ai", Selection: "selected", CredentialID: "byok", Budget: Limits()}))
	need(t, e)
	l.models = h.Models
	l.recomposeLaunch(t, l.configs)
	n.composePlayer(t)
	h.Compose(t)
	return h
}
func (h *Harness) Context() context.Context                     { return h.ctx }
func (h *Harness) Repository() *postgres.PlatformAuthRepository { return h.r }
func (h *Harness) Authority() *auth.RoomAuthority               { return h.authority }
func (h *Harness) RoomStorage() room.Storage                    { return h.store }
func (h *Harness) Rooms() *room.Service                         { return h.rooms }
func (h *Harness) Configurations() []launch.Configuration       { return h.configs }
func (h *Harness) Policies() platformsession.PolicyProvider     { return h.policy }
func (h *Harness) Descriptions() []player.Description {
	out := []player.Description{}
	for _, c := range h.configs {
		v := c.StorageValue()
		out = append(out, player.Description{WorkspaceID: v.WorkspaceID, ConfigurationID: v.ID, GameID: "game", Title: "Owned installed game"})
	}
	return out
}
func (h *Harness) Workspace() string { return h.w }
func (h *Harness) Scope() core.Scope {
	return core.Scope{WorkspaceID: h.w, RoomID: h.room, GameID: "game"}
}
func (h *Harness) SQL(t *testing.T, q string) string { return h.sql(t, q) }
func (h *Harness) Compose(t *testing.T) {
	t.Helper()
	h.server.Close()
	source, e := model.NewPresentationSource(h.Models, map[string]string{h.w + "/selected": "Current verified AI"})
	need(t, e)
	schema, e := ReadProductFile("schemas/platform/platform-player-presentation-api-v1.schema.json")
	need(t, e)
	facade, e := player.NewPresentationService(h.players, source, schema)
	need(t, e)
	h.Serve(t, func(origin string) http.Handler {
		handler, e := httpapi.NewPlayerPresentationHandler(h.players, h.rooms, facade, origin)
		need(t, e)
		return handler
	})
}
func (h *Harness) Serve(t *testing.T, build func(string) http.Handler) {
	t.Helper()
	if h.server != nil {
		h.server.Close()
	}
	s := httptest.NewUnstartedServer(nil)
	s.Config.Handler = build("https://" + s.Listener.Addr().String())
	s.Config.ErrorLog = log.New(io.Discard, "", 0)
	s.StartTLS()
	s.Client().Timeout = 7 * time.Second
	h.server = s
	t.Cleanup(s.Close)
}
func (h *Harness) GET(t *testing.T, which, configuration string, mutate func(*http.Request)) (map[string]any, int, http.Header, []byte) {
	t.Helper()
	a := h.owner
	if which == "participant" {
		a = h.participant
	}
	if which == "unrelated" {
		a = h.account(t)
	}
	r, e := http.NewRequest("GET", h.server.URL+"/api/v1/workspaces/"+h.w+"/games/"+configuration+"/presentation", nil)
	need(t, e)
	r.AddCookie(&http.Cookie{Name: httpapi.SessionCookie, Value: a.StorageValue().Cookie.StorageValue()})
	r.Header.Set("Origin", h.server.URL)
	r.Header.Set("Sec-Fetch-Site", "same-origin")
	if mutate != nil {
		mutate(r)
	}
	res, e := h.server.Client().Do(r)
	need(t, e)
	defer res.Body.Close()
	b, e := io.ReadAll(io.LimitReader(res.Body, player.MaxResponseBytes+1))
	need(t, e)
	var v map[string]any
	if json.Unmarshal(b, &v) != nil {
		t.Fatal("bounded public JSON missing")
	}
	return v, res.StatusCode, res.Header, b
}
func (h *Harness) Baseline(t *testing.T) string {
	t.Helper()
	parts := []string{}
	for _, table := range []string{"platform_room.rooms", "platform_room.participants", "platform_launch.preparation", "platform_launch.acknowledgments", "platform_launch.sessions", "platform_model.credentials", "platform_model.configurations", "platform_budget.tasks", "platform_budget.reservations", "platform_budget.pauses", "platform_budget.counters"} {
		parts = append(parts, h.sql(t, "SELECT md5(coalesce(string_agg(row_to_json(v)::text,'' ORDER BY row_to_json(v)::text),'')) FROM "+table+" v WHERE workspace_id='"+h.w+"'"))
	}
	return checkpoint.Hash([]byte(strings.Join(parts, "/")))
}
func (h *Harness) RemoveMembership(t *testing.T) {
	_ = h.sql(t, "DELETE FROM platform_core.memberships WHERE workspace_id='"+h.w+"' AND account_id='"+h.owner.StorageValue().ID+"'")
}
func (h *Harness) RevokeCookie(t *testing.T) {
	_ = h.sql(t, "UPDATE platform_auth.sessions SET revoked=true WHERE token_hash='"+hashed(h.owner.StorageValue().Cookie.StorageValue())+"'")
}
func (h *Harness) RevokeModel(t *testing.T) {
	_ = h.sql(t, "UPDATE platform_model.configurations SET revoked=true WHERE workspace_id='"+h.w+"'")
}
func (h *Harness) RevokeCredential(t *testing.T) {
	_ = h.sql(t, "UPDATE platform_model.credentials SET revoked=true WHERE workspace_id='"+h.w+"'")
}
func (h *Harness) RevokeCertificate(t *testing.T) {
	need(t, h.Models.RevokeCertification(h.w, "certified"))
}
func (h *Harness) Confirm(t *testing.T) {
	p, e := h.service.PlayerLobby(h.ctx, h.caller(h.owner, ""), h.w, h.room)
	need(t, e)
	rev := p.StorageValue().Preparation.Revision
	need(t, h.acknowledge(t, h.owner, h.room, rev, true, true, true, nil, "owner-"+token(t)))
	need(t, h.acknowledge(t, h.participant, h.room, rev, true, true, true, nil, "participant-"+token(t)))
}
func (h *Harness) ExhaustBudget(t *testing.T) {
	b, _ := json.Marshal(Limits())
	zero, _ := json.Marshal(model.Limits{})
	_ = h.sql(t, "INSERT INTO platform_budget.counters(workspace_id,level,node_id,cap,used,held) VALUES('"+h.w+"','workspace','"+h.w+"',decode('"+hexBytes(b)+"','hex'),decode('"+hexBytes(b)+"','hex'),decode('"+hexBytes(zero)+"','hex'))")
}
func hexBytes(b []byte) string {
	const alphabet = "0123456789abcdef"
	out := make([]byte, len(b)*2)
	for i, v := range b {
		out[2*i] = alphabet[v>>4]
		out[2*i+1] = alphabet[v&15]
	}
	return string(out)
}
