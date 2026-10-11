// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package m2

import (
	"context"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/ipc"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
)

func supervisorFixture(t *testing.T) (*SupervisorLauncher, *Supervisor, *Service, context.CancelFunc) {
	t.Helper()
	dir := privateDirectory(t)
	runner := os.Getenv("TRPG_M2_RUNNER")
	if runner == "" {
		runner = filepath.Join(dir, "lua-runner")
		cmd := exec.Command("go", "build", "-trimpath", "-o", runner, "../../../cmd/lua-runner")
		if b, e := cmd.CombinedOutput(); e != nil {
			t.Fatalf("real runner build: %s: %v", b, e)
		}
		if e := os.Chmod(runner, 0555); e != nil {
			t.Fatal(e)
		}
	}
	binary, e := os.ReadFile(runner)
	if e != nil {
		t.Fatal(e)
	}
	files := pkiFixture(t, dir, "platformd", "lua-runner")
	c := Config{DeploymentID: "m2-test-instance", Runner: runner, RunnerHash: object.Hash(binary), Limits: profile.DefaultLimits(), SupervisorSocket: filepath.Join(dir, "runner.sock"), PeerUID: uint32(os.Getuid()), TLS: files["lua-runner"]}
	s, e := NewSupervisor(c)
	if e != nil {
		t.Fatal("supervisor", e, RequireLinux(), VerifyRunner(c.Runner, c.RunnerHash))
	}
	tc, e := TLSConfig(c.TLS, "platformd", true)
	if e != nil {
		t.Fatal("server TLS", e)
	}
	ctx, cancel := context.WithCancel(context.Background())
	service, e := ServeUnix(ctx, c.SupervisorSocket, tc, c.PeerUID, s.Handler())
	if e != nil {
		cancel()
		t.Fatal("UDS", e)
	}
	c.TLS = files["platformd"]
	l, e := NewSupervisorLauncher(c)
	if e != nil {
		cancel()
		t.Fatal("launcher", e)
	}
	t.Cleanup(func() {
		cancel()
		stop, done := context.WithTimeout(context.Background(), time.Second)
		defer done()
		service.Close(stop)
		if e := s.Close(); e != nil {
			t.Error(e)
		}
		l.Close()
	})
	return l, s, service, cancel
}
func goneChild(t *testing.T, id ChildIdentity) {
	t.Helper()
	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		_, start, _, e := childOS(id.PID)
		if e != nil || start != id.StartTime {
			return
		}
		time.Sleep(10 * time.Millisecond)
	}
	t.Fatal("original kernel process was not joined", id.PID)
}
func TestSupervisedOriginalPIDKernelBindingAndActualWait(t *testing.T) {
	l, _, _, _ := supervisorFixture(t)
	ctx := context.Background()
	runner, e := l.Start(ctx, l.config.Runner, profile.Config{Limits: l.config.Limits})
	if e != nil {
		t.Fatal(e)
	}
	r := runner.(*remoteRunner)
	id := r.Identity()
	pp, start, ns, e := childOS(id.PID)
	if e != nil || pp != os.Getpid() || id.ParentPID != pp || start != id.StartTime || ns != id.Namespace {
		t.Fatal("claimed child differs from kernel process", e)
	}
	out, e := r.Call(ctx, ipc.Request{Operation: "execute", Source: []byte("return 40+2")})
	if e != nil || out.PID != id.PID || len(out.Result.Values) != 1 || out.Result.Values[0].Number != "42" {
		t.Fatal("real original runner response failed", e)
	}
	// Lifetime ownership outlives the normal private request write timeout.
	time.Sleep(5100 * time.Millisecond)
	out, e = r.Call(ctx, ipc.Request{Operation: "execute", Source: []byte("return 43")})
	if e != nil || out.PID != id.PID {
		t.Fatal("persistent VM connection was lost", e)
	}
	stop, cancel := context.WithTimeout(ctx, time.Second)
	defer cancel()
	exit, e := r.StopAndWait(stop)
	if e != nil || !exit.Reaped || exit.PID != id.PID {
		t.Fatal("actual parent did not acknowledge Wait", e)
	}
	goneChild(t, id)
}
func TestSupervisedDisconnectAndWrongSequenceStayUnknown(t *testing.T) {
	for _, fault := range []string{"disconnect", "sequence", "pid", "epoch"} {
		t.Run(fault, func(t *testing.T) {
			l, _, _, _ := supervisorFixture(t)
			ctx := context.Background()
			runner, e := l.Start(ctx, l.config.Runner, profile.Config{Limits: l.config.Limits})
			if e != nil {
				t.Fatal(e)
			}
			r := runner.(*remoteRunner)
			id := r.Identity()
			switch fault {
			case "disconnect":
				r.lifeCancel()
				<-r.lifeDone
			case "sequence":
				r.binding.Sequence++
			case "pid":
				r.binding.Identity.PID++
			case "epoch":
				r.binding.Identity.Epoch = "000000000000000000000000000000000000000000000000"
			}
			if _, e := r.Call(ctx, ipc.Request{Operation: "execute", Source: []byte("return 1")}); e == nil {
				t.Fatal("invalid lifetime/binding accepted")
			}
			stop, cancel := context.WithTimeout(ctx, time.Second)
			defer cancel()
			exit, e := r.StopAndWait(stop)
			if !errors.Is(e, ipc.ErrUnknownExit) || (exit.Reaped && e == nil) {
				t.Fatal("unproven exit became success", e)
			}
			goneChild(t, id)
		})
	}
}
