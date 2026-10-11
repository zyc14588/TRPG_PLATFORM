//go:build linux

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package install

import (
	"context"
	"errors"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/ipc"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
)

type validationLauncher struct {
	children []*ipc.Client
	loseACK  bool
}
type validationChild struct {
	*ipc.Client
	loseACK bool
}

func (c *validationChild) StopAndWait(ctx context.Context) (ipc.Exit, error) {
	exit, e := c.Client.StopAndWait(ctx)
	if c.loseACK {
		return ipc.Exit{PID: c.PID()}, ipc.ErrUnknownExit
	}
	return exit, e
}
func (l *validationLauncher) Start(ctx context.Context, path string, config profile.Config) (ipc.Runner, error) {
	c, e := ipc.Start(ctx, path, config)
	if e != nil {
		return nil, e
	}
	l.children = append(l.children, c)
	return &validationChild{c, l.loseACK}, nil
}

func TestInstallationSelectedLauncherUsesActualWaitAndRejectsMissingACK(t *testing.T) {
	runtime := buildRunner(t)
	for _, lost := range []bool{false, true} {
		name := "known"
		if lost {
			name = "lost-ack"
		}
		t.Run(name, func(t *testing.T) {
			launcher := &validationLauncher{loseACK: lost}
			c := runtime
			c.Launcher = launcher
			p := scriptPackage(t, "game-system", "return true")
			a := approval(t, p)
			a.Tests = []Test{{Name: "actual-selected-runner", Source: []byte("return true")}}
			var executions []Execution
			digest, e := validateRuntime(context.Background(), c, []staged{{pkg: p, approval: a}}, func(v Execution) error {
				executions = append(executions, v)
				return nil
			})
			if lost {
				if !errors.Is(e, ipc.ErrUnknownExit) || digest != "" || len(launcher.children) != 1 {
					t.Fatal("unproven quarantine accepted or reused", e)
				}
			} else if e != nil || digest == "" || len(launcher.children) != 2 {
				t.Fatal("actual launcher bypassed by quarantine or VM", e)
			}
			terminal := 0
			for _, v := range executions {
				if v.Case == "ipc-destroy" || v.Case == "vm-destroy" {
					terminal++
					if v.Reaped == lost {
						t.Fatal("parent ACK did not control Reaped evidence")
					}
				}
			}
			if terminal != len(launcher.children) {
				t.Fatal("terminal evidence missing")
			}
			for _, child := range launcher.children {
				assertReaped(t, child.PID())
			}
		})
	}
}
