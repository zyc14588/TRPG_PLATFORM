//go:build linux

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"os"
	"syscall"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	migrationfixture "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/migration"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/migration"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

// The Linux fixture daemon and offline operator share an OS-held exclusive
// guard. It is released even after SIGKILL; an active daemon cannot be migrated.
func lockMigrationFixture(cfg fixtureConfig) (io.Closer, error) {
	path := cfg.Objects + ".m1-" + cfg.Workspace + "-" + cfg.Session + ".lock"
	fd, err := syscall.Open(path, syscall.O_CREAT|syscall.O_RDWR|syscall.O_NOFOLLOW|syscall.O_CLOEXEC, 0600)
	if err != nil {
		return nil, data.ErrDenied
	}
	f := os.NewFile(uintptr(fd), path)
	stat, err := f.Stat()
	if err != nil || !stat.Mode().IsRegular() || stat.Mode().Perm()&0077 != 0 {
		f.Close()
		return nil, data.ErrDenied
	}
	if err = syscall.Flock(fd, syscall.LOCK_EX|syscall.LOCK_NB); err != nil {
		f.Close()
		return nil, data.ErrConflict
	}
	return f, nil
}

func init() {
	fixtureMigrationGuard = lockMigrationFixture
	runMigrationFixture = runLinuxMigrationFixture
}

func runLinuxMigrationFixture(ctx context.Context, args []string, out, errOut io.Writer) int {
	flags := flag.NewFlagSet("m1-migration-fixture", flag.ContinueOnError)
	flags.SetOutput(errOut)
	path := flags.String("config", "", "private operator configuration")
	operation := flags.String("operation", "", "upgrade or restore-point")
	expected := flags.Uint64("expected-version", 0, "exact current Session version")
	point := flags.String("point", "before-upgrade", "recorded recovery point ID")
	command := flags.String("command", "fixture-upgrade", "unique operator command ID")
	if flags.Parse(args) != nil || flags.NArg() != 0 || *path == "" || (*operation != "upgrade" && *operation != "restore-point") || *expected == 0 || !store.ValidID(*point) || !store.ValidID(*command) {
		return 2
	}
	cfg, code := readFixtureConfig(*path)
	if code != "" {
		fmt.Fprintln(errOut, code)
		return 1
	}
	if (*operation == "upgrade" && cfg.MigrationVersion != "1.0.0") || (*operation == "restore-point" && cfg.MigrationVersion != "1.1.0") {
		fmt.Fprintln(errOut, "FIXTURE_MIGRATION_CONFIG_REJECTED")
		return 1
	}
	guard, err := lockMigrationFixture(cfg)
	if err != nil {
		fmt.Fprintln(errOut, "FIXTURE_MIGRATION_REQUIRES_STOPPED_DAEMON")
		return 1
	}
	defer guard.Close()
	ctx, cancel := context.WithTimeout(ctx, 30*time.Second)
	defer cancel()
	runtime := install.RuntimeConfig{Runner: cfg.Runner, SHA256: cfg.RunnerHash, Limits: profile.DefaultLimits()}
	pair, err := migrationfixture.BuildPair(runtime, true, false)
	if err != nil {
		fmt.Fprintln(errOut, "FIXTURE_MIGRATION_FAILED")
		return 1
	}
	policy, err := install.NewPolicy(pair.Config)
	if err != nil {
		fmt.Fprintln(errOut, "FIXTURE_MIGRATION_FAILED")
		return 1
	}
	access, err := store.NewAccess(map[store.Credential][]store.Membership{fixtureInstallCredential: {{Principal: "operator", Workspace: cfg.Workspace, Read: true, Install: true}}})
	if err != nil {
		fmt.Fprintln(errOut, "FIXTURE_MIGRATION_FAILED")
		return 1
	}
	services, err := openPackageServices(ctx, cfg.DSN, cfg.Objects, install.Options{StagingRoot: cfg.Staging, Policy: policy, Access: access, Runtime: runtime, Observe: func(string) error { return nil }, Execution: func(e install.Execution) error {
		raw, err := json.Marshal(e)
		if err != nil {
			return err
		}
		_, err = fmt.Fprintf(errOut, "FIXTURE_EXECUTION %s\n", raw)
		return err
	}}, sessionValidation{Audit: func(data.Audit) error { return nil }, Validate: func(context.Context, data.Commit) error { return nil }})
	if err != nil {
		fmt.Fprintln(errOut, "FIXTURE_MIGRATION_FAILED")
		return 1
	}
	defer services.Close()
	operator, err := migration.New(services.Sessions, func(ctx context.Context, from, to *store.Graph, b data.Binding, v uint64) (migration.Lease, error) {
		return services.HostCommands.AcquireMigration(ctx, from, to, b, v)
	})
	if err != nil {
		fmt.Fprintln(errOut, "FIXTURE_MIGRATION_FAILED")
		return 1
	}
	r := migration.Request{From: pair.Request(string(fixtureInstallCredential), cfg.Workspace, cfg.Session, false), To: pair.Request(string(fixtureInstallCredential), cfg.Workspace, cfg.Session, true), ExpectedVersion: *expected, PointID: *point, CommandID: *command, Plan: pair.Plan()}
	var result migration.Result
	if *operation == "restore-point" {
		r.From, r.To = r.To, r.From
		result, err = operator.RestorePoint(ctx, r)
	} else {
		result, err = operator.Run(ctx, r)
	}
	if err != nil {
		fmt.Fprintln(errOut, "FIXTURE_MIGRATION_FAILED")
		return 1
	}
	if json.NewEncoder(out).Encode(result) != nil {
		return 1
	}
	return 0
}
