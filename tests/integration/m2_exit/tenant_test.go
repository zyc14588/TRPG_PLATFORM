//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package m2_exit_test

import (
	"bytes"
	"errors"
	"net/http"
	"os"
	"reflect"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	fixtures "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

// These carriers reuse the current Chrome native-composition fixture helpers,
// with a distinct B012 owned database. Qualification still executes Chrome in
// its separate original B010 suite.
func TestForeignWorkspaceSameHashInstallDeniesWithoutChangingGrants(t *testing.T) {
	f := newFixture(t, nil)
	root, staging := t.TempDir(), t.TempDir()
	need(t, os.Chmod(root, 0700))
	need(t, os.Chmod(staging, 0700))
	objects, e := object.Open(root)
	need(t, e)
	t.Cleanup(func() { need(t, objects.Close()) })
	repo, e := postgres.OpenInstallationRepository(f.ctx, dsn, objects, extension.DefaultSupport, nil)
	t.Cleanup(func() { need(t, repo.Close()) })
	need(t, e)
	need(t, repo.Bootstrap(f.ctx))
	a, b := "install-a-"+prefix, "install-b-"+prefix
	need(t, repo.ProvisionWorkspace(f.ctx, a))
	need(t, repo.ProvisionWorkspace(f.ctx, b))
	ca, cb := store.Credential("B012-owned-synthetic-install-a"), store.Credential("B012-owned-synthetic-install-b")
	access, e := store.NewAccess(map[store.Credential][]store.Membership{ca: {{Principal: "alice", Workspace: a, Install: true, Read: true}}, cb: {{Principal: "bob", Workspace: b, Install: true, Read: true}}})
	need(t, e)
	pkg, e := fixtures.Build(fixtures.Files("test.publisher/fixture", "assets", ""))
	need(t, e)
	doc, e := pkg.Manifest()
	need(t, e)
	policy, e := install.NewPolicy(install.PolicyConfig{Context: "ci", HostMajor: 1, Artifacts: map[string]install.Approval{string(pkg.ArtifactIdentity().Digest()): {RightsDigest: install.RightsDigest(*doc.Package), Safety: "ACTIVE", Retention: "test-only"}}})
	need(t, e)
	installer, e := install.New(install.Options{StagingRoot: staging, Policy: policy, Access: access, Objects: objects, Repository: repo, Support: extension.DefaultSupport, Observe: func(string) error { return nil }, Execution: func(install.Execution) error { return nil }})
	need(t, e)
	raw, e := fixtures.Archive(pkg)
	need(t, e)
	defer clear(raw)
	request := func(c store.Credential, w, id string) install.Request {
		return install.Request{Credential: c, Workspace: w, ID: id, Root: install.Input{Archive: bytes.NewReader(raw)}}
	}
	for _, q := range []install.Request{request(ca, a, "positive-a"), request(cb, b, "positive-b")} {
		r, e := installer.Install(f.ctx, q)
		need(t, e)
		if r.Root != string(pkg.ArtifactIdentity().Digest()) {
			t.Fatal("positive same-hash installation absent")
		}
	}
	baseline := func() string {
		return f.sql(t, `SELECT md5(coalesce(string_agg(row_to_json(x)::text,'' ORDER BY row_to_json(x)::text),'')) FROM (SELECT 'artifacts' AS kind,to_jsonb(a) AS row FROM package_install.artifacts a UNION ALL SELECT 'grants',to_jsonb(g) FROM package_install.grants g UNION ALL SELECT 'requests',to_jsonb(r) FROM package_install.requests r) x`)
	}
	before := baseline()
	physical, e := os.ReadDir(root)
	need(t, e)
	denied, e := installer.Install(f.ctx, request(cb, a, "foreign-same-hash"))
	if !errors.Is(e, store.ErrDenied) || !reflect.ValueOf(denied).IsZero() {
		t.Fatal("foreign credential returned an installation or failed to deny")
	}
	after, e := os.ReadDir(root)
	need(t, e)
	if baseline() != before || len(after) != len(physical) {
		t.Fatal("denied same-hash installation changed authoritative grants/installations or physical objects")
	}
	reader, e := store.NewReader(repo, objects, access, extension.DefaultSupport)
	need(t, e)
	if out, e := reader.Load(f.ctx, cb, a, string(pkg.ArtifactIdentity().Digest())); !errors.Is(e, store.ErrDenied) || out != nil {
		t.Fatal("foreign install credential obtained content-addressed data")
	}
	t.Log("actual same-hash installer denial returned no data; authoritative artifacts/grants/requests and physical object count unchanged")
}

func TestForeignWorkspaceAuthenticatedExportReturnsNoDataAndKeepsSession(t *testing.T) {
	a, b := newPlayerFixture(t, false), newPlayerFixture(t, false)
	a.configureAndLaunch(t)
	b.configureAndLaunch(t)
	ga := a.configs[0].StorageValue().Request.Root
	gb := b.configs[0].StorageValue().Request.Root
	if ga == "" || ga != gb {
		t.Fatal("export tenant fixtures did not share content hash")
	}
	baseline := func() string {
		return a.sql(t, `SELECT md5(coalesce(string_agg(row_to_json(x)::text,'' ORDER BY row_to_json(x)::text),'')) FROM (SELECT 'session' AS kind,to_jsonb(s) AS row FROM host_command.sessions s UNION ALL SELECT 'launch',to_jsonb(l) FROM platform_launch.sessions l UNION ALL SELECT 'request',to_jsonb(r) FROM host_command.requests r UNION ALL SELECT 'event',to_jsonb(e) FROM host_command.events e UNION ALL SELECT 'grant',to_jsonb(g) FROM package_install.grants g UNION ALL SELECT 'control',to_jsonb(c) FROM platform_player.control c) x`)
	}
	before := baseline()
	fields := map[string]any{"kind": "personal", "after_cursor": "0", "limit": 8}
	// This request carries B's live authenticated account cookie and CSRF, but
	// targets A's real launched session through the current trusted HTTPS route.
	response, status := a.wire(t, b.owner, "export", a.w, a.room, "foreign-export-"+token(t), fields)
	if status != http.StatusForbidden || response["data"] != nil {
		t.Fatal("authenticated foreign-workspace export returned data or did not deny")
	}
	er, ok := response["error"].(map[string]any)
	if !ok || er["code"] != "DENIED" {
		t.Fatal("foreign export did not use the existing authorization denial")
	}
	private(t, response, false)
	if baseline() != before {
		t.Fatal("foreign export mutated authoritative session/grant/control records")
	}
	own := a.api(t, a.participant, "export", map[string]any{"kind": "personal", "after_cursor": "0", "limit": 8})
	if own["session_id"] != a.sessionID {
		t.Fatal("positive current-workspace export absent")
	}
	private(t, own, false)
	t.Log("actual authenticated foreign HTTPS session export denied with no data; durable session/event/request/grant/control records unchanged")
}
