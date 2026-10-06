//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package replay_test

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/json"
	"fmt"
	"reflect"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	replayfixture "github.com/zyc14588/TRPG_PLATFORM/internal/session/testdata/replay"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

// The dense event fixture can exceed the ordinary operator inspection's
// 32 MiB JSON/hex readback budget before reaching 4 MiB of actual evidence.
// This test-only fixed SQL reader streams at most 64 MiB into digests; it never
// returns payloads, writes data, or changes the production inspection budget.
func capacityInspection(t *testing.T, b data.Binding) postgres.RecoveryInspection {
	t.Helper()
	db, err := sql.Open("pgx", dsn)
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	immutable, derived := sha256.New(), sha256.New()
	size := 0
	queries := []string{
		`SELECT jsonb_build_array(graph_hash,version,event_sequence,schema_hash) FROM host_command.sessions WHERE workspace=$1 AND session=$2`,
		`SELECT to_jsonb(t) FROM host_command.creation t WHERE workspace=$1 AND session=$2`,
		`SELECT to_jsonb(t) FROM host_command.installed_graphs t WHERE workspace=$1 AND session=$2`,
		`SELECT to_jsonb(t) FROM host_command.replay_effects t WHERE workspace=$1 AND session=$2 ORDER BY version`,
		`SELECT to_jsonb(t) FROM host_command.requests t WHERE workspace=$1 AND session=$2 ORDER BY command_id`,
		`SELECT to_jsonb(t) FROM host_command.events t WHERE workspace=$1 AND session=$2 ORDER BY sequence`,
		`SELECT to_jsonb(t) FROM host_command.patches t WHERE workspace=$1 AND session=$2 ORDER BY command_id,ordinal`,
		`SELECT to_jsonb(t) FROM host_command.tasks t WHERE workspace=$1 AND session=$2 ORDER BY id`,
		`SELECT to_jsonb(t) FROM host_command.continuations t WHERE workspace=$1 AND session=$2 ORDER BY id`,
		`SELECT to_jsonb(t) FROM host_command.outbox t WHERE workspace=$1 AND session=$2 ORDER BY id`,
		`SELECT to_jsonb(t) FROM host_command.audit t WHERE workspace=$1 AND session=$2 ORDER BY command_id,ordinal`,
		`SELECT to_jsonb(t) FROM host_command.endings t WHERE workspace=$1 AND session=$2`,
		`SELECT to_jsonb(t) FROM package_install.data_targets t WHERE workspace=$1 AND state_reference='host-session:'||$2 ORDER BY package_id`,
		`SELECT state FROM host_command.sessions WHERE workspace=$1 AND session=$2`,
		`SELECT to_jsonb(t) FROM host_command.documents t WHERE workspace=$1 AND session=$2 ORDER BY package_id,namespace,key`,
		`SELECT to_jsonb(t) FROM host_command.quantity t WHERE workspace=$1 AND session=$2 ORDER BY package_id,key`,
		`SELECT to_jsonb(t) FROM host_command.checkpoints t WHERE workspace=$1 AND session=$2`,
		`SELECT to_jsonb(t) FROM host_command.projection_caches t WHERE workspace=$1 AND session=$2`,
	}
	for i, query := range queries {
		digest := immutable
		if i >= 13 {
			digest = derived
		}
		fmt.Fprintf(digest, "%d:%s\n", i, query)
		rows, err := db.QueryContext(context.Background(), query+" LIMIT $3", b.Workspace, b.Session, eventstore.MaxRecords*257+1)
		if err != nil {
			t.Fatal(err)
		}
		count := 0
		for rows.Next() {
			var raw []byte
			if err := rows.Scan(&raw); err != nil {
				rows.Close()
				t.Fatal(err)
			}
			count++
			size += len(raw)
			if count > eventstore.MaxRecords*257 || size > 64<<20 {
				rows.Close()
				t.Fatal("test readback budget exceeded before capacity proof")
			}
			fmt.Fprintf(digest, "%d:", len(raw))
			digest.Write(raw)
		}
		err = rows.Err()
		rows.Close()
		if err != nil {
			t.Fatal(err)
		}
	}
	var records int
	if err = db.QueryRowContext(context.Background(), `SELECT count(*) FROM host_command.replay_effects WHERE workspace=$1 AND session=$2`, b.Workspace, b.Session).Scan(&records); err != nil {
		t.Fatal(err)
	}
	return postgres.RecoveryInspection{ImmutableHash: fmt.Sprintf("sha256:%x", immutable.Sum(nil)), DerivedHash: fmt.Sprintf("sha256:%x", derived.Sum(nil)), Records: records}
}

