// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package outbox

import (
	"errors"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"strings"
	"testing"
)

func TestNotificationsRequireCommittedNonReplayReceipt(t *testing.T) {
	r := data.Receipt{Header: data.Header{Binding: data.Binding{Workspace: "workspace", Session: "session", GraphHash: "sha256:" + strings.Repeat("a", 64)}, CommandID: "command", Principal: "gm", Fingerprint: "sha256:" + strings.Repeat("b", 64), ExpectedVersion: 1}, Version: 2, Cursor: 9, Events: []data.Event{{ID: "event", Type: "fixture/change", Payload: checkpoint.Int(1)}}}
	n, err := FromCommitted(r)
	if err != nil || len(n.Events) != 1 || n.Events[0].Sequence != 9 {
		t.Fatal(n, err)
	}
	for _, alter := range []func(*data.Receipt){func(r *data.Receipt) { r.Replayed = true }, func(r *data.Receipt) { r.Header.ReadOnly = true }, func(r *data.Receipt) { r.Events = nil }, func(r *data.Receipt) { r.Version = 1 }, func(r *data.Receipt) { r.Cursor = 0 }, func(r *data.Receipt) { r.Header.Principal = "" }} {
		rejected := r
		alter(&rejected)
		if _, err := FromCommitted(rejected); !errors.Is(err, ErrNotification) {
			t.Fatal("uncommitted/replayed notification", err)
		}
	}
}
