// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package ipc

import (
	"context"
	"errors"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
)

// ErrUnknownExit means the actual process parent has not acknowledged Wait.
// It must never be converted to a successful destroy or replacement.
var ErrUnknownExit = errors.New("runner exit unproven")

type Exit struct {
	PID    int
	Reaped bool
}

// Runner is selected by trusted Go composition, never package or client data.
// PID is relative to the runner parent's namespace, not necessarily our own.
type Runner interface {
	PID() int
	Call(context.Context, Request) (Response, error)
	CallWithHost(context.Context, Request, profile.HostHandler) (Response, error)
	StopAndWait(context.Context) (Exit, error)
	Kill() // retained for local internal callers; remote callers use StopAndWait
}

type Launcher interface {
	Start(context.Context, string, profile.Config) (Runner, error)
}

func Launch(ctx context.Context, launcher Launcher, executable string, config profile.Config) (Runner, error) {
	if launcher == nil {
		return Start(ctx, executable, config)
	}
	return launcher.Start(ctx, executable, config)
}

func (c *Client) StopAndWait(ctx context.Context) (Exit, error) {
	// Kill still joins the original Cmd.Wait; a timed-out caller cannot claim
	// that proof. UNKNOWN remains sticky while actual parent cleanup continues.
	c.stopOnce.Do(func() {
		c.stopDone = make(chan struct{})
		go func() { c.Kill(); close(c.stopDone) }()
	})
	c.stopMu.Lock()
	defer c.stopMu.Unlock()
	if c.stopReaped {
		return Exit{PID: c.PID(), Reaped: true}, nil
	}
	if c.stopUnknown || ctx == nil || ctx.Err() != nil {
		c.stopUnknown = true
		return Exit{PID: c.PID()}, ErrUnknownExit
	}
	select {
	case <-c.stopDone:
		if ctx.Err() != nil {
			c.stopUnknown = true
			return Exit{PID: c.PID()}, ErrUnknownExit
		}
		c.stopReaped = true
		return Exit{PID: c.PID(), Reaped: true}, nil
	case <-ctx.Done():
		c.stopUnknown = true
		return Exit{PID: c.PID()}, ErrUnknownExit
	}
}

var _ Runner = (*Client)(nil)
