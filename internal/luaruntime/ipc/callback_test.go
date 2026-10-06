// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package ipc

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
)

// The self-test child deliberately violates the protocol. It is never offered
// as a production runner and receives no execution token or application env.
func TestMain(m *testing.M) {
	if len(os.Args) > 1 && os.Args[1] == "serve" {
		mockCallbacks()
		os.Exit(0)
	}
	os.Exit(m.Run())
}
func mockCallbacks() {
	raw, e := ReadFrame(os.Stdin)
	if e != nil {
		return
	}
	var init Request
	if checkpoint.StrictDecode(raw, &init, MaxFrameBytes) != nil {
		return
	}
	scenario := string(init.Config.Modules["scenario.lua"])
	respond := func(id uint64) {
		r := Response{Version: Version, ID: id, Profile: profile.ID, Runtime: profile.RuntimeVersion, PID: os.Getpid(), Result: profile.Result{Audit: profile.Audit{Level: "AUDIT-0", Sequence: id, Outcome: "PASS"}}}
		if id != init.ID {
			switch scenario {
			case "output-negative":
				r.Result.OutputBytes = -1
			case "output-excess":
				r.Result.OutputBytes = profile.DefaultLimits().OutputBytes + 1
			case "output-missing":
				r.Result.Output = []string{"sha256:untrusted"}
			case "output-unaccounted":
				r.Result.OutputBytes = 1
			}
		}
		b, _ := json.Marshal(r)
		WriteFrame(os.Stdout, b)
	}
	respond(init.ID)
	raw, e = ReadFrame(os.Stdin)
	if e != nil {
		return
	}
	var req Request
	if checkpoint.StrictDecode(raw, &req, MaxFrameBytes) != nil {
		return
	}
	cb := Callback{Kind: "callback", Version: Version, ID: req.ID, Sequence: 1, PID: os.Getpid(), Profile: profile.ID, Runtime: profile.RuntimeVersion, Call: profile.HostCall{Module: "main.lua", Line: 1, Phase: "execute", Capability: "host.state", Operation: "get", Arguments: []checkpoint.Value{checkpoint.Array(checkpoint.Text("x"))}}}
	switch scenario {
	case "version":
		cb.Version++
	case "pid":
		cb.PID++
	case "profile":
		cb.Profile = "other"
	case "runtime":
		cb.Runtime = "other"
	case "outer":
		cb.ID++
	case "sequence":
		cb.Sequence = 0
	case "direction":
		cb.Kind = "callback-reply"
	}
	b, _ := json.Marshal(cb)
	if scenario == "unknown-field" {
		var value map[string]any
		json.Unmarshal(b, &value)
		value["execution_token"] = "untrusted"
		b, _ = json.Marshal(value)
	}
	if scenario == "duplicate-field" {
		b = append([]byte(`{"id":42,`), b[1:]...)
	}
	if WriteFrame(os.Stdout, b) != nil {
		return
	}
	if _, e = ReadFrame(os.Stdin); e != nil {
		return
	}
	if scenario == "repeat" || scenario == "limit" {
		if scenario == "limit" {
			cb.Sequence = 2
		}
		b, _ = json.Marshal(cb)
		if WriteFrame(os.Stdout, b) != nil {
			return
		}
		if _, e = ReadFrame(os.Stdin); e != nil {
			return
		}
	}
	// A hostile child may try to report PASS after a denied callback. The client
	// remembers the denial and rejects this final response independently.
	respond(req.ID)
	_, _ = ReadFrame(os.Stdin)
}
func mockClient(t *testing.T, scenario string) *Client {
	t.Helper()
	exe, e := os.Executable()
	if e != nil {
		t.Fatal(e)
	}
	c, e := Start(context.Background(), exe, profile.Config{Limits: profile.DefaultLimits(), Modules: map[string][]byte{"scenario.lua": []byte(scenario)}, Host: &profile.HostConfig{CallbackLimit: 1}})
	if e != nil {
		t.Fatal(e)
	}
	t.Cleanup(c.Kill)
	return c
}
func TestCallbackDirectionSequenceAndBindingFailClosed(t *testing.T) {
	for _, name := range []string{"version", "pid", "profile", "runtime", "outer", "sequence", "direction", "unknown-field", "duplicate-field", "repeat", "limit"} {
		t.Run(name, func(t *testing.T) {
			c := mockClient(t, name)
			calls := 0
			_, e := c.CallWithHost(context.Background(), Request{Operation: "host-invoke", Callback: "command"}, func(context.Context, profile.HostCall) (checkpoint.Value, error) {
				calls++
				return checkpoint.Int(1), nil
			})
			if e == nil {
				t.Fatal("malformed callback accepted")
			}
			if name != "repeat" && name != "limit" && calls != 0 {
				t.Fatal("malformed envelope reached Host dispatcher")
			}
			select {
			case <-c.done:
			case <-time.After(time.Second):
				t.Fatal("bad runner not reaped")
			}
		})
	}
}
func TestCallbackFailureCannotBecomeChildPASS(t *testing.T) {
	c := mockClient(t, "deny")
	_, e := c.CallWithHost(context.Background(), Request{Operation: "host-invoke", Callback: "command"}, func(context.Context, profile.HostCall) (checkpoint.Value, error) {
		return checkpoint.Value{}, profile.Fail(profile.ErrCapability)
	})
	if profile.Code(e) != profile.ErrCapability {
		t.Fatal("child PASS erased Host denial", e)
	}
}

func TestUntrustedOutputAccountingCannotLowerCommandCharge(t *testing.T) {
	for _, name := range []string{"output-negative", "output-excess", "output-missing", "output-unaccounted"} {
		t.Run(name, func(t *testing.T) {
			c := mockClient(t, name)
			_, e := c.CallWithHost(context.Background(), Request{Operation: "host-invoke", Callback: "command"}, func(context.Context, profile.HostCall) (checkpoint.Value, error) { return checkpoint.Int(1), nil })
			if e != ErrProtocol {
				t.Fatal("invalid output metric accepted", e)
			}
			select {
			case <-c.done:
			case <-time.After(time.Second):
				t.Fatal("untrusted output runner not reaped")
			}
		})
	}
}
func TestUnsolicitedCallbackAndCancellationAreJoined(t *testing.T) {
	t.Run("unsolicited", func(t *testing.T) {
		c := mockClient(t, "valid")
		_, e := c.CallWithHost(context.Background(), Request{Operation: "state"}, func(context.Context, profile.HostCall) (checkpoint.Value, error) {
			t.Fatal("unsolicited callback reached dispatcher")
			return checkpoint.Int(0), nil
		})
		if e == nil {
			t.Fatal("callback outside invocation accepted")
		}
	})
	t.Run("cancel", func(t *testing.T) {
		c := mockClient(t, "valid")
		pid := c.PID()
		ctx, cancel := context.WithCancel(context.Background())
		entered := make(chan struct{})
		done := make(chan error, 1)
		go func() {
			_, e := c.CallWithHost(ctx, Request{Operation: "host-invoke", Callback: "command"}, func(ctx context.Context, _ profile.HostCall) (checkpoint.Value, error) {
				close(entered)
				<-ctx.Done()
				return checkpoint.Value{}, ctx.Err()
			})
			done <- e
		}()
		<-entered
		cancel()
		select {
		case e := <-done:
			if e == nil {
				t.Fatal("cancel returned success")
			}
		case <-time.After(time.Second):
			t.Fatal("callback not joined")
		}
		if _, e := os.Stat(fmt.Sprintf("/proc/%d", pid)); !os.IsNotExist(e) {
			t.Fatal("cancel runner not reaped", e)
		}
	})
}
