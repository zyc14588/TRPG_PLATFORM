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
	mu     sync.Mutex
	cmd    *exec.Cmd
	input  io.WriteCloser
	output io.ReadCloser
	done   chan struct{}
	next   uint64
	closed bool
	limits profile.Limits
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
	if _, err := c.Call(ctx, Request{Operation: "initialize", Config: &config}); err != nil {
		c.Kill()
		return nil, err
	}
	return c, nil
}

func (c *Client) PID() int { return c.cmd.Process.Pid }

func (c *Client) Call(ctx context.Context, request Request) (Response, error) {
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
		raw, err := ReadFrame(c.output)
		if err != nil {
			finished <- outcome{err: ErrRunner}
			return
		}
		if err := checkpoint.StrictDecode(raw, &out, MaxFrameBytes); err != nil {
			finished <- outcome{err: ErrProtocol}
			return
		}
		if out.ID != request.ID || out.Version != Version || out.Profile != profile.ID || out.Runtime != profile.RuntimeVersion || out.Result.Audit.Level != "AUDIT-0" || out.Result.Audit.Sequence != request.ID || out.PID != c.PID() {
			finished <- outcome{err: ErrProtocol}
			return
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
