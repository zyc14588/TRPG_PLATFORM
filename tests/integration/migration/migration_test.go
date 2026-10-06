//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package migration_test

import (
	"context"
	"errors"
	"reflect"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	lockmigration "github.com/zyc14588/TRPG_PLATFORM/internal/package/migration"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/migration"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/recovery"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

func TestExactFiveRoleUpgradeOldEventReplayAndRecordedPointRestore(t *testing.T) {
	e := setup(t, true, false)
	b := e.create(t, "positive", 1)
	old := e.reconstruct(t, b.Session, false)
	request := e.migrationRequest(b.Session, 2)
	g, err := e.reader.LoadGraph(context.Background(), request.From.Credential, e.workspace, request.From.Root, request.From.Dependencies)
	if err != nil {
		t.Fatal(err)
	}
	lock, err := lockmigration.ExactLock(g)
	if err != nil {
		t.Fatal(err)
	}
	if len(lock.Packages) != 5 {
		t.Fatal("five roles not locked")
	}
	for _, p := range lock.Packages {
		if p.Version != "1.0.0" || len(p.Schemas) == 0 {
			t.Fatal("incomplete exact package lock")
		}
	}
	events, err := e.host.ReadJournal(context.Background(), b, 0, 16)
	if err != nil {
		t.Fatal(err)
	}
	beforeControl, err := e.host.InspectMigration(context.Background(), b)
	if err != nil {
		t.Fatal(err)
	}
	beforePhysical, err := e.host.InspectRecovery(context.Background(), b)
	if err != nil {
		t.Fatal(err)
	}
	upgraded, err := e.operator(t, e.host).Run(context.Background(), request)
	if err != nil {
		t.Fatal(err)
	}
	next := data.Binding{Workspace: b.Workspace, Session: b.Session, GraphHash: e.pair.New.Hash()}
	migrated := e.reconstruct(t, b.Session, true)
	if migrated.Image.Version != 3 || migrated.Image.Cursor != old.Image.Cursor || !reflect.DeepEqual(migrated.Image.State, fixture.State(true, 2)) || len(migrated.Image.Rows) != 1 || migrated.Image.Rows[0].Namespace != "records" || !reflect.DeepEqual(migrated.Image.Rows[0].Value.Table["value"], checkpoint.Int(2)) || len(migrated.Image.Quantities) != 1 || migrated.Image.Quantities[0].Value != 1 || !migrated.CheckpointAccepted {
		t.Fatal("migration image/checkpoint invalid")
	}
	newEvents, err := e.host.ReadJournal(context.Background(), next, 0, 16)
	if err != nil || !reflect.DeepEqual(newEvents.Events, events.Events) {
		t.Fatal("old event bytes changed", err)
	}
	point, err := e.host.ReadMigrationPoint(context.Background(), next, request.PointID)
	if err != nil || point.Hash != upgraded.PointHash || eventstore.Digest(point.Verified.State) != eventstore.Digest(old.Image.State) {
		t.Fatal("pre-upgrade point lost", err)
	}
	control, err := e.host.InspectMigration(context.Background(), next)
	if err != nil || control.Points != 1 || control.Stages+control.Documents+control.Quantities != 0 || control.Pins != 10 || control.OriginHash != beforeControl.OriginHash || control.ActiveHash == beforeControl.ActiveHash {
		t.Fatal("atomic lock/isolation/pinning invalid", err)
	}
	restore := request
	restore.From, restore.To = request.To, request.From
	restore.ExpectedVersion = 3
	restore.CommandID = "upgrade-restore"
	restored, err := e.operator(t, e.host).RestorePoint(context.Background(), restore)
	if err != nil {
		t.Fatal(err)
	}
	physical := e.physical(t, b, 4)
	if !reflect.DeepEqual(physical.State, old.Image.State) || !reflect.DeepEqual(physical.Rows, old.Image.Rows) || !reflect.DeepEqual(physical.Quantities, old.Image.Quantities) || physical.Version != 4 {
		t.Fatal("pre-upgrade database facts not restored")
	}
	afterPhysical, err := e.host.InspectRecovery(context.Background(), b)
	if err != nil || afterPhysical.DerivedHash != beforePhysical.DerivedHash {
		t.Fatal("recorded raw state/data/provenance/cache/checkpoint bytes not restored", err)
	}
	afterControl, err := e.host.InspectMigration(context.Background(), b)
	if err != nil || afterControl.ActiveHash != beforeControl.ActiveHash || afterControl.OriginHash != beforeControl.OriginHash || afterControl.Stages != 0 {
		t.Fatal("recorded lock not restored", err)
	}
	recovered := e.reconstruct(t, b.Session, false)
	if !reflect.DeepEqual(recovered.Image.State, old.Image.State) || recovered.Image.Cursor != old.Image.Cursor || recovered.Image.Version != 4 {
		t.Fatal("restored history not reconstructible")
	}
	original, err := e.host.ReadJournal(context.Background(), b, 0, 16)
	if err != nil || !reflect.DeepEqual(original.Events, events.Events) {
		t.Fatal("point restoration rewrote event history", err)
	}
	if _, err = e.operator(t, e.host).RestorePoint(context.Background(), restore); err == nil {
		t.Fatal("repeat point restoration accepted")
	}
	evidence(t, "upgrade-and-point-restoration", struct {
		Upgrade, Restore any
		OldEventsHash    string
		Control          postgres.MigrationInspection
	}{upgraded, restored, eventstore.Digest(events.Events), afterControl})
}
func TestSQLAbortAtEveryMigrationPhaseLeavesBaselineExactlyUsable(t *testing.T) {
	for _, phase := range []string{"migration-locked", "migration-point", "migration-stage", "migration-validated", "migration-data", "migration-effect", "migration-lock", "migration-cache", "migration-commit"} {
		t.Run(phase, func(t *testing.T) {
			e := setup(t, true, false)
			b := e.create(t, "abort", 1)
			old := e.reconstruct(t, b.Session, false)
			before, err := e.host.InspectRecovery(context.Background(), b)
			if err != nil {
				t.Fatal(err)
			}
			control, err := e.host.InspectMigration(context.Background(), b)
			if err != nil {
				t.Fatal(err)
			}
			fault, err := e.host.WithMigrationTransactionAbort(phase)
			if err != nil {
				t.Fatal(err)
			}
			if _, err = e.operator(t, fault).Run(context.Background(), e.migrationRequest(b.Session, 2)); err == nil {
				t.Fatal("real SQL abort hidden")
			}
			after, err := e.host.InspectRecovery(context.Background(), b)
			if err != nil || after != before {
				t.Fatal("partial baseline mutation", err, before, after)
			}
			afterControl, err := e.host.InspectMigration(context.Background(), b)
			if err != nil || afterControl != control {
				t.Fatal("partial lock/point/isolation mutation", err)
			}
			rebuilt := e.reconstruct(t, b.Session, false)
			if !reflect.DeepEqual(old.Image, rebuilt.Image) {
				t.Fatal("old recovery changed")
			}
			evidence(t, phase, struct {
				Before, After postgres.RecoveryInspection
				Control       postgres.MigrationInspection
			}{before, after, afterControl})
		})
	}
}
func TestUnsafeUndeclaredAndCriticalBoundariesDenyBeforePoint(t *testing.T) {
	for _, c := range []struct {
		Name           string
		Safe, Critical bool
	}{{"declared-false", false, false}, {"critical-continuation", true, true}} {
		t.Run(c.Name, func(t *testing.T) {
			e := setup(t, c.Safe, c.Critical)
			b := e.create(t, "unsafe", 1)
			before, err := e.host.InspectRecovery(context.Background(), b)
			if err != nil {
				t.Fatal(err)
			}
			if _, err = e.operator(t, e.host).Run(context.Background(), e.migrationRequest(b.Session, 2)); !errors.Is(err, data.ErrDenied) {
				t.Fatal("unsafe Session accepted", err)
			}
			after, err := e.host.InspectRecovery(context.Background(), b)
			if err != nil || after != before {
				t.Fatal("unsafe denial mutated baseline", err)
			}
			control, err := e.host.InspectMigration(context.Background(), b)
			if err != nil || control.Points+control.Stages+control.Pins != 0 {
				t.Fatal("unsafe denial wrote a point", err)
			}
		})
	}
}
func TestNotStartedSessionNeedsNoDeclaredBoundary(t *testing.T) {
	e := setup(t, false, false)
	b := e.create(t, "unstarted", 0)
	result, err := e.operator(t, e.host).Run(context.Background(), e.migrationRequest(b.Session, 1))
	if err != nil {
		t.Fatal(err)
	}
	report := e.reconstruct(t, b.Session, true)
	if report.Image.Cursor != 0 || report.Image.Version != 2 {
		t.Fatal("unstarted upgrade changed event cursor")
	}
	evidence(t, "unstarted-upgrade", result)
}
func TestInFlightCommandAndStaleHeadDenyBeforePoint(t *testing.T) {
	e := setup(t, true, false)
	b := e.create(t, "inflight", 1)
	tx, err := e.host.Begin(context.Background(), data.Header{Binding: b, Principal: "operator", CommandID: "inflight-command", Fingerprint: eventstore.Digest("inflight"), ExpectedVersion: 2})
	if err != nil {
		t.Fatal(err)
	}
	_, err = e.operator(t, e.host).Run(context.Background(), e.migrationRequest(b.Session, 2))
	closeErr := tx.Rollback()
	if !errors.Is(err, data.ErrConflict) || closeErr != nil {
		t.Fatal("running command not denied", err, closeErr)
	}
	if _, err = e.operator(t, e.host).Run(context.Background(), e.migrationRequest(b.Session, 1)); !errors.Is(err, data.ErrConflict) {
		t.Fatal("stale head accepted", err)
	}
	control, err := e.host.InspectMigration(context.Background(), b)
	if err != nil || control.Points+control.Stages != 0 {
		t.Fatal("concurrency denial wrote point", err)
	}
}
func TestPlanSchemaFailureAndPostUpgradeCommandDenyPointRestoration(t *testing.T) {
	e := setup(t, true, false)
	b := e.create(t, "schema", 1)
	r := e.migrationRequest(b.Session, 2)
	r.Plan.StateFields = nil
	if _, err := e.operator(t, e.host).Run(context.Background(), r); err == nil {
		t.Fatal("unmigrated state schema accepted")
	}
	control, err := e.host.InspectMigration(context.Background(), b)
	if err != nil || control.Points != 0 {
		t.Fatal("invalid plan saved point", err)
	}
	r = e.migrationRequest(b.Session, 2)
	if _, err = e.operator(t, e.host).Run(context.Background(), r); err != nil {
		t.Fatal(err)
	}
	s, err := e.factory.Recover(context.Background(), r.To, func(ctx context.Context, o install.RecoveryContext) (install.RecoveryResult, error) {
		service, _ := recovery.New(e.host)
		return service.Build(ctx, o)
	})
	if err != nil {
		t.Fatal(err)
	}
	_, err = s.Commands.Execute(context.Background(), s.VM.Token(), hostapi.Command{ID: "after-upgrade", Principal: "operator", ExpectedVersion: 3, Input: fixture.Command(1)})
	closeErr := s.Close()
	if err != nil || closeErr != nil {
		t.Fatal(err, closeErr)
	}
	r.From, r.To = r.To, r.From
	r.ExpectedVersion = 4
	r.CommandID = "late-restore"
	if _, err = e.operator(t, e.host).RestorePoint(context.Background(), r); err == nil {
		t.Fatal("restore point discarded intervening command")
	}
}
