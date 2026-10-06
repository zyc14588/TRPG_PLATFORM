//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package replay_test

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"reflect"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	replayfixture "github.com/zyc14588/TRPG_PLATFORM/internal/session/testdata/replay"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

// Dense valid events can exceed the ordinary 32 MiB operator readback budget
// before reaching 4 MiB of replay evidence. Use the fixed capacity readback
// seam; the package and test receive no SQL, connection, or configurable bound.
func capacityInspection(t *testing.T, e *environment, b data.Binding) postgres.RecoveryInspection {
	t.Helper()
	got, err := e.host.InspectRecoveryCapacity(context.Background(), b)
	if err != nil {
		t.Fatal(err)
	}
	return got
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
				before := capacityInspection(t, e, b)
				got, err := r.registry.Submit(context.Background(), r.gm, envelope(b, fmt.Sprintf("capacity-command-%d", k), "increment", uint64(k)))
				if err != nil {
					if !errors.Is(err, data.ErrDenied) {
						t.Fatal("command failed before the intended capacity rejection", err)
					}
					if capacityInspection(t, e, b) != before || !reflect.DeepEqual(got, data.Receipt{}) {
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
			before := capacityInspection(t, e, b)
			duplicate, err := r.registry.Submit(context.Background(), r.gm, envelope(b, "capacity-command-1", "increment", 1))
			if err != nil || !duplicate.Replayed || duplicate.Version != 2 || capacityInspection(t, e, b) != before {
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
			if report.Image.Version != h.Version || report.Image.Cursor != h.Cursor || capacityInspection(t, e, b).ImmutableHash != before.ImmutableHash {
				t.Fatal("capacity recovery changed original history, intent, version, or cursor")
			}
			if err = installed.Close(); err != nil {
				t.Fatal(err)
			}
			t.Logf("CAPACITY_RECOVERED boundary=%s complete_records=%d evidence_bytes=%d version=%d cursor=%d immutable=%s", boundary, committed, bytes, h.Version, h.Cursor, before.ImmutableHash)
		})
	}
}
