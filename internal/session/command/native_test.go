// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package command

import (
	"context"
	"encoding/json"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
)

func nativeFixture(t *testing.T) (*Authority, NativeSeat, Envelope) {
	t.Helper()
	a, fixture, e := testAuthority(t)
	v := NativeSeat{Binding: fixture.Binding(), Principal: "native-person", Seat: "player", Commands: map[string]func(checkpoint.Value) error{"increment": func(v checkpoint.Value) error {
		if v.Kind != "integer" {
			return ErrEnvelope
		}
		return nil
	}}, Views: ViewPolicy{ViewFields: []string{"counter", "private"}, EventFields: map[string][]string{"change": {"counter"}}}, RecoveryPoint: true, Inputs: func(context.Context, Envelope) (NativeInputs, error) {
		return NativeInputs{Time: 1, Random: []int64{7}}, nil
	}}
	return a, v, e
}
func TestNativeCurrentPermissionIsRevalidatedWithoutCachedGrants(t *testing.T) {
	a, v, e := nativeFixture(t)
	var mu sync.Mutex
	live := true
	resolve := func(context.Context) (NativeSeat, error) {
		mu.Lock()
		defer mu.Unlock()
		if !live {
			return NativeSeat{}, ErrDenied
		}
		return v, nil
	}
	i, err := a.IssueNative(context.Background(), resolve)
	if err != nil {
		t.Fatal("issue native identity")
	}
	if _, err = a.Validate(i, e); err != nil {
		t.Fatal("native command validation")
	}
	mu.Lock()
	v.Views.ViewFields = []string{"counter"}
	v.RecoveryPoint = false
	delete(v.Commands, "increment")
	mu.Unlock()
	p, err := a.Policy(i)
	if err != nil || len(p.ViewFields) != 1 || p.ViewFields[0] != "counter" {
		t.Fatal("policy grant cached")
	}
	if _, err = a.Validate(i, e); err != ErrDenied {
		t.Fatal("removed command remained authorized")
	}
	if a.CheckRecoveryPoint(context.Background(), i) != ErrDenied {
		t.Fatal("removed recovery permission remained authorized")
	}
	mu.Lock()
	live = false
	mu.Unlock()
	if a.Verify(i) != ErrDenied {
		t.Fatal("revoked identity remained authorized")
	}
}
func TestNativeIdentityCannotChangeTenantSeatPrincipalOrIssuer(t *testing.T) {
	for _, name := range []string{"workspace", "session", "graph", "principal", "seat"} {
		t.Run(name, func(t *testing.T) {
			a, v, _ := nativeFixture(t)
			resolve := func(context.Context) (NativeSeat, error) { return v, nil }
			i, e := a.IssueNative(context.Background(), resolve)
			if e != nil {
				t.Fatal("issue")
			}
			switch name {
			case "workspace":
				v.Binding.Workspace = "other"
			case "session":
				v.Binding.Session = "other"
			case "graph":
				v.Binding.GraphHash = "sha256:" + strings.Repeat("b", 64)
			case "principal":
				v.Principal = "other"
			case "seat":
				v.Seat = "other"
			}
			if a.Verify(i) != ErrDenied {
				t.Fatal("identity boundary substituted")
			}
		})
	}
	a, v, _ := nativeFixture(t)
	i, e := a.IssueNative(context.Background(), func(context.Context) (NativeSeat, error) { return v, nil })
	if e != nil {
		t.Fatal("issue")
	}
	other, _, _ := nativeFixture(t)
	if other.Verify(i) != ErrDenied || a.Verify(Identity{}) != ErrDenied {
		t.Fatal("unrelated issuer accepted")
	}
}
func TestNativeReleaseInvalidatesAnInFlightResolverAndReclaimsCapacity(t *testing.T) {
	a, v, _ := nativeFixture(t)
	var blocking atomic.Bool
	entered, release := make(chan struct{}), make(chan struct{})
	resolve := func(ctx context.Context) (NativeSeat, error) {
		if blocking.Load() {
			close(entered)
			select {
			case <-release:
			case <-ctx.Done():
				return NativeSeat{}, ErrDenied
			}
		}
		return v, nil
	}
	i, e := a.IssueNative(context.Background(), resolve)
	if e != nil {
		t.Fatal("issue")
	}
	blocking.Store(true)
	done := make(chan error, 1)
	go func() { done <- a.Verify(i) }()
	<-entered
	if e = a.ReleaseNative(i); e != nil {
		t.Fatal("release deadlocked or failed")
	}
	close(release)
	if <-done != ErrDenied {
		t.Fatal("closed in-flight identity survived")
	}
	fast := func(context.Context) (NativeSeat, error) { return v, nil }
	ids := []Identity{}
	for range 255 {
		id, e := a.IssueNative(context.Background(), fast)
		if e != nil {
			t.Fatal("bounded record allowance changed")
		}
		ids = append(ids, id)
	}
	if _, e = a.IssueNative(context.Background(), fast); e != ErrDenied {
		t.Fatal("unbounded native record map")
	}
	if a.ReleaseNative(ids[0]) != nil {
		t.Fatal("capacity release")
	}
	if _, e = a.IssueNative(context.Background(), fast); e != nil {
		t.Fatal("released capacity not reused")
	}
}
func TestNativeInputsComeFromCurrentTrustedSourceAndAreOwned(t *testing.T) {
	a, v, e := nativeFixture(t)
	random := []int64{7}
	v.Inputs = func(context.Context, Envelope) (NativeInputs, error) {
		return NativeInputs{Time: 1000, Random: random}, nil
	}
	i, err := a.IssueNative(context.Background(), func(context.Context) (NativeSeat, error) { return v, nil })
	if err != nil {
		t.Fatal("issue")
	}
	got, err := a.InputsContext(context.Background(), i, e)
	if err != nil || got.Time != 1000 || got.Random[0] != 7 {
		t.Fatal("trusted source lost")
	}
	got.Random[0] = 8
	if random[0] != 7 {
		t.Fatal("random output aliases policy")
	}
	random[0] = 9
	if got.Random[0] != 8 {
		t.Fatal("policy input aliases result")
	}
	for _, value := range []NativeInputs{{Time: -1}, {Random: []int64{-1}}, {Random: make([]int64, 257)}} {
		v.Inputs = func(context.Context, Envelope) (NativeInputs, error) { return value, nil }
		if _, err = a.InputsContext(context.Background(), i, e); err != ErrDenied {
			t.Fatal("invalid native input accepted")
		}
	}
	v.Inputs = nil
	if _, err = a.InputsContext(context.Background(), i, e); err != ErrDenied {
		t.Fatal("absent trusted source granted command inputs")
	}
}
func TestNativeOpaqueHandlesDoNotFormatPrivateIdentityOrPolicies(t *testing.T) {
	a, v, _ := nativeFixture(t)
	marker := "private_native_marker_0123456789"
	v.Principal = marker
	v.Views.ViewFields = []string{marker}
	i, e := a.IssueNative(context.Background(), func(context.Context) (NativeSeat, error) { return v, nil })
	if e != nil {
		t.Fatal("issue")
	}
	for _, value := range []any{a, *a, i, &i} {
		for _, verb := range []string{"%v", "%+v", "%#v", "%d", "%f", "%x", "%w", "%*v"} {
			if strings.Contains(fmt.Sprintf(verb, value), marker) {
				t.Fatal("native private identity escaped formatting")
			}
		}
		if _, e := json.Marshal(value); e == nil {
			t.Fatal("native identity JSON export permitted")
		}
	}
	if _, e = a.IssueNative(context.Background(), nil); e != ErrDenied {
		t.Fatal("nil resolver accepted")
	}
	if a.VerifyContext(nil, i) != ErrDenied {
		t.Fatal("nil context accepted")
	}
}
