// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// This helper deliberately terminates the installer process at a named fault
// boundary. It receives only a test-created database and private directories.
package main

import (
	"bytes"
	"context"
	"database/sql"
	_ "github.com/jackc/pgx/v5/stdlib"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
	"os"
)

func main() {
	if err := run(); err != nil {
		os.Exit(1)
	}
}
func run() error {
	ctx := context.Background()
	db, err := sql.Open("pgx", os.Getenv("B003_CRASH_DSN"))
	if err != nil {
		return err
	}
	defer db.Close()
	objects, err := object.Open(os.Getenv("B003_CRASH_OBJECTS"))
	if err != nil {
		return err
	}
	defer objects.Close()
	phase := os.Getenv("B003_CRASH_PHASE")
	crash := func(point string) error {
		if point == phase {
			os.Exit(23)
		}
		return nil
	}
	repo, err := postgres.New(postgres.Options{DB: db, Objects: objects, Support: extension.DefaultSupport, Fault: func(_ context.Context, p string, _ *sql.Tx) error { return crash(p) }})
	if err != nil {
		return err
	}
	credential := store.Credential("fixture-alice-private-credential")
	access, err := store.NewAccess(map[store.Credential][]store.Membership{credential: {{Principal: "alice", Workspace: "a", Install: true, Read: true}}})
	if err != nil {
		return err
	}
	p, err := fixtures.Build(fixtures.Files("test.publisher/fixture", "assets", ""))
	if err != nil {
		return err
	}
	doc, err := p.Manifest()
	if err != nil {
		return err
	}
	policy, err := install.NewPolicy(install.PolicyConfig{Context: "ci", HostMajor: 1, Artifacts: map[string]install.Approval{string(p.ArtifactIdentity().Digest()): {RightsDigest: install.RightsDigest(*doc.Package), Retention: "fixture", Safety: "ACTIVE"}}})
	if err != nil {
		return err
	}
	i, err := install.New(install.Options{StagingRoot: os.Getenv("B003_CRASH_STAGING"), Objects: objects, Repository: repo, Access: access, Policy: policy, Support: extension.DefaultSupport, Observe: crash, Execution: func(install.Execution) error { return nil }})
	if err != nil {
		return err
	}
	raw, err := fixtures.Archive(p)
	if err != nil {
		return err
	}
	_, err = i.Install(ctx, install.Request{Credential: credential, Workspace: "a", ID: "crash", Root: install.Input{Archive: bytes.NewReader(raw)}})
	return err
}
