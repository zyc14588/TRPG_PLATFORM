// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"bytes"
	"context"
	"fmt"
	"io"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	migrationfixture "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/migration"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/session/testdata"
)

const fixtureInstallCredential = store.Credential("m1-fixture-install-operator-credential")

type fixturePackageSelection struct {
	Config           install.PolicyConfig
	Installs         []install.Request
	Session          install.SessionRequest
	Field, EventType string
}

// Only the private operator configuration selects a fixed fixture version. The
// existing daemon fixture remains the default; no running lock resolves ranges.
func selectFixturePackages(runtime install.RuntimeConfig, cfg fixtureConfig) (fixturePackageSelection, error) {
	if cfg.MigrationVersion == "" {
		pkg, config, err := fixture.Build(runtime, "")
		if err != nil {
			return fixturePackageSelection{}, err
		}
		exported, err := pkg.Export()
		if err != nil {
			return fixturePackageSelection{}, err
		}
		return fixturePackageSelection{Config: config, Installs: []install.Request{{Credential: fixtureInstallCredential, Workspace: cfg.Workspace, ID: "fixture-install", Root: install.Input{Archive: bytes.NewReader(exported.Bytes())}}}, Session: install.SessionRequest{Credential: fixtureInstallCredential, Workspace: cfg.Workspace, Session: cfg.Session, Root: string(pkg.ArtifactIdentity().Digest())}, Field: "counter", EventType: fixture.PackageID + "/change"}, nil
	}
	if cfg.MigrationVersion != "1.0.0" && cfg.MigrationVersion != "1.1.0" {
		return fixturePackageSelection{}, install.ErrPolicy
	}
	pair, err := migrationfixture.BuildPair(runtime, true, false)
	if err != nil {
		return fixturePackageSelection{}, err
	}
	input := func(pkg *archive.Package) (install.Input, error) {
		exported, err := pkg.Export()
		if err != nil {
			return install.Input{}, err
		}
		return install.Input{Archive: bytes.NewReader(exported.Bytes()), Evidence: pair.Evidence[string(pkg.ArtifactIdentity().Digest())]}, nil
	}
	next := cfg.MigrationVersion == "1.1.0"
	selection := fixturePackageSelection{Config: pair.Config, Session: pair.Request(string(fixtureInstallCredential), cfg.Workspace, cfg.Session, next), Field: "counter", EventType: migrationfixture.PackageID + "/change"}
	if next {
		selection.Field = "total"
	}
	for n, graph := range []migrationfixture.Graph{pair.Old, pair.New} {
		r := install.Request{Credential: fixtureInstallCredential, Workspace: cfg.Workspace, ID: fmt.Sprintf("fixture-migration-install-%d", n)}
		r.Root, err = input(graph.Root)
		if err != nil {
			return selection, err
		}
		for _, pkg := range graph.Dependencies {
			dep, err := input(pkg)
			if err != nil {
				return selection, err
			}
			r.Dependencies = append(r.Dependencies, dep)
		}
		selection.Installs = append(selection.Installs, r)
	}
	return selection, nil
}

var fixtureMigrationGuard func(fixtureConfig) (io.Closer, error)
var runMigrationFixture = func(_ context.Context, _ []string, _ io.Writer, errOut io.Writer) int {
	fmt.Fprintln(errOut, "FIXTURE_MIGRATION_PLATFORM_REQUIRED")
	return 2
}
