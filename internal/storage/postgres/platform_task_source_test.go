// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package postgres

import (
	"encoding/json"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

func sourceFixture(t *testing.T, kind, dispatch string) ([]byte, task.JobData) {
	t.Helper()
	b := data.Binding{Workspace: "owned-workspace", Session: "owned-session", GraphHash: checkpoint.Hash([]byte("owned-graph"))}
	state := checkpoint.Object(map[string]checkpoint.Value{})
	c := data.Commit{Header: data.Header{Binding: b, Principal: "owned-principal", CommandID: "owned-command", Fingerprint: checkpoint.Hash([]byte("owned-command")), ExpectedVersion: 1}, State: state, SchemaHash: checkpoint.Hash([]byte("owned-schema")), Events: []data.Event{{ID: "owned-event", Type: "example.test/host/changed", Payload: state, SchemaVersion: 1, SchemaHash: checkpoint.Hash([]byte("owned-schema"))}}, Tasks: []data.Intent{{ID: "owned-task", PackageID: "example.test/host", Kind: kind, Payload: checkpoint.Int(7)}}, Continuations: []data.Intent{{ID: "owned-continuation", PackageID: "example.test/host", Kind: "continuation", Payload: checkpoint.Object(map[string]checkpoint.Value{"task": checkpoint.Text("owned-task"), "value": checkpoint.Int(3)})}}, Outbox: []data.Intent{{ID: "owned-outbox", PackageID: "example.test/host", Kind: dispatch, Payload: checkpoint.Object(map[string]checkpoint.Value{"task": checkpoint.Text("owned-task")})}}, Inputs: data.Inputs{Callback: "execute_command", Time: 1, Command: state}, Result: checkpoint.Int(1)}
	r, e := eventstore.Record(data.Snapshot{Binding: b, Version: 1, State: state}, c, 0)
	if e != nil || eventstore.Validate(r) != nil {
		t.Fatal("synthetic immutable fixture invalid")
	}
	raw, e := json.Marshal(r)
	if e != nil {
		t.Fatal("synthetic immutable fixture encoding failed")
	}
	return raw, task.JobData{Binding: b, SourceCommand: "owned-command", TaskID: "owned-task", OutboxID: "owned-outbox", PackageID: "example.test/host"}
}

func TestTaskAndAIIntentsUseTheirMatchingImmutableDispatch(t *testing.T) {
	for _, kind := range []string{"task", "ai"} {
		t.Run(kind, func(t *testing.T) {
			raw, v := sourceFixture(t, kind, "dispatch-"+kind)
			got, e := projectTask(raw, v)
			if e != nil || got.OriginVersion != 2 || got.SourceHash != checkpoint.Hash(raw) || got.OriginPrincipal != "owned-principal" || len(got.Continuations) != 1 {
				t.Fatal("matching durable projection rejected or lost binding")
			}
			p, e := got.Payload.StorageValue()
			if e != nil || p.Number != "7" || got.Continuations[0].ID != "owned-continuation" {
				t.Fatal("durable projection changed payload or continuation")
			}
		})
	}
}

func TestTaskSourceRejectsUnsupportedAndCrossKindDispatch(t *testing.T) {
	for _, c := range []struct{ kind, dispatch string }{{"task", "dispatch-ai"}, {"ai", "dispatch-task"}, {"unknown", "dispatch-unknown"}, {"ai", "dispatch-unknown"}} {
		t.Run(c.kind+"-"+c.dispatch, func(t *testing.T) {
			raw, v := sourceFixture(t, c.kind, c.dispatch)
			if _, e := projectTask(raw, v); e != task.ErrDenied {
				t.Fatal("unsupported or mismatched immutable dispatch admitted")
			}
		})
	}
}

func TestAITaskProjectionKeepsTenantPackageAndSourceBinding(t *testing.T) {
	raw, original := sourceFixture(t, "ai", "dispatch-ai")
	for _, name := range []string{"workspace", "session", "graph", "package", "command", "task", "outbox"} {
		t.Run(name, func(t *testing.T) {
			v := original
			switch name {
			case "workspace":
				v.Binding.Workspace = "foreign"
			case "session":
				v.Binding.Session = "foreign"
			case "graph":
				v.Binding.GraphHash = checkpoint.Hash([]byte("foreign"))
			case "package":
				v.PackageID = "foreign.test/package"
			case "command":
				v.SourceCommand = "foreign"
			case "task":
				v.TaskID = "foreign"
			case "outbox":
				v.OutboxID = "foreign"
			}
			if _, e := projectTask(raw, v); e != task.ErrDenied {
				t.Fatal("foreign task identity admitted")
			}
		})
	}
}
