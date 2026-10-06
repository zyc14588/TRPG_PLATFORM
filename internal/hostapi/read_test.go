// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package hostapi_test

import (
	"context"
	"fmt"
	host "github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/hostapi/hostapitest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"strings"
	"testing"
)

// This unit fixture explicitly grants official module trust to reach the named
// operation guard; it is not installed-package publisher/signature evidence.
func TestReadOnlyNamedWriteDeniedAfterAuthorizedNamedRead(t *testing.T) {
	source := strings.Replace(fixture.Source(""), "return M", `M.project_view=function(input)
 if input.add then return host.db.named("increase",{key="one",delta=1}) end
 return host.db.named("read",{key="one",delta=0})
 end
 return M`, 1)
	pkg, err := fixture.Package("", source, nil)
	if err != nil {
		t.Fatal(err)
	}
	s, err := fixture.Runtime(context.Background(), runner, fmt.Sprintf("named-read-%d", sequence.Add(1)), pkg, capability.TrustOfficial)
	if err != nil {
		t.Fatal(err)
	}
	defer s.Destroy()
	m := &memory{receipts: map[string]data.Receipt{}}
	o, err := fixture.Options(s, pkg, m, "unit-workspace")
	if err != nil {
		t.Fatal(err)
	}
	denied := false
	o.Audit = func(a data.Audit) error {
		if a.Operation == "host.db.named" && a.Outcome == "VALIDATION_EFFECT_DENIED" {
			denied = true
		}
		return nil
	}
	m.snapshot = data.Snapshot{Binding: o.Binding, Version: 1, State: fixture.State(), SchemaHash: o.StateSchema.Digest()}
	service, err := host.New(o)
	if err != nil {
		t.Fatal(err)
	}
	c := host.Command{ID: "allowed-read", Principal: "gm", ExpectedVersion: 1, Callback: "project_view", Input: checkpoint.Object(map[string]checkpoint.Value{"add": checkpoint.Bool(false)})}
	receipt, err := service.Read(context.Background(), s.Token(), c)
	if err != nil || receipt.Version != 1 || receipt.Result.Number != "0" || len(m.commits) != 0 {
		t.Fatal("authorized named read unavailable", receipt, err)
	}
	c.ID = "denied-write"
	c.Input.Table["add"] = checkpoint.Bool(true)
	receipt, err = service.Read(context.Background(), s.Token(), c)
	assertRollback(t, m, receipt, err)
	if !denied {
		t.Fatal("named write did not reach readonly effect guard")
	}
	assertPoisoned(t, s)
}