func TestRealCommandCapacityPreservesLastRecoverableHistory(t *testing.T) {
	for _, boundary := range []string{"records", "bytes"} {
		t.Run(boundary, func(t *testing.T) {
			source := replayfixture.FactsSource
			if boundary == "bytes" {
				// Each payload fits the installed event schema and the normal
				// 64-event command budget. Reach the cumulative byte limit with
				// fully approved commands, not an invalid oversized fixture.
				source = strings.Replace(source, " host.event.emit(\"change\",state())\n", " local event={counter=next,secret=string.rep(\"x\",128)};for i=1,64 do host.event.emit(\"change\",event) end\n", 1)
			}
			e := setup(t, source)
			t.Cleanup(func() { e.assertReaped(t) })
			b := e.create(t, "capacity-"+boundary)
			g, err := e.reader.LoadGraph(context.Background(), credential, e.workspace, string(e.pkg.ArtifactIdentity().Digest()), nil)
			if err != nil {
				t.Fatal(err)
			}
			r := newRig(t, e, b, e.host)
			committed := 0
			var rejected bool
			var cursor uint64
			for k := 1; k <= eventstore.MaxRecords+1; k++ {
				before := capacityInspection(t, b)
				got, err := r.registry.Submit(context.Background(), r.gm, envelope(b, fmt.Sprintf("capacity-command-%d", k), "increment", uint64(k)))
				if err != nil {
					if capacityInspection(t, b) != before || !reflect.DeepEqual(got, data.Receipt{}) {
						t.Fatal("capacity rejection changed committed facts or returned a receipt")
					}
					rejected = true
					break
				}
				if got.Version != uint64(k+1) {
					t.Fatal("complete command did not advance its original version")
				}
				committed++
				cursor = got.Cursor
			}
			if !rejected || (boundary == "records" && committed != eventstore.MaxRecords) || (boundary == "bytes" && (committed == 0 || committed >= eventstore.MaxRecords)) {
				t.Fatalf("cumulative %s boundary not reached: committed=%d rejected=%v", boundary, committed, rejected)
			}
			h, err := e.host.ReadReplayHistory(context.Background(), g, b)
			if err != nil || len(h.Records) != committed || h.Version != uint64(committed+1) || h.Cursor != cursor {
				t.Fatal("last accepted history became unreadable", err)
			}
			bytes := 0
			for _, record := range h.Records {
				raw, err := json.Marshal(record)
				if err != nil || !record.Complete || eventstore.Validate(record) != nil {
					t.Fatal("capacity fixture did not commit complete approved effects", err)
				}
				bytes += len(raw)
			}
			if bytes > eventstore.MaxHistoryBytes || (boundary == "bytes" && bytes < eventstore.MaxHistoryBytes-eventstore.MaxRecordBytes) {
				t.Fatal("byte capacity fixture failed to reach the intended history boundary")
			}
			before := capacityInspection(t, b)
			duplicate, err := r.registry.Submit(context.Background(), r.gm, envelope(b, "capacity-command-1", "increment", 1))
			if err != nil || !duplicate.Replayed || duplicate.Version != 2 || capacityInspection(t, b) != before {
				t.Fatal("original duplicate receipt did not resolve at capacity", err)
			}
			if err = r.registry.Sleep(context.Background(), r.gm); err != nil {
				t.Fatal("sleep after rejection", err)
			}
			if err = r.registry.Close(); err != nil {
				t.Fatal(err)
			}
			if err = e.host.DropDerived(context.Background(), b); err != nil {
				t.Fatal(err)
			}
			installed, report, err := recoverSession(t, e, b, e.host)
			if err != nil {
				t.Fatal("last accepted state cannot reactivate from immutable history", err)
			}
			assertVMFacts(t, installed, int64(committed+1))
			if report.Image.Version != h.Version || report.Image.Cursor != h.Cursor || capacityInspection(t, b).ImmutableHash != before.ImmutableHash {
				t.Fatal("capacity recovery changed original history, intent, version, or cursor")
			}
			if err = installed.Close(); err != nil {
				t.Fatal(err)
			}
			t.Logf("CAPACITY_RECOVERED boundary=%s complete_records=%d evidence_bytes=%d version=%d cursor=%d immutable=%s", boundary, committed, bytes, h.Version, h.Cursor, before.ImmutableHash)
		})
	}
}
