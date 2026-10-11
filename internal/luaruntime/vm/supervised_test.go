//go:build linux

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package vm

import (
	"context"
	"errors"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/ipc"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"os"
	"testing"
)

// The fixture forwards all original calls and uses real child PIDs/Wait. Only
// the negative case intentionally loses the parent's completed Wait ACK.
type originalLauncher struct {
	children                      []*originalChild
	loseFirstACK, failReplacement bool
}
type originalChild struct {
	*ipc.Client
	loseACK bool
}

func (r *originalChild) StopAndWait(ctx context.Context) (ipc.Exit, error) {
	exit, e := r.Client.StopAndWait(ctx)
	if r.loseACK {
		return ipc.Exit{PID: r.PID()}, ipc.ErrUnknownExit
	}
	return exit, e
}
func (l *originalLauncher) Start(ctx context.Context, path string, c profile.Config) (ipc.Runner, error) {
	if l.failReplacement && len(l.children) > 0 {
		return nil, profile.Fail(profile.ErrConfiguration)
	}
	r, e := ipc.Start(ctx, path, c)
	if e != nil {
		return nil, e
	}
	child := &originalChild{r, l.loseFirstACK && len(l.children) == 0}
	l.children = append(l.children, child)
	return child, nil
}
func kernelGone(t *testing.T, pid int) {
	t.Helper()
	if _, e := os.Stat(fmt.Sprintf("/proc/%d", pid)); !os.IsNotExist(e) {
		t.Fatal("actual owned child remains", pid, e)
	}
}
func TestSelectedLauncherFailedReplacementKeepsOriginalVM(t *testing.T) {
	l := &originalLauncher{failReplacement: true}
	o := options(t, "launcher-failed-replacement", fixture(t, 1, "", `return true`))
	o.Launcher = l
	s, e := New(context.Background(), o)
	if e != nil {
		t.Fatal(e)
	}
	t.Cleanup(func() { _ = s.Destroy() })
	pid := s.PID()
	token := s.Token()
	if e = s.Reconstruct(context.Background(), o.State, nil); e == nil || s.PID() != pid || len(l.children) != 1 {
		t.Fatal("failed replacement replaced original VM")
	}
	if _, e = s.Execute(context.Background(), token, []byte("return 42")); e != nil {
		t.Fatal("original VM no longer usable after failed start", e)
	}
	if e = s.Destroy(); e != nil || !s.Reaped() {
		t.Fatal("original parent Wait missing", e)
	}
	kernelGone(t, pid)
}
func TestSelectedLauncherMissingWaitACKPoisonsReplacement(t *testing.T) {
	l := &originalLauncher{loseFirstACK: true}
	o := options(t, "launcher-unknown-replacement", fixture(t, 1, "", `return true`))
	o.Launcher = l
	s, e := New(context.Background(), o)
	if e != nil {
		t.Fatal(e)
	}
	if e = s.Reconstruct(context.Background(), o.State, nil); !errors.Is(e, ipc.ErrUnknownExit) || len(l.children) != 2 {
		t.Fatal("unproven original accepted as replacement", e)
	}
	for _, child := range l.children {
		kernelGone(t, child.PID())
	}
	if s.Reaped() {
		t.Fatal("missing ACK promoted to Reaped")
	}
	if e = s.Reconstruct(context.Background(), o.State, nil); !errors.Is(e, ipc.ErrUnknownExit) || len(l.children) != 2 {
		t.Fatal("uncertain runner spawned another replacement")
	}
	if _, e = s.Execute(context.Background(), s.Token(), []byte("return 42")); e == nil {
		t.Fatal("uncertain VM executed")
	}
	for i := 0; i < 2; i++ {
		if e = s.Destroy(); !errors.Is(e, ipc.ErrUnknownExit) || s.Reaped() {
			t.Fatal("Destroy promoted UNKNOWN", e)
		}
	}
}
func TestSelectedLauncherAuditFailureWaitsForOriginalChild(t *testing.T) {
	l := &originalLauncher{}
	o := options(t, "launcher-audit-failure", fixture(t, 1, "", `return true`))
	o.Launcher = l
	armed := false
	o.Audit = func(profile.Audit) error {
		if armed {
			return errors.New("test-only audit unavailable")
		}
		return nil
	}
	s, e := New(context.Background(), o)
	if e != nil {
		t.Fatal(e)
	}
	pid := s.PID()
	armed = true
	if _, e = s.Execute(context.Background(), s.Token(), []byte("return 42")); e == nil {
		t.Fatal("failed audit accepted")
	}
	kernelGone(t, pid)
	if !s.Reaped() {
		t.Fatal("actual parent Wait lost on audit cleanup")
	}
	_ = s.Destroy()
	kernelGone(t, pid)
}
