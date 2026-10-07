//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package launch_test

import (
	"bytes"
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	archivefixture "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	sessionfixture "github.com/zyc14588/TRPG_PLATFORM/internal/session/testdata"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
	"sync"
	"testing"
)

type launchFixture struct {
	*fixture
	storage    *postgres.PlatformLaunchStorage
	service    *launch.Service
	configs    []launch.Configuration
	mu         sync.Mutex
	executions []install.Execution
	vmFault    func(install.Execution) error
}

func newLaunchFixture(t *testing.T, fault func(context.Context, string) error) *launchFixture {
	t.Helper()
	f := newFixture(t, fault)
	l := &launchFixture{fixture: f}
	ls, e := postgres.NewPlatformLaunchStorage(f.r)
	need(t, e)
	need(t, ls.Bootstrap(f.ctx))
	l.storage = ls
	runtime := install.RuntimeConfig{Runner: runner, SHA256: runnerHash, Limits: profile.DefaultLimits()}
	pkg, pc, e := sessionfixture.Build(runtime, "")
	need(t, e)
	policy, e := install.NewPolicy(pc)
	need(t, e)
	objects, e := object.Open(t.TempDir())
	need(t, e)
	t.Cleanup(func() { need(t, objects.Close()) })
	repo, e := postgres.OpenInstallationRepository(f.ctx, dsn, objects, extension.DefaultSupport, nil)
	need(t, e)
	t.Cleanup(func() { need(t, repo.Close()) })
	need(t, repo.Bootstrap(f.ctx))
	need(t, repo.ProvisionWorkspace(f.ctx, f.w))
	credential := store.Credential("m2-b004-owned-synthetic-install-credential")
	access, e := store.NewAccess(map[store.Credential][]store.Membership{credential: {{Principal: "operator", Workspace: f.w, Install: true, Read: true}}})
	need(t, e)
	observe := func(v install.Execution) error {
		l.mu.Lock()
		defer l.mu.Unlock()
		l.executions = append(l.executions, v)
		if l.vmFault != nil {
			return l.vmFault(v)
		}
		return nil
	}
	i, e := install.New(install.Options{StagingRoot: t.TempDir(), Policy: policy, Access: access, Objects: objects, Repository: repo, Runtime: runtime, Support: extension.DefaultSupport, Observe: func(string) error { return nil }, Execution: observe})
	need(t, e)
	raw, e := archivefixture.Archive(pkg)
	need(t, e)
	_, e = i.Install(f.ctx, install.Request{Credential: credential, Workspace: f.w, ID: "initial-install", Root: install.Input{Archive: bytes.NewReader(raw)}})
	clear(raw)
	need(t, e)
	reader, e := store.NewReader(repo, objects, access, extension.DefaultSupport)
	need(t, e)
	factory, e := install.NewSessionFactory(install.SessionOptions{Reader: reader, Policy: policy, Repository: ls.SessionRepository(), Runtime: runtime, Execution: observe, Audit: func(data.Audit) error { return nil }, Validate: func(context.Context, data.Commit) error { return nil }})
	need(t, e)
	base := launch.ConfigurationData{ID: "minimal", WorkspaceID: f.w, Factory: factory, Request: install.SessionRequest{Credential: credential, Workspace: f.w, Root: string(pkg.ArtifactIdentity().Digest())}, Seats: []launch.SeatRule{{ID: "gm", Required: true, Modes: []string{"human"}}}, SafetyTags: []string{"violence", "gore"}}
	violent := base
	violent.ID = "labeled"
	violent.ContentTags = []string{"violence"}
	ai := base
	ai.ID = "ai-required"
	ai.Seats = append([]launch.SeatRule{}, base.Seats...)
	ai.Seats = append(ai.Seats, launch.SeatRule{ID: "ai", Required: true, Modes: []string{"ai"}, ModelCapabilities: []string{"structured-actions"}})
	l.configs = []launch.Configuration{auth.RoomSecret(base), auth.RoomSecret(violent), auth.RoomSecret(ai)}
	l.recomposeLaunch(t, l.configs)
	return l
}
func (l *launchFixture) recomposeLaunch(t *testing.T, configs []launch.Configuration) {
	t.Helper()
	if l.service != nil {
		need(t, l.service.Close())
	}
	s, e := launch.New(launch.Options{Context: l.ctx, Authority: l.authority, Rooms: l.store, Storage: l.storage, Configurations: configs, MaxSessions: 8})
	need(t, e)
	l.service = s
	t.Cleanup(func() { need(t, s.Close()) })
}
func (l *launchFixture) caller(a actor, key string) launch.Caller {
	v := a.StorageValue()
	return auth.RoomSecret(launch.CallerData{Credential: v.Cookie, CSRF: v.CSRF, IdempotencyKey: key, Network: v.Network})
}
func (l *launchFixture) hostRoom(t *testing.T) (string, string) {
	t.Helper()
	r := l.create(t)
	inv := l.invite(t, r, false, 8)
	admission := l.join(t, l.owner, inv)
	p := admission["participant_id"].(string)
	l.call(t, l.owner, "set_role", l.w, r, p, map[string]any{"role": "host", "enabled": true})
	return r, p
}
func (l *launchFixture) configure(t *testing.T, r, p, id string) launch.Preparation {
	t.Helper()
	slots := []launch.Slot{{ID: "gm", Mode: "human", ParticipantID: p}}
	if id == "ai-required" {
		slots = append(slots, launch.Slot{ID: "ai", Mode: "ai", ModelSelection: "owned-model-selection"})
	}
	out, e := l.service.Configure(l.ctx, l.caller(l.owner, "configure-"+token(t)), auth.RoomSecret(launch.ConfigureData{WorkspaceID: l.w, RoomID: r, ConfigurationID: id, Slots: slots}))
	need(t, e)
	return out
}
func (l *launchFixture) acknowledge(t *testing.T, a actor, r string, revision uint64, consent, ready, safety bool, boundaries []string, key string) error {
	t.Helper()
	return l.service.Acknowledge(l.ctx, l.caller(a, key), auth.RoomSecret(launch.AcknowledgeData{WorkspaceID: l.w, RoomID: r, Revision: revision, Consent: consent, Ready: ready, SafetyConfirmed: safety, Boundaries: boundaries}))
}
func (l *launchFixture) start(t *testing.T, a actor, r string, revision uint64, key string) (launch.Session, error) {
	t.Helper()
	return l.service.Launch(l.ctx, l.caller(a, key), auth.RoomSecret(launch.LaunchData{WorkspaceID: l.w, RoomID: r, Revision: revision}))
}
func (l *launchFixture) ready(t *testing.T, r string, p launch.Preparation) {
	t.Helper()
	need(t, l.acknowledge(t, l.owner, r, p.StorageValue().Revision, true, true, true, nil, "ready-"+token(t)))
}
func (l *launchFixture) inspectAbsent(t *testing.T, r string) {
	t.Helper()
	if l.sql(t, `SELECT (SELECT count(*) FROM platform_launch.sessions WHERE workspace_id='`+l.w+`')+(SELECT count(*) FROM host_command.sessions WHERE workspace='`+l.w+`')+(SELECT count(*) FROM host_command.creation WHERE workspace='`+l.w+`')+(SELECT count(*) FROM package_install.data_targets WHERE workspace='`+l.w+`')`) != "0" {
		t.Fatal("failed launch left partial authoritative records")
	}
	if l.sql(t, `SELECT state FROM platform_room.rooms WHERE workspace_id='`+l.w+`' AND room_id='`+r+`'`) != "lobby" {
		t.Fatal("failed launch changed lobby state")
	}
}
