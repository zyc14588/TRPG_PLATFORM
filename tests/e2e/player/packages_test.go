//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player_test

import (
	"bytes"
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	archivefixture "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
	"os"
	"strings"
	"sync"
	"testing"
)

type launchFixture struct {
	*fixture
	storage       *postgres.PlatformLaunchStorage
	playerStorage *postgres.PlatformPlayerStorage
	control       *player.Control
	guarded       *postgres.ControlledLaunchStorage
	service       *launch.Service
	configs       []launch.Configuration
	models        launch.ModelChecker
	mu            sync.Mutex
	executions    []install.Execution
	vmFault       func(install.Execution) error
}

func newLaunchFixture(t *testing.T, fault func(context.Context, string) error) *launchFixture {
	t.Helper()
	return newLaunchFixtureWithBuilder(t, fault, Build)
}
func newLaunchFixtureWithBuilder(t *testing.T, fault func(context.Context, string) error, build func(install.RuntimeConfig, string) (*archive.Package, install.PolicyConfig, error)) *launchFixture {
	t.Helper()
	f := newFixture(t, fault)
	l := &launchFixture{fixture: f}
	ls, e := postgres.NewPlatformLaunchStorage(f.r)
	need(t, e)
	need(t, ls.Bootstrap(f.ctx))
	l.storage = ls
	l.playerStorage, e = postgres.NewPlatformPlayerStorage(f.r)
	need(t, e)
	need(t, l.playerStorage.Bootstrap(f.ctx))
	l.control, e = player.NewControl(l.playerStorage)
	need(t, e)
	l.guarded, e = postgres.NewControlledLaunchStorage(ls, l.playerStorage)
	need(t, e)
	runtime := install.RuntimeConfig{Runner: runner, SHA256: runnerHash, Limits: profile.DefaultLimits()}
	pkg, pc, e := build(runtime, "")
	phaseNeed(t, "native-package-build", e)
	policy, e := install.NewPolicy(pc)
	phaseNeed(t, "native-install-policy", e)
	objectRoot, stagingRoot := t.TempDir(), t.TempDir()
	need(t, os.Chmod(objectRoot, 0700))
	need(t, os.Chmod(stagingRoot, 0700))
	objects, e := object.Open(objectRoot)
	need(t, e)
	t.Cleanup(func() { need(t, objects.Close()) })
	repo, e := postgres.OpenInstallationRepository(f.ctx, dsn, objects, extension.DefaultSupport, nil)
	need(t, e)
	t.Cleanup(func() { need(t, repo.Close()) })
	need(t, repo.Bootstrap(f.ctx))
	need(t, repo.ProvisionWorkspace(f.ctx, f.w))
	credential := store.Credential("m2-b010-owned-synthetic-install-credential")
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
	i, e := install.New(install.Options{StagingRoot: stagingRoot, Policy: policy, Access: access, Objects: objects, Repository: repo, Runtime: runtime, Support: extension.DefaultSupport, Observe: func(string) error { return nil }, Execution: observe})
	need(t, e)
	raw, e := archivefixture.Archive(pkg)
	need(t, e)
	_, e = i.Install(f.ctx, install.Request{Credential: credential, Workspace: f.w, ID: "initial-install", Root: install.Input{Archive: bytes.NewReader(raw)}})
	clear(raw)
	if e != nil {
		for _, execution := range l.executions {
			t.Logf("native-install-profile case=%s outcome=%s profile=%s runtime=%s runner_hash=%s reaped=%t", execution.Case, execution.Outcome, execution.Profile, execution.Runtime, execution.RunnerHash, execution.Reaped)
		}
		t.Logf("native-install-failure type=%T profile_code=%s", e, profile.Code(e))
	}
	phaseNeed(t, "native-package-install", e)
	reader, e := store.NewReader(repo, objects, access, extension.DefaultSupport)
	need(t, e)
	factory, e := install.NewSessionFactory(install.SessionOptions{Reader: reader, Policy: policy, Repository: ls.SessionRepository(), Runtime: runtime, Execution: observe, Audit: func(data.Audit) error { return nil }, Validate: func(context.Context, data.Commit) error { return nil }})
	phaseNeed(t, "native-session-factory", e)
	base := launch.ConfigurationData{ID: "minimal", WorkspaceID: f.w, Factory: factory, Request: install.SessionRequest{Credential: credential, Workspace: f.w, Root: string(pkg.ArtifactIdentity().Digest())}, Seats: []launch.SeatRule{{ID: "gm", Required: true, Modes: []string{"human"}}, {ID: "player", Required: true, Modes: []string{"human"}}}, SafetyTags: []string{"violence", "gore"}}
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
	s, e := launch.New(launch.Options{Context: l.ctx, Authority: l.authority, Rooms: l.store, Storage: l.guarded, Configurations: configs, MaxSessions: 8, PlayerControl: l.control, Models: l.models})
	phaseNeed(t, "native-launch-composition", e)
	l.service = s
	t.Cleanup(func() { need(t, s.Close()) })
}
func (l *launchFixture) caller(a actor, key string) launch.Caller {
	v := a.StorageValue()
	return auth.RoomSecret(launch.CallerData{Credential: v.Cookie, CSRF: v.CSRF, IdempotencyKey: "b013-request-" + key, Network: v.Network})
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

func phaseNeed(t *testing.T, phase string, e error) {
	t.Helper()
	if e != nil {
		t.Fatalf("owned fixture phase %s failed: %v; private detail withheld", phase, auth.SafeError(e))
	}
}

func genericTaskBuild(runtime install.RuntimeConfig, source string) (*archive.Package, install.PolicyConfig, error) {
	if source == "" {
		source = strings.Replace(IncrementSource, ";host.ai.request({value=next})", "", 1)
		source = strings.Replace(source, "\nreturn M\n", `
M.resume_continuation=function(command)
 if not command.payload then return {} end
 assert(command.seat_id=="task-system" and command.type=="resume-continuation")
 assert(type(command.payload.continuation)=="table" and math.type(command.payload.result)=="integer")
 host.state.put({"counter"},host.state.get({"counter"})+command.payload.result)
 host.event.emit("change",state())
 return {}
end
return M
`, 1)
	}
	return Build(runtime, source)
}
