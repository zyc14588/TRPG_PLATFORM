// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package action

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

func allowed() map[string]func(checkpoint.Value) error {
	return map[string]func(checkpoint.Value) error{"increment": func(v checkpoint.Value) error {
		if v.Kind != "table" || len(v.Table) != 1 || v.Table["delta"].Number != "1" {
			return auth.ErrDenied
		}
		return nil
	}}
}
func proposalRaw() []byte {
	b, _ := json.Marshal(ProposalData{Type: "increment", ExpectedVersion: 2, Payload: checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(1)})})
	return b
}
func TestStructuredProposalAllowsOneLocalRepairAndRejectsAuthorityEscalation(t *testing.T) {
	for _, raw := range [][]byte{proposalRaw(), []byte("```json\n" + string(proposalRaw()) + "\n```")} {
		p, e := Decode(raw, 2, allowed())
		if e != nil || p.StorageValue().Type != "increment" {
			t.Fatal("bounded structured proposal rejected")
		}
	}
	for _, name := range []string{"version", "type", "extra", "duplicate", "payload", "size", "trailing"} {
		t.Run(name, func(t *testing.T) {
			raw := string(proposalRaw())
			switch name {
			case "version":
				raw = strings.Replace(raw, `"expected_state_version":2`, `"expected_state_version":3`, 1)
			case "type":
				raw = strings.Replace(raw, "increment", "write-event", 1)
			case "extra":
				raw = raw[:len(raw)-1] + `,"authority":true}`
			case "duplicate":
				raw = raw[:len(raw)-1] + `,"type":"increment"}`
			case "payload":
				raw = strings.Replace(raw, `"number":"1"`, `"number":"999"`, 1)
			case "size":
				raw = strings.Repeat("x", MaxActionBytes+1)
			case "trailing":
				raw += "{}"
			}
			if _, e := Decode([]byte(raw), 2, allowed()); e == nil {
				t.Fatal("invalid authority proposal accepted")
			}
		})
	}
}
func commitFixture(t *testing.T) Commit {
	t.Helper()
	r := data.Receipt{Header: data.Header{Binding: data.Binding{Workspace: "owned-workspace", Session: "owned-session", GraphHash: checkpoint.Hash([]byte("graph"))}, Principal: "task-system", CommandID: "owned-command", ExpectedVersion: 2}, Version: 3, Cursor: 7}
	c, e := Committed(r, checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(4)}))
	if e != nil {
		t.Fatal("committed fixture rejected")
	}
	return c
}
func TestNarrativeRunsAfterCommitAndFailureKeepsDeterministicResult(t *testing.T) {
	c := commitFixture(t)
	for _, name := range []string{"error", "panic", "empty", "oversize", "cancelled"} {
		t.Run(name, func(t *testing.T) {
			ctx, cancel := context.WithCancel(context.Background())
			defer cancel()
			if name == "cancelled" {
				cancel()
			}
			text, used, e := AfterCommit(ctx, c, func(_ context.Context, got Commit) (string, error) {
				if got.Version() != 3 {
					t.Fatal("narrative saw uncommitted version")
				}
				switch name {
				case "panic":
					panic("synthetic-private-provider-error")
				case "empty":
					return "", nil
				case "oversize":
					return strings.Repeat("x", MaxNarrativeBytes+1), nil
				case "cancelled":
					return "synthetic narrative", nil
				}
				return "", errors.New("synthetic-private-provider-error")
			})
			want, _ := c.Template()
			if e != nil || !used || text != want || c.Version() != 3 || strings.Contains(text, "synthetic-private") {
				t.Fatal("narrative failure changed committed result or leaked error")
			}
		})
	}
	text, used, e := AfterCommit(context.Background(), c, func(context.Context, Commit) (string, error) { return "行动成功。", nil })
	if e != nil || used || text != "行动成功。" {
		t.Fatal("valid committed narrative rejected")
	}
}
func TestCommittedHandleRejectsIncompleteReceiptAndOwnsFilteredCopy(t *testing.T) {
	if _, e := Committed(data.Receipt{}, checkpoint.Int(1)); e == nil {
		t.Fatal("uncommitted receipt admitted")
	}
	c := commitFixture(t)
	v, e := c.Result()
	if e != nil {
		t.Fatal("result unavailable")
	}
	v.Table["counter"] = checkpoint.Int(999)
	fresh, _ := c.Result()
	if fresh.Table["counter"].Number != "4" {
		t.Fatal("private filtered commit aliased caller value")
	}
	for _, format := range []string{"%v", "%+v", "%#v", "%q", "%.*s"} {
		if strings.Contains(fmt.Sprintf(format, c), "counter") {
			t.Fatal("private result in diagnostic")
		}
	}
	if _, e := json.Marshal(c); e == nil {
		t.Fatal("private commit exported")
	}
}
