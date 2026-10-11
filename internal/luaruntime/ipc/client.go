// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package ipc

import (
	"context"
	"encoding/json"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"sync"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
)

type Client struct {
	mu                      sync.Mutex
	cmd                     *exec.Cmd
	input                   io.WriteCloser
	output                  io.ReadCloser
	done                    chan struct{}
	next                    uint64
	closed                  bool
	limits                  profile.Limits
	callbackLimit           int
	stopMu                  sync.Mutex
	stopOnce                sync.Once
	stopDone                chan struct{}
	stopUnknown, stopReaped bool
}

func Start(ctx context.Context, executable string, config profile.Config) (*Client, error) {
	if !filepath.IsAbs(executable) || config.Limits.Validate() != nil {
		return nil, profile.Fail(profile.ErrConfiguration)
	}
	cmd := exec.Command(executable, "serve")
	cmd.Env = []string{"GOMAXPROCS=2", "GOMEMLIMIT=67108864", "TZ=UTC"}
	cmd.Dir = "/"
	cmd.SysProcAttr = processAttributes()
	cmd.Stderr = io.Discard
	readInput, input, err := os.Pipe()
	if err != nil {
		return nil, err
	}
	output, writeOutput, err := os.Pipe()
	if err != nil {
		input.Close()
		readInput.Close()
		return nil, err
	}
	cmd.Stdin = readInput
	cmd.Stdout = writeOutput
	c := &Client{cmd: cmd, input: input, output: output, done: make(chan struct{}), limits: config.Limits}
	if err := cmd.Start(); err != nil {
		input.Close()
		output.Close()
		readInput.Close()
		writeOutput.Close()
		return nil, err
	}
	readInput.Close()
	writeOutput.Close()
	go func() { _ = cmd.Wait(); close(c.done) }()
	if config.Host != nil {
		c.callbackLimit = config.Host.CallbackLimit
	}
	if _, err := c.Call(ctx, Request{Operation: "initialize", Config: &config}); err != nil {
		c.Kill()
		return nil, err
	}
	return c, nil
}

func (c *Client) PID() int { return c.cmd.Process.Pid }

func (c *Client) Call(ctx context.Context, request Request) (Response, error) {
	return c.CallWithHost(ctx, request, nil)
}

// The trusted handler must be bounded and honor ctx. Cancellation closes pipes,
// reaps the child and joins this invocation before returning to its workspace.
func (c *Client) CallWithHost(ctx context.Context, request Request, handler profile.HostHandler) (Response, error) {
	c.mu.Lock()
	defer c.mu.Unlock()
	var response Response
	if c.closed {
		return response, ErrRunner
	}
	if ctx == nil {
		return response, ErrProtocol
	}
	ctx, cancel := context.WithTimeout(ctx, time.Duration(c.limits.WallMillis)*time.Millisecond+time.Second)
	defer cancel()
	c.next++
	request.ID = c.next
	request.Version = Version
	raw, err := json.Marshal(request)
	if err != nil || len(raw) > MaxFrameBytes {
		return response, ErrProtocol
	}
	type outcome struct {
		response Response
		err      error
	}
	finished := make(chan outcome, 1)
	go func() {
		var out Response
		if err := WriteFrame(c.input, raw); err != nil {
			finished <- outcome{err: ErrRunner}
			return
		}
		var callbacks uint64
		var callbackError error
		for {
			raw, err := ReadFrame(c.output)
			if err != nil {
				finished <- outcome{err: ErrRunner}
				return
			}
			var tag struct {
				Kind string `json:"kind"`
			}
			if json.Unmarshal(raw, &tag) != nil {
				finished <- outcome{err: ErrProtocol}
				return
			}
			if tag.Kind != "" {
				var cb Callback
				if checkpoint.StrictDecode(raw, &cb, MaxFrameBytes) != nil || cb.Kind != "callback" || cb.Version != Version || cb.ID != request.ID || cb.Sequence != callbacks+1 || cb.Sequence > uint64(c.callbackLimit) || cb.PID != c.PID() || cb.Profile != profile.ID || cb.Runtime != profile.RuntimeVersion || request.Operation != "host-invoke" || handler == nil {
					finished <- outcome{err: ErrProtocol}
					return
				}
				callbacks++
				if err := ctx.Err(); err != nil {
					finished <- outcome{err: err}
					return
				}
				value, callErr := handler(ctx, cb.Call)
				if callErr == nil {
					callErr = checkpoint.Validate(value)
				}
				if err := ctx.Err(); err != nil {
					finished <- outcome{err: err}
					return
				}
				reply := CallbackReply{Kind: "callback-reply", Version: Version, ID: request.ID, Sequence: callbacks, Value: value}
				if callErr != nil {
					reply.Error = profile.Code(callErr)
					reply.Value = checkpoint.Value{Kind: "nil"}
					if callbackError == nil {
						callbackError = profile.Fail(reply.Error)
					}
				}
				encoded, err := json.Marshal(reply)
				if err != nil || WriteFrame(c.input, encoded) != nil {
					finished <- outcome{err: ErrRunner}
					return
				}
				continue
			}
			if checkpoint.StrictDecode(raw, &out, MaxFrameBytes) != nil || out.ID != request.ID || out.Version != Version || out.Profile != profile.ID || out.Runtime != profile.RuntimeVersion || out.Result.Audit.Level != "AUDIT-0" || out.Result.Audit.Sequence != request.ID || out.PID != c.PID() {
				finished <- outcome{err: ErrProtocol}
				return
			}
			if out.Result.OutputBytes < 0 || out.Result.OutputBytes > c.limits.OutputBytes || (len(out.Result.Output) > 0 && out.Result.OutputBytes < len(out.Result.Output)) || (len(out.Result.Output) == 0 && out.Result.OutputBytes != 0) {
				finished <- outcome{err: ErrProtocol}
				return
			}
			if callbackError != nil {
				finished <- outcome{response: out, err: callbackError}
				return
			}
			break
		}
		finished <- outcome{response: out}
	}()
	select {
	case out := <-finished:
		if out.err != nil {
			c.kill()
			return out.response, out.err
		}
		if out.response.Error != "" {
			return out.response, profile.Fail(out.response.Error)
		}
		return out.response, nil
	case <-ctx.Done():
		c.kill()
		<-finished
		return response, ctx.Err()
	}
}
func (c *Client) kill() {
	if c.closed {
		return
	}
	c.closed = true
	_ = c.input.Close()
	_ = c.output.Close()
	_ = c.cmd.Process.Kill()
	<-c.done
}
func (c *Client) Kill() { c.mu.Lock(); defer c.mu.Unlock(); c.kill() }
