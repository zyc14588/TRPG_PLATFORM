//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package model_gateway_test

import (
	"bytes"
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	archivefixture "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
	"os"
	"sync"
	"testing"
)

type loopLaunchFixture struct {
	*loopFixture
	storage                   *postgres.PlatformLaunchStorage
	service                   *launch.Service
	configs                   []launch.Configuration
	models                    launch.ModelChecker
	mu                        sync.Mutex
	executions                []install.Execution
	vmFault                   func(install.Execution) error
	inputSchema, resultSchema hostapi.Schema
}

func loopNewLaunchFixture(t *testing.T, fault func(context.Context, string) error) *loopLaunchFixture {
	t.Helper()
	f := loopNewFixture(t, fault)
	l := &loopLaunchFixture{loopFixture: f}
	ls, e := postgres.NewPlatformLaunchStorage(f.r)
	loopNeed(t, e)
	loopNeed(t, ls.Bootstrap(f.ctx))
	l.storage = ls
	runtime := install.RuntimeConfig{Runner: loopRunner, SHA256: loopRunnerHash, Limits: profile.DefaultLimits()}
	pkg, pc, e := loopBuild(runtime, "")
	loopNeed(t, e)
	for _, pair := range []struct {
		path string
		seed checkpoint.Value
		out  *hostapi.Schema
	}{{"schemas/intent.schema.json", checkpoint.Object(map[string]checkpoint.Value{"value": checkpoint.Int(1)}), &l.inputSchema}, {"schemas/result.schema.json", checkpoint.Int(1), &l.resultSchema}} {
		entry, ok := pkg.Entry(pair.path)
		if !ok {
			t.Fatal("approved task Schema absent")
		}
		*pair.out, e = hostapi.BindSchema(pkg, pair.path, checkpoint.Hash(entry.Bytes()), pair.seed)
		loopNeed(t, e)
	}
	policy, e := install.NewPolicy(pc)
	loopNeed(t, e)
	objectRoot, stagingRoot := t.TempDir(), t.TempDir()
	loopNeed(t, os.Chmod(objectRoot, 0700))
	loopNeed(t, os.Chmod(stagingRoot, 0700))
	objects, e := object.Open(objectRoot)
	loopNeed(t, e)
	t.Cleanup(func() { loopNeed(t, objects.Close()) })
	repo, e := postgres.OpenInstallationRepository(f.ctx, loopDsn, objects, extension.DefaultSupport, nil)
	loopNeed(t, e)
	t.Cleanup(func() { loopNeed(t, repo.Close()) })
	loopNeed(t, repo.Bootstrap(f.ctx))
	loopNeed(t, repo.ProvisionWorkspace(f.ctx, f.w))
	credential := store.Credential("m2-b006-owned-synthetic-install-credential")
	access, e := store.NewAccess(map[store.Credential][]store.Membership{credential: {{Principal: "operator", Workspace: f.w, Install: true, Read: true}}})
	loopNeed(t, e)
	observe := func(v install.Execution) error {
		l.mu.Lock()
		defer l.mu.Unlock()
		l.executions = append(l.executions, v)
		if l.vmFault != nil {
			return l.vmFault(v)
		}
		return nil
	}
	i, e := install.New(install.Options{StagingRoot: stagingRoot, Policy: policy, Access: access, Objects: objects, Repository: repo, Runtime: runtime, Support: extension.DefaultSupport, Observe: func(string) error { return nil }, Execution: observe})
	loopNeed(t, e)
	raw, e := archivefixture.Archive(pkg)
	loopNeed(t, e)
	_, e = i.Install(f.ctx, install.Request{Credential: credential, Workspace: f.w, ID: "initial-install", Root: install.Input{Archive: bytes.NewReader(raw)}})
	clear(raw)
	loopNeed(t, e)
	reader, e := store.NewReader(repo, objects, access, extension.DefaultSupport)
	loopNeed(t, e)
	factory, e := install.NewSessionFactory(install.SessionOptions{Reader: reader, Policy: policy, Repository: ls.SessionRepository(), Runtime: runtime, Execution: observe, Audit: func(data.Audit) error { return nil }, Validate: func(context.Context, data.Commit) error { return nil }})
	loopNeed(t, e)
	base := launch.ConfigurationData{ID: "minimal", WorkspaceID: f.w, Factory: factory, Request: install.SessionRequest{Credential: credential, Workspace: f.w, Root: string(pkg.ArtifactIdentity().Digest())}, Seats: []launch.SeatRule{{ID: "gm", Required: true, Modes: []string{"human"}}, {ID: "player", Required: false, Modes: []string{"human"}}}, SafetyTags: []string{"violence", "gore"}}
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
func (l *loopLaunchFixture) recomposeLaunch(t *testing.T, configs []launch.Configuration) {
	t.Helper()
	if l.service != nil {
		loopNeed(t, l.service.Close())
	}
	s, e := launch.New(launch.Options{Context: l.ctx, Authority: l.authority, Rooms: l.store, Storage: l.storage, Configurations: configs, MaxSessions: 8, Models: l.models})
	loopNeed(t, e)
	l.service = s
	t.Cleanup(func() { loopNeed(t, s.Close()) })
}
func (l *loopLaunchFixture) caller(a loopActor, key string) launch.Caller {
	v := a.StorageValue()
	return auth.RoomSecret(launch.CallerData{Credential: v.Cookie, CSRF: v.CSRF, IdempotencyKey: "b006-request-" + key, Network: v.Network})
}
func (l *loopLaunchFixture) hostRoom(t *testing.T) (string, string) {
	t.Helper()
	r := l.create(t)
	inv := l.invite(t, r, false, 8)
	admission := l.join(t, l.owner, inv)
	p := admission["participant_id"].(string)
	l.call(t, l.owner, "set_role", l.w, r, p, map[string]any{"role": "host", "enabled": true})
	return r, p
}
func (l *loopLaunchFixture) configure(t *testing.T, r, p, id string) launch.Preparation {
	t.Helper()
	slots := []launch.Slot{{ID: "gm", Mode: "human", ParticipantID: p}}
	if id == "ai-required" {
		slots = append(slots, launch.Slot{ID: "ai", Mode: "ai", ModelSelection: "selected"})
	}
	out, e := l.service.Configure(l.ctx, l.caller(l.owner, "configure-"+loopToken(t)), auth.RoomSecret(launch.ConfigureData{WorkspaceID: l.w, RoomID: r, ConfigurationID: id, Slots: slots}))
	loopNeed(t, e)
	return out
}
func (l *loopLaunchFixture) acknowledge(t *testing.T, a loopActor, r string, revision uint64, consent, ready, safety bool, boundaries []string, key string) error {
	t.Helper()
	return l.service.Acknowledge(l.ctx, l.caller(a, key), auth.RoomSecret(launch.AcknowledgeData{WorkspaceID: l.w, RoomID: r, Revision: revision, Consent: consent, Ready: ready, SafetyConfirmed: safety, Boundaries: boundaries}))
}
func (l *loopLaunchFixture) start(t *testing.T, a loopActor, r string, revision uint64, key string) (launch.Session, error) {
	t.Helper()
	return l.service.Launch(l.ctx, l.caller(a, key), auth.RoomSecret(launch.LaunchData{WorkspaceID: l.w, RoomID: r, Revision: revision}))
}
func (l *loopLaunchFixture) ready(t *testing.T, r string, p launch.Preparation) {
	t.Helper()
	loopNeed(t, l.acknowledge(t, l.owner, r, p.StorageValue().Revision, true, true, true, nil, "ready-"+loopToken(t)))
}
func (l *loopLaunchFixture) inspectAbsent(t *testing.T, r string) {
	t.Helper()
	if l.sql(t, `SELECT (SELECT count(*) FROM platform_launch.sessions WHERE workspace_id='`+l.w+`')+(SELECT count(*) FROM host_command.sessions WHERE workspace='`+l.w+`')+(SELECT count(*) FROM host_command.creation WHERE workspace='`+l.w+`')+(SELECT count(*) FROM package_install.data_targets WHERE workspace='`+l.w+`')`) != "0" {
		t.Fatal("failed launch left partial authoritative records")
	}
	if l.sql(t, `SELECT state FROM platform_room.rooms WHERE workspace_id='`+l.w+`' AND room_id='`+r+`'`) != "lobby" {
		t.Fatal("failed launch changed lobby state")
	}
}
