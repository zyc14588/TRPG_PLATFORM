//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package migration_test

import (
	"context"
	"errors"
	"fmt"
	"reflect"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/migration"
	sessionmigration "github.com/zyc14588/TRPG_PLATFORM/internal/session/migration"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/recovery"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

func (e *environment) assertUnchanged(t *testing.T, b data.Binding, attempt func() error) {
	t.Helper()
	before, err := e.host.InspectRecovery(context.Background(), b)
	if err != nil {
		t.Fatal(err)
	}
	control, err := e.host.InspectMigration(context.Background(), b)
	if err != nil {
		t.Fatal(err)
	}
	if err = attempt(); err == nil {
		t.Fatal("invalid migration accepted")
	}
	after, err := e.host.InspectRecovery(context.Background(), b)
	if err != nil || after != before {
		t.Fatal("denial or failure changed physical baseline", err)
	}
	afterControl, err := e.host.InspectMigration(context.Background(), b)
	if err != nil || afterControl != control {
		t.Fatal("denial or failure changed locks, points, pins or stages", err)
	}
}

func TestUndeclaredBoundaryFailsBeforePoint(t *testing.T) {
	e := setupOptions(t, fixture.Options{Safe: true, Undeclared: true})
	b := e.create(t, "undeclared", 1)
	authority, err := e.factory.Authenticate(context.Background(), e.request(b.Session, false))
	if err != nil || authority.SafeBoundaryDeclared {
		t.Fatal("fixture did not authenticate an undeclared boundary", err)
	}
	e.assertUnchanged(t, b, func() error {
		_, err := e.operator(t, e.host).Run(context.Background(), e.migrationRequest(b.Session, 2))
		if !errors.Is(err, data.ErrDenied) {
			t.Fatal("undeclared boundary not denied", err)
		}
		return err
	})
}

func TestEndedSessionUpgradeAndPointRestoreRetainTerminalHistory(t *testing.T) {
	e := setup(t, false, false)
	b := e.create(t, "ended", 1)
	rebuild, err := recovery.New(e.host)
	if err != nil {
		t.Fatal(err)
	}
	s, err := e.factory.Recover(context.Background(), e.request(b.Session, false), rebuild.Build)
	if err != nil {
		t.Fatal(err)
	}
	_, err = s.Commands.Execute(context.Background(), s.VM.Token(), hostapi.Command{Callback: "on_session_end", ID: "end", Principal: "operator", ExpectedVersion: 2, Input: checkpoint.Object(map[string]checkpoint.Value{}), Envelope: &data.EnvelopeMetadata{Seat: "gm", Type: "end", Correlation: "fixture"}})
	closeErr := s.Close()
	if err != nil || closeErr != nil {
		t.Fatal(err, closeErr)
	}
	old := e.reconstruct(t, b.Session, false)
	if !old.Image.Ended {
		t.Fatal("fixture never committed ending")
	}
	original, err := e.host.ReadJournal(context.Background(), b, 0, 16)
	if err != nil {
		t.Fatal(err)
	}
	r := e.migrationRequest(b.Session, 3)
	if _, err = e.operator(t, e.host).Run(context.Background(), r); err != nil {
		t.Fatal(err)
	}
	next := e.reconstruct(t, b.Session, true)
	if !next.Image.Ended || next.Image.Cursor != old.Image.Cursor || next.Image.Version != 4 {
		t.Fatal("ended migration changed ending or event cursor")
	}
	r.From, r.To = r.To, r.From
	r.ExpectedVersion, r.CommandID = 4, "restore-ended"
	if _, err = e.operator(t, e.host).RestorePoint(context.Background(), r); err != nil {
		t.Fatal(err)
	}
	after := e.reconstruct(t, b.Session, false)
	journal, err := e.host.ReadJournal(context.Background(), b, 0, 16)
	if err != nil || !after.Image.Ended || after.Image.Version != 5 || !reflect.DeepEqual(after.Image.State, old.Image.State) || !reflect.DeepEqual(journal.Events, original.Events) {
		t.Fatal("point restore lost terminal history", err)
	}
	if tx, err := e.host.Begin(context.Background(), data.Header{Binding: b, Principal: "operator", CommandID: "after-ending", ExpectedVersion: 5, Fingerprint: checkpoint.Hash([]byte("terminal-command"))}); err == nil {
		tx.Rollback()
		t.Fatal("restoration reopened ended Session")
	}
}

func TestRestoreSQLAbortAtEveryPhaseKeepsUpgradedBaselineAndPoint(t *testing.T) {
	for _, phase := range []string{"migration-locked", "migration-point", "migration-stage", "migration-validated", "migration-data", "migration-effect", "migration-lock", "migration-cache", "migration-commit"} {
		t.Run(phase, func(t *testing.T) {
			e := setup(t, true, false)
			b := e.create(t, "restore-abort", 1)
			r := e.migrationRequest(b.Session, 2)
			if _, err := e.operator(t, e.host).Run(context.Background(), r); err != nil {
				t.Fatal(err)
			}
			r.From, r.To = r.To, r.From
			r.ExpectedVersion, r.CommandID = 3, "restore-abort"
			next := data.Binding{Workspace: b.Workspace, Session: b.Session, GraphHash: e.pair.New.Hash()}
			fault, err := e.host.WithMigrationTransactionAbort(phase)
			if err != nil {
				t.Fatal(err)
			}
			e.assertUnchanged(t, next, func() error { _, err := e.operator(t, fault).RestorePoint(context.Background(), r); return err })
			if _, err = e.operator(t, e.host).RestorePoint(context.Background(), r); err != nil {
				t.Fatal("aborted restore consumed recorded point", err)
			}
			if e.reconstruct(t, b.Session, false).Image.Version != 4 {
				t.Fatal("old Session not recoverable after retry")
			}
		})
	}
}

func TestTargetRestoreRehearsalFailureKeepsOldSessionUsable(t *testing.T) {
	e := setupOptions(t, fixture.Options{Safe: true, TargetRestoreFailure: true})
	b := e.create(t, "rehearsal-failure", 7)
	e.assertUnchanged(t, b, func() error {
		_, err := e.operator(t, e.host).Run(context.Background(), e.migrationRequest(b.Session, 8))
		return err
	})
	if report := e.reconstruct(t, b.Session, false); report.Image.Version != 8 || !reflect.DeepEqual(report.Image.State, fixture.State(false, 8)) {
		t.Fatal("failed target rehearsal lost old recovery")
	}
}

func TestMigrationAuthorityGraphAndEntryBoundsFailClosed(t *testing.T) {
	e := setup(t, true, false)
	cases := []struct {
		name   string
		mutate func(*sessionmigration.Request)
	}{
		{"no-membership", func(r *sessionmigration.Request) { r.From.Credential = "unknown-fixture-principal" }},
		{"read-without-install", func(r *sessionmigration.Request) {
			r.From.Credential, r.To.Credential = readOnlyCredential, readOnlyCredential
		}},
		{"principal-mismatch", func(r *sessionmigration.Request) { r.To.Credential = readOnlyCredential }},
		{"cross-workspace", func(r *sessionmigration.Request) { r.To.Workspace = "other-workspace" }},
		{"cross-session", func(r *sessionmigration.Request) { r.To.Session = "other-session" }},
		{"uninstalled-hash", func(r *sessionmigration.Request) { r.To.Root = checkpoint.Hash([]byte("uninstalled-artifact")) }},
		{"extra-dependency", func(r *sessionmigration.Request) { r.To.Dependencies = append(r.To.Dependencies, r.To.Root) }},
		{"substituted-dependency", func(r *sessionmigration.Request) { r.To.Dependencies[0] = r.From.Dependencies[0] }},
		{"unknown-evidence", func(r *sessionmigration.Request) {
			r.To.Evidence[checkpoint.Hash([]byte("unknown-evidence"))] = r.To.Evidence[r.To.Root]
		}},
		{"plan-lock-mismatch", func(r *sessionmigration.Request) { r.Plan.ToLock = r.Plan.FromLock }},
		{"zero-head", func(r *sessionmigration.Request) { r.ExpectedVersion = 0 }},
		{"point-traversal", func(r *sessionmigration.Request) { r.PointID = "../outside" }},
		{"unbounded-plan", func(r *sessionmigration.Request) { r.Plan.StateFields = make([]sessionmigration.FieldMove, 1025) }},
	}
	for n, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			b := e.create(t, fmt.Sprintf("entry-%d", n), 1)
			r := e.migrationRequest(b.Session, 2)
			c.mutate(&r)
			e.assertUnchanged(t, b, func() error { _, err := e.operator(t, e.host).Run(context.Background(), r); return err })
		})
	}
}
