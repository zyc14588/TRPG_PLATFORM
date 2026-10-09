//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package fixture

import (
	"context"
	"crypto/rand"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/credential"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	packages "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

func (h *Harness) RoomGET(t *testing.T, which, roomID string, mutate func(*http.Request)) (map[string]any, int, http.Header, []byte) {
	t.Helper()
	return h.GET(t, which, "ai-required", func(r *http.Request) {
		r.URL.Path = "/api/v1/workspaces/" + h.w + "/rooms/" + roomID + "/presentation"
		if mutate != nil {
			mutate(r)
		}
	})
}

func (h *Harness) RoomConsent(t *testing.T, which string) map[string]any {
	t.Helper()
	a := h.owner
	if which == "participant" {
		a = h.participant
	}
	lobby := h.api(t, a, "lobby", nil)
	h.api(t, a, "consent", map[string]any{"revision": lobby["revision"], "consent": true, "ready": true, "safety_confirmed": true, "boundaries": []string{}})
	return h.api(t, a, "lobby", nil)
}

func (h *Harness) ClaimRoomGuest(t *testing.T) {
	t.Helper()
	a := h.participant.StorageValue()
	q := h.authRequest(t, "claim", map[string]any{"mode": "new_account", "login_name": fmt.Sprintf("claim_%s_%d", prefix, sequence.Add(1)), "password": "synthetic-room-claim-password", "display_name": "Claimed participant"})
	out, e := h.authority.Authentication().Mutate(h.ctx, a.Cookie, a.CSRF, "room-claim-"+token(t), "owned-network", q)
	need(t, e)
	d := public(t, out)
	id := h.sql(t, "SELECT account_id FROM platform_room.participants WHERE workspace_id='"+h.w+"' AND id='"+h.playerPart+"'")
	if id == "" {
		t.Fatal("actual claim did not retain the room participation")
	}
	claimContext, ok := d["context"].(map[string]any)
	if !ok {
		t.Fatal("actual claim context missing")
	}
	csrf, ok := claimContext["csrf_token"].(string)
	if !ok || csrf == "" {
		t.Fatal("actual claim CSRF missing")
	}
	h.participant = auth.RoomSecret(actorData{ID: id, Cookie: out.StorageValue().Cookie, CSRF: csrf})
}

func (h *Harness) ParticipantOwnWorkspace(t *testing.T) string {
	t.Helper()
	w := fmt.Sprintf("foreign_%s_%d", prefix, sequence.Add(1))
	id := h.participant.StorageValue().ID
	need(t, h.r.Transact(h.ctx, func(tx auth.Transaction) error {
		if e := tx.Core().InsertWorkspace(h.ctx, core.Workspace{ID: w, Name: "Independent account workspace", OwnerID: id}); e != nil {
			return e
		}
		return tx.Core().PutMembership(h.ctx, core.Membership{WorkspaceID: w, AccountID: id, Role: core.Owner})
	}))
	return w
}

func (h *Harness) PendingRoomInvitation(t *testing.T) {
	t.Helper()
	a := h.account(t)
	admission := h.join(t, a, h.invite(t, h.room, true, 2))
	if admission["status"] != "pending" {
		t.Fatal("invitation-only account was already admitted")
	}
	h.participant = a
}

func (h *Harness) UnseatedRoomManager(t *testing.T) {
	t.Helper()
	h.AddParticipantMembership(t)
	h.call(t, h.owner, "set_role", h.w, h.room, h.playerPart, map[string]any{"role": "administrator", "enabled": true})
	h.RevokeRoomBasis(t, "participant")
}

func (h *Harness) RoomRelationshipCounts(t *testing.T) string {
	t.Helper()
	return h.sql(t, "SELECT (SELECT count(*) FROM platform_core.memberships WHERE workspace_id='"+h.w+"')||'/'||(SELECT count(*) FROM platform_room.participants WHERE workspace_id='"+h.w+"' AND room_id='"+h.room+"')||'/'||(SELECT count(*) FROM platform_launch.preparation WHERE workspace_id='"+h.w+"' AND room_id='"+h.room+"')")
}

func (h *Harness) RoomBaseline(t *testing.T) string {
	t.Helper()
	parts := []string{h.Baseline(t)}
	for _, table := range []string{"platform_core.memberships", "platform_core.guests"} {
		parts = append(parts, h.sql(t, "SELECT md5(coalesce(string_agg(row_to_json(v)::text,'' ORDER BY row_to_json(v)::text),'')) FROM "+table+" v WHERE workspace_id='"+h.w+"'"))
	}
	owner, participant := hashed(h.owner.StorageValue().Cookie.StorageValue()), hashed(h.participant.StorageValue().Cookie.StorageValue())
	parts = append(parts, h.sql(t, "SELECT md5(coalesce(string_agg(row_to_json(v)::text,'' ORDER BY row_to_json(v)::text),'')) FROM platform_auth.sessions v WHERE token_hash IN('"+owner+"','"+participant+"')"))
	parts = append(parts, h.sql(t, "SELECT count(*) FROM platform_auth.receipts WHERE owner_hash IN('"+owner+"','"+participant+"')"))
	h.mu.Lock()
	count := 0
	for _, x := range h.executions {
		if strings.HasPrefix(x.Case, "session-vm") {
			count++
		}
	}
	h.mu.Unlock()
	parts = append(parts, fmt.Sprint(count), fmt.Sprint(h.ProviderCalls.Load()))
	return checkpoint.Hash([]byte(strings.Join(parts, "/")))
}

func (h *Harness) RevokeRoomBasis(t *testing.T, kind string) {
	t.Helper()
	switch kind {
	case "participant":
		_ = h.sql(t, "UPDATE platform_room.participants SET active=false WHERE workspace_id='"+h.w+"' AND id='"+h.playerPart+"'")
	case "account":
		need(t, h.r.Transact(h.ctx, func(tx auth.Transaction) error {
			return tx.Core().DisableAccount(h.ctx, h.participant.StorageValue().ID)
		}))
	case "cookie":
		_ = h.sql(t, "UPDATE platform_auth.sessions SET revoked=true WHERE token_hash='"+hashed(h.participant.StorageValue().Cookie.StorageValue())+"'")
	case "cookie_expired":
		_ = h.sql(t, "UPDATE platform_auth.sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE token_hash='"+hashed(h.participant.StorageValue().Cookie.StorageValue())+"'")
	case "guest_expired":
		_ = h.sql(t, "UPDATE platform_core.guests SET expires_at=clock_timestamp()-interval '1 second' WHERE workspace_id='"+h.w+"' AND room_id='"+h.room+"'")
	case "guest_revoked":
		h.DisableGuest(t)
	case "closed":
		_ = h.sql(t, "UPDATE platform_room.rooms SET state='closed' WHERE workspace_id='"+h.w+"' AND room_id='"+h.room+"'")
	case "preparation":
		_ = h.sql(t, "DELETE FROM platform_launch.preparation WHERE workspace_id='"+h.w+"' AND room_id='"+h.room+"'")
	case "malformed_preparation":
		_ = h.sql(t, "UPDATE platform_launch.preparation SET body=decode('7b7d','hex') WHERE workspace_id='"+h.w+"' AND room_id='"+h.room+"'")
	case "malformed_model":
		_ = h.sql(t, "UPDATE platform_model.configurations SET body=decode('7b7d','hex') WHERE workspace_id='"+h.w+"' AND room_id='"+h.room+"'")
	default:
		t.Fatal("unrecognized bounded fixture revocation")
	}
}

func (h *Harness) modelCaller(t *testing.T) model.Caller {
	t.Helper()
	a := h.owner.StorageValue()
	return auth.RoomSecret(model.CallerData{Credential: a.Cookie, CSRF: a.CSRF, IdempotencyKey: "room-model-" + token(t)})
}

// AddSecondRoom uses actual admission, preparation, credential and model
// services. Both rooms select the same configuration/selection, while their
// certified model tuples, AI seats and consent states differ.
func (h *Harness) AddSecondRoom(t *testing.T) string {
	t.Helper()
	configs := []launch.Configuration{}
	for _, c := range h.configs {
		v := c.StorageValue()
		if v.ID == "ai-required" {
			v.Seats = append(append([]launch.SeatRule{}, v.Seats...), launch.SeatRule{ID: "ai-other", Modes: []string{"ai"}, ModelCapabilities: []string{"structured-actions"}})
		}
		configs = append(configs, auth.RoomSecret(v))
	}
	h.configs = configs
	opts := h.modelOptions
	endpoint := opts.Endpoints[0].StorageValue()
	endpoint.Models = append(append([]string{}, endpoint.Models...), "fixture:other")
	opts.Endpoints = []model.Endpoint{model.NewEndpoint(endpoint)}
	cert := h.certificate.StorageValue()
	cert.ID, cert.Tuple.Model, cert.Level = "other-certified", "fixture:other", 2
	cert.Capabilities = []string{"structured-actions"}
	cert.Games = append([]model.GameEvidence{}, cert.Games...)
	for i := range cert.Games {
		cert.Games[i].Capabilities = []string{"structured-actions"}
	}
	opts.Certifications = append(append([]model.Certification{}, opts.Certifications...), model.NewCertification(cert))
	var e error
	h.Models, e = model.New(opts)
	need(t, e)
	h.modelOptions = opts
	h.models = h.Models
	h.recomposeLaunch(t, h.configs)
	h.composePlayer(t)
	h.Compose(t)
	// A trusted fixture changes the registered seat rules. The original player
	// configure route correctly refuses its stale prior preparation; use the
	// existing authorized launch service to establish the new current revision.
	_, e = h.service.Configure(h.ctx, h.caller(h.owner, "rules-"+token(t)), auth.RoomSecret(launch.ConfigureData{WorkspaceID: h.w, RoomID: h.room, ConfigurationID: "ai-required", Slots: []launch.Slot{{ID: "gm", Mode: "human", ParticipantID: h.hostPart}, {ID: "player", Mode: "human", ParticipantID: h.playerPart}, {ID: "ai", Mode: "ai", ModelSelection: "selected"}}}))
	need(t, e)
	_, e = h.Models.Configure(h.ctx, h.modelCaller(t), model.NewConfigureRequest(model.ConfigureRequestData{Scope: h.Scope(), SeatID: "ai", Selection: "selected", CredentialID: "byok", Budget: Limits(), ExpectedVersion: 1}))
	need(t, e)
	r, host := h.hostRoom(t)
	joined := h.join(t, h.participant, h.invite(t, r, false, 2))
	_, e = h.service.Configure(h.ctx, h.caller(h.owner, "second-"+token(t)), auth.RoomSecret(launch.ConfigureData{WorkspaceID: h.w, RoomID: r, ConfigurationID: "ai-required", Slots: []launch.Slot{{ID: "gm", Mode: "human", ParticipantID: host}, {ID: "player", Mode: "human", ParticipantID: joined["participant_id"].(string)}, {ID: "ai-other", Mode: "ai", ModelSelection: "selected"}}}))
	need(t, e)
	scope := core.Scope{WorkspaceID: h.w, RoomID: r, GameID: "game"}
	key, e := credential.NewKey([]byte("owned-B015-other-room-synthetic-private-key"))
	need(t, e)
	need(t, h.Models.StoreCredential(h.ctx, h.modelCaller(t), auth.RoomSecret(model.CredentialRequestData{Scope: scope, SeatID: "ai-other", ID: "byok", Lifetime: credential.Retained, Key: key})))
	key.Close()
	_, e = h.Models.Configure(h.ctx, h.modelCaller(t), model.NewConfigureRequest(model.ConfigureRequestData{Scope: scope, SeatID: "ai-other", Selection: "selected", CertificationID: "other-certified", CredentialID: "byok", Budget: Limits()}))
	need(t, e)
	return r
}

// SeedRoomCandidates inserts strict native records, not ready aliases. The
// original selected row alone matches the current slot; the extra physical
// candidates exercise the pre-visibility 64/65 result bound.
func (h *Harness) SeedRoomCandidates(t *testing.T, roomID, seat string, extras int, identicalHash *model.ConfigurationData) {
	t.Helper()
	scope := core.Scope{WorkspaceID: h.w, RoomID: roomID, GameID: "game"}
	row, e := h.Models.Read(h.ctx, h.modelCaller(t), auth.RoomSecret(model.TargetData{Scope: scope, SeatID: seat, ID: "selected"}))
	need(t, e)
	storage, e := postgres.NewPlatformModelStorage(h.r)
	need(t, e)
	need(t, h.authority.Inspect(h.ctx, h.owner.StorageValue().Cookie, h.owner.StorageValue().CSRF, true, func(ctx context.Context, tx auth.Transaction, _ auth.SessionData) error {
		mt, e := storage.Bind(tx.Core())
		if e != nil {
			return e
		}
		for i := 0; i < extras; i++ {
			v := row.StorageValue()
			v.Selection, v.Version = fmt.Sprintf("candidate-%03d", i), 1
			if identicalHash != nil {
				v.ConfigurationHash, v.GraphHash = identicalHash.ConfigurationHash, identicalHash.GraphHash
			}
			if e = mt.PutConfiguration(h.ctx, auth.RoomSecret(v), 0); e != nil {
				return e
			}
		}
		return nil
	}))
}

func (h *Harness) RoomModel(t *testing.T) model.ConfigurationData {
	t.Helper()
	row, e := h.Models.Read(h.ctx, h.modelCaller(t), auth.RoomSecret(model.TargetData{Scope: h.Scope(), SeatID: "ai", ID: "selected"}))
	need(t, e)
	return row.StorageValue()
}

func (h *Harness) ChangeRoomModelRecord(t *testing.T, fault string) {
	t.Helper()
	v := h.RoomModel(t)
	expected := v.Version
	v.Version++
	switch fault {
	case "stale_revision":
		v.PreparationRevision++
	case "configuration_hash":
		v.ConfigurationHash = "sha256:" + strings.Repeat("f", 64)
	case "graph_hash":
		v.GraphHash = "sha256:" + strings.Repeat("f", 64)
	case "incompatible":
		v.Tuple.Model = "unqualified:model"
	case "uncertified":
		v.Primary.ID = "unregistered-certificate"
	case "overbudget":
		v.Budget.Calls = Limits().Calls + 1
	default:
		t.Fatal("unknown bounded model-record fault")
	}
	storage, e := postgres.NewPlatformModelStorage(h.r)
	need(t, e)
	need(t, h.authority.Inspect(h.ctx, h.owner.StorageValue().Cookie, h.owner.StorageValue().CSRF, true, func(ctx context.Context, tx auth.Transaction, _ auth.SessionData) error {
		mt, e := storage.Bind(tx.Core())
		if e != nil {
			return e
		}
		return mt.PutConfiguration(h.ctx, auth.RoomSecret(v), expected)
	}))
}

type RoomReadProbe struct {
	Calls   atomic.Uint64
	Unknown atomic.Bool
	Delay   atomic.Bool
}
type roomReadRepository struct {
	auth.Repository
	probe *RoomReadProbe
}

func (r roomReadRepository) Transact(ctx context.Context, f func(auth.Transaction) error) error {
	r.probe.Calls.Add(1)
	e := r.Repository.Transact(ctx, func(tx auth.Transaction) error {
		if e := f(tx); e != nil {
			return e
		}
		if r.probe.Delay.Load() {
			<-ctx.Done()
			return ctx.Err()
		}
		return nil
	})
	if e == nil && r.probe.Unknown.Load() {
		return auth.ErrOutcomeUnknown
	}
	return e
}

// ProbeRoomRead retains the real PostgreSQL transaction and all admission,
// installation and model checks. It observes transaction count or injects a
// terminal timeout/unknown outcome, without substituting trusted data.
func (h *Harness) ProbeRoomRead(t *testing.T) *RoomReadProbe {
	t.Helper()
	probe := &RoomReadProbe{}
	v, e := room.NewAdmissionVerifier(h.store, invitationKey)
	need(t, e)
	h.authority, e = auth.NewRoomAuthority(roomReadRepository{h.r, probe}, cookieKey, replayKey, invitationKey, authSchema, v, v)
	need(t, e)
	h.rooms, e = room.NewService(h.authority, h.store, invitationKey, roomSchema)
	need(t, e)
	opts := h.modelOptions
	opts.Authority = h.authority
	h.Models, e = model.New(opts)
	need(t, e)
	h.modelOptions, h.models = opts, h.Models
	h.recomposeLaunch(t, h.configs)
	h.composePlayer(t)
	h.Compose(t)
	probe.Calls.Store(0)
	return probe
}

func (h *Harness) RoomCanceled(t *testing.T) error {
	t.Helper()
	source, e := model.NewPresentationSource(h.Models, nil)
	need(t, e)
	schema, e := ReadProductFile("schemas/platform/platform-player-presentation-api-v1.schema.json")
	need(t, e)
	facade, e := player.NewPresentationService(h.players, source, schema)
	need(t, e)
	ctx, cancel := context.WithCancel(h.ctx)
	cancel()
	_, e = facade.ReadRoom(ctx, h.caller(h.owner, ""), h.w, h.room)
	return e
}

func (h *Harness) RoomRuntimeProof(t *testing.T) {
	t.Helper()
	h.mu.Lock()
	count := 0
	for _, e := range h.executions {
		if e.RunnerHash == runnerHash && e.PID > 0 && strings.HasPrefix(e.Case, "host:") {
			count++
		}
	}
	h.mu.Unlock()
	if count == 0 || runnerHash == "" {
		t.Fatal("actual production Lua quarantine evidence missing")
	}
	t.Logf("owned PostgreSQL=%s production Lua=%s host cases=%d same-origin HTTPS=true", os.Getenv("M2B014_CONTAINER_ID"), runnerHash, count)
}

func NewRoomPackageGraph(t *testing.T, count int) *Harness {
	t.Helper()
	deps := []*archive.Package{}
	for i := 1; i < count; i++ {
		p, e := packages.Build(packages.Files(fmt.Sprintf("fixture.example/content-%02d", i), "content", ""))
		need(t, e)
		deps = append(deps, p)
	}
	build := func(runtime install.RuntimeConfig, source string) (*archive.Package, install.PolicyConfig, error) {
		root, pc, e := aiBuild(runtime, source)
		if e != nil {
			return nil, pc, e
		}
		files := map[string][]byte{}
		for _, entry := range root.Entries() {
			files[entry.Path()] = entry.Bytes()
		}
		nodes := root.ExactLock().Packages()[1:]
		for _, dep := range deps {
			m, e := dep.Manifest()
			if e != nil {
				return nil, pc, e
			}
			files[archive.ManifestPath] = append(files[archive.ManifestPath], []byte(fmt.Sprintf("\n[[dependencies]]\npackage_id=%q\nversion=\"1.0.0\"\n", m.Package.PackageID))...)
			nodes = append(nodes, dep.ExactLock().Packages()[0])
			pc.Artifacts[string(dep.ArtifactIdentity().Digest())] = install.Approval{RightsDigest: install.RightsDigest(*m.Package), Retention: "synthetic", Safety: "ACTIVE"}
		}
		full, e := packages.Build(files, nodes...)
		if e != nil {
			return nil, pc, e
		}
		approval := pc.Artifacts[string(root.ArtifactIdentity().Digest())]
		delete(pc.Artifacts, string(root.ArtifactIdentity().Digest()))
		pc.Artifacts[string(full.ArtifactIdentity().Digest())] = approval
		return full, pc, nil
	}
	return newRoomGraphHarness(t, build, deps...)
}

// ACC-M2-B015-001: build the native preparation once through its authenticated
// service. The HTTP presentation operation keeps its original five-second bound.
func newRoomGraphHarness(t *testing.T, build func(install.RuntimeConfig, string) (*archive.Package, install.PolicyConfig, error), dependencies ...*archive.Package) *Harness {
	t.Helper()
	l := newLaunchFixtureWithBuilder(t, nil, build, dependencies...)
	n := playerFixtureWithLaunch(t, l, false)
	h := &Harness{playerFixture: n}
	started := time.Now()
	prep, e := l.service.Configure(l.ctx, l.caller(l.owner, "room-graph-"+token(t)), auth.RoomSecret(launch.ConfigureData{WorkspaceID: l.w, RoomID: n.room, ConfigurationID: "ai-required", Slots: []launch.Slot{{ID: "gm", Mode: "human", ParticipantID: n.hostPart}, {ID: "player", Mode: "human", ParticipantID: n.playerPart}, {ID: "ai", Mode: "ai", ModelSelection: "selected"}}}))
	phaseNeed(t, "room-package-bound-native-configure", e)
	if prep.StorageValue().Scope != h.Scope() || prep.StorageValue().ConfigurationID != "ai-required" {
		t.Fatal("package-bound native preparation scope lost")
	}
	t.Logf("package-bound fixture native Configure authenticated=true elapsed=%s", time.Since(started))
	storage, e := postgres.NewPlatformModelStorage(l.r)
	need(t, e)
	need(t, storage.Bootstrap(l.ctx))
	budgets, e := postgres.NewPlatformBudgetStorage(l.r)
	need(t, e)
	need(t, budgets.Bootstrap(l.ctx))
	raw := make([]byte, 32)
	_, e = rand.Read(raw)
	need(t, e)
	master := filepath.Join(t.TempDir(), "master")
	need(t, os.WriteFile(master, raw, 0400))
	clear(raw)
	vault, e := credential.New(master)
	need(t, e)
	t.Cleanup(vault.Close)
	h.modelOptions = model.Options{Authority: l.authority, Rooms: l.store, Launches: l.storage, Storage: storage, Vault: vault, WorkspaceLimits: map[string]model.Limits{l.w: Limits()}}
	h.Models, e = model.New(h.modelOptions)
	need(t, e)
	l.models = h.Models
	l.recomposeLaunch(t, l.configs)
	n.composePlayer(t)
	h.Compose(t)
	return h
}
