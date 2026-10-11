// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package m2

import (
	"bytes"
	"context"
	"crypto/rand"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/ipc"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
)

type ChildIdentity struct {
	Deployment, Epoch, Handle, Namespace, StartTime, RunnerHash, Profile, Runtime, LimitsHash string
	PID, ParentPID                                                                            int
}
type supervised struct {
	mu       sync.Mutex
	client   *ipc.Client
	identity ChildIdentity
	sequence uint64
	exit     ipc.Exit
	closed   bool
}
type Supervisor struct {
	mu       sync.Mutex
	config   Config
	epoch    string
	children map[string]*supervised
	closed   bool
	max      int
	opening  int
}

func nonce() (string, error) {
	var b [24]byte
	if _, e := rand.Read(b[:]); e != nil {
		return "", ErrPrivate
	}
	return hex.EncodeToString(b[:]), nil
}
func childOS(pid int) (parent int, start, namespace string, err error) {
	b, e := os.ReadFile(fmt.Sprintf("/proc/%d/stat", pid))
	if e != nil {
		return 0, "", "", ErrPrivate
	}
	i := strings.LastIndexByte(string(b), ')')
	if i < 0 {
		return 0, "", "", ErrPrivate
	}
	fields := strings.Fields(string(b[i+1:]))
	if len(fields) < 20 {
		return 0, "", "", ErrPrivate
	}
	parent, e = strconv.Atoi(fields[1])
	if e != nil {
		return 0, "", "", ErrPrivate
	}
	start = fields[19]
	namespace, e = os.Readlink(fmt.Sprintf("/proc/%d/ns/pid", pid))
	if e != nil || start == "" {
		return 0, "", "", ErrPrivate
	}
	return parent, start, namespace, nil
}
func NewSupervisor(c Config) (*Supervisor, error) {
	if RequireLinux() != nil || VerifyRunner(c.Runner, c.RunnerHash) != nil || c.Limits.Validate() != nil || c.DeploymentID == "" {
		return nil, ErrConfiguration
	}
	e, e2 := nonce()
	if e2 != nil {
		return nil, e2
	}
	return &Supervisor{config: c, epoch: e, children: map[string]*supervised{}, max: 32}, nil
}

type openRunner struct{ Config profile.Config }
type runnerBinding struct {
	Identity ChildIdentity
	Sequence uint64
}
type runnerCall struct {
	Binding runnerBinding
	Request ipc.Request
}
type runnerFrame struct {
	Kind     string
	Binding  runnerBinding
	Callback uint64
	Call     *profile.HostCall
	Value    *checkpoint.Value
	Response *ipc.Response
	Code     string
}
type runnerStatus struct {
	Binding runnerBinding
	Exit    ipc.Exit
	Closed  bool
}

func (s *Supervisor) Handler() http.Handler {
	m := http.NewServeMux()
	m.HandleFunc("/open", s.open)
	m.HandleFunc("/call", s.call)
	m.HandleFunc("/stop", s.stop)
	m.HandleFunc("/status", s.status)
	m.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		if !privateMethod(w, r) {
			return
		}
		privateReply(w, struct{ Epoch string }{s.epoch})
	})
	return m
}
func (s *Supervisor) open(w http.ResponseWriter, r *http.Request) {
	if !privateMethod(w, r) {
		return
	}
	var v openRunner
	if decodeRequest(r.Context(), r.Body, &v, ipc.MaxFrameBytes) != nil || v.Config.Limits != s.config.Limits {
		privateError(w)
		return
	}
	s.mu.Lock()
	if s.closed || len(s.children)+s.opening >= 4096 {
		s.mu.Unlock()
		privateError(w)
		return
	}
	xs := make([]*supervised, 0, len(s.children))
	for _, x := range s.children {
		xs = append(xs, x)
	}
	active := 0
	for _, x := range xs {
		x.mu.Lock()
		if !x.closed {
			active++
		}
		x.mu.Unlock()
	}
	if active+s.opening >= s.max {
		s.mu.Unlock()
		privateError(w)
		return
	}
	s.opening++
	s.mu.Unlock()
	defer func() { s.mu.Lock(); s.opening--; s.mu.Unlock() }()
	if VerifyRunner(s.config.Runner, s.config.RunnerHash) != nil {
		privateError(w)
		return
	}
	c, e := ipc.Start(r.Context(), s.config.Runner, v.Config)
	if e != nil {
		privateError(w)
		return
	}
	id, e := nonce()
	if e != nil {
		c.Kill()
		privateError(w)
		return
	}
	pp, st, ns, e := childOS(c.PID())
	if e != nil || pp != os.Getpid() {
		c.Kill()
		privateError(w)
		return
	}
	b, _ := encode(s.config.Limits)
	identity := ChildIdentity{Deployment: s.config.DeploymentID, Epoch: s.epoch, Handle: id, Namespace: ns, StartTime: st, RunnerHash: s.config.RunnerHash, Profile: profile.ID, Runtime: profile.RuntimeVersion, LimitsHash: object.Hash(b), PID: c.PID(), ParentPID: os.Getpid()}
	x := &supervised{client: c, identity: identity}
	s.mu.Lock()
	if s.closed {
		s.mu.Unlock()
		c.Kill()
		privateError(w)
		return
	}
	s.children[id] = x
	s.mu.Unlock()
	w.Header().Set("Content-Type", "application/octet-stream")
	w.Header().Set("Cache-Control", "no-store")
	if http.NewResponseController(w).SetWriteDeadline(time.Time{}) != nil {
		c.Kill()
		return
	}
	raw, _ := encode(runnerStatus{Binding: runnerBinding{Identity: identity}})
	if ipc.WriteFrame(w, raw) == nil && http.NewResponseController(w).Flush() == nil {
		// This authenticated connection owns the VM lifetime. Daemon death, loss
		// of the connection or cancellation closes the real child at its parent.
		<-r.Context().Done()
	}
	x.mu.Lock()
	defer x.mu.Unlock()
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()
	exit, err := c.StopAndWait(ctx)
	if err == nil && exit.Reaped {
		x.exit = exit
		x.closed = true
	}
}
func (s *Supervisor) find(b runnerBinding) (*supervised, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	x := s.children[b.Identity.Handle]
	if x == nil || x.identity != b.Identity || b.Identity.Epoch != s.epoch {
		return nil, ErrPrivate
	}
	return x, nil
}
func (s *Supervisor) call(w http.ResponseWriter, r *http.Request) {
	if !privateMethod(w, r) {
		return
	}
	if http.NewResponseController(w).EnableFullDuplex() != nil {
		privateError(w)
		return
	}
	raw, e := ipc.ReadFrame(r.Body)
	if e != nil {
		privateError(w)
		return
	}
	var v runnerCall
	if checkpoint.StrictDecode(raw, &v, ipc.MaxFrameBytes) != nil || v.Request.Config != nil || v.Request.Operation == "initialize" {
		privateError(w)
		return
	}
	x, e := s.find(v.Binding)
	if e != nil {
		privateError(w)
		return
	}
	x.mu.Lock()
	defer x.mu.Unlock()
	if x.closed || v.Binding.Sequence != x.sequence+1 {
		if !x.closed {
			x.client.Kill()
			x.closed = true
			x.exit = ipc.Exit{PID: x.client.PID(), Reaped: true}
		}
		privateError(w)
		return
	}
	pp, st, ns, e := childOS(x.client.PID())
	if e != nil || pp != x.identity.ParentPID || st != x.identity.StartTime || ns != x.identity.Namespace {
		x.client.Kill()
		x.closed = true
		x.exit = ipc.Exit{PID: x.client.PID(), Reaped: true}
		privateError(w)
		return
	}
	x.sequence++
	binding := runnerBinding{Identity: x.identity, Sequence: x.sequence}
	w.Header().Set("Content-Type", "application/octet-stream")
	w.Header().Set("Cache-Control", "no-store")
	w.WriteHeader(200)
	_ = http.NewResponseController(w).Flush()
	write := func(f runnerFrame) error {
		b, e := encode(f)
		if e != nil {
			return ErrPrivate
		}
		if ipc.WriteFrame(w, b) != nil {
			return ErrPrivate
		}
		return http.NewResponseController(w).Flush()
	}
	var sequence uint64
	callbackDone := make(chan struct{})
	defer close(callbackDone)
	go func() {
		select {
		case <-r.Context().Done():
			_ = http.NewResponseController(w).SetReadDeadline(time.Now())
			_ = r.Body.Close()
		case <-callbackDone:
		}
	}()
	handler := func(ctx context.Context, c profile.HostCall) (checkpoint.Value, error) {
		if deadline, ok := ctx.Deadline(); ok {
			if http.NewResponseController(w).SetReadDeadline(deadline) != nil {
				return checkpoint.Value{}, ErrPrivate
			}
		}
		sequence++
		if e := write(runnerFrame{Kind: "callback", Binding: binding, Callback: sequence, Call: &c}); e != nil {
			return checkpoint.Value{}, e
		}
		b, e := ipc.ReadFrame(r.Body)
		if e != nil {
			return checkpoint.Value{}, ErrPrivate
		}
		var reply runnerFrame
		if checkpoint.StrictDecode(b, &reply, ipc.MaxFrameBytes) != nil || reply.Kind != "reply" || reply.Binding != binding || reply.Callback != sequence || reply.Value == nil || reply.Call != nil || reply.Response != nil || checkpoint.Validate(*reply.Value) != nil || ctx.Err() != nil {
			return checkpoint.Value{}, ipc.ErrProtocol
		}
		if reply.Code != "" {
			return checkpoint.Value{}, profile.Fail(reply.Code)
		}
		return *reply.Value, nil
	}
	var response ipc.Response
	if v.Request.Operation == "host-invoke" {
		response, e = x.client.CallWithHost(r.Context(), v.Request, handler)
	} else {
		response, e = x.client.Call(r.Context(), v.Request)
	}
	if e != nil && (errors.Is(e, ipc.ErrProtocol) || errors.Is(e, ipc.ErrRunner) || r.Context().Err() != nil) {
		x.client.Kill()
		x.closed = true
		x.exit = ipc.Exit{PID: x.client.PID(), Reaped: true}
	}
	code := ""
	if e != nil {
		code = profile.Code(e)
		if errors.Is(e, ipc.ErrProtocol) {
			code = "IPC_PROTOCOL_REJECTED"
		}
		if errors.Is(e, ipc.ErrRunner) {
			code = "RUNNER_FAILED"
		}
	}
	if write(runnerFrame{Kind: "response", Binding: binding, Response: &response, Code: code}) != nil {
		x.client.Kill()
		x.closed = true
		x.exit = ipc.Exit{PID: x.client.PID(), Reaped: true}
	}
}
func (s *Supervisor) stop(w http.ResponseWriter, r *http.Request) {
	if !privateMethod(w, r) {
		return
	}
	var v runnerBinding
	if decodeRequest(r.Context(), r.Body, &v, 8192) != nil {
		privateError(w)
		return
	}
	x, e := s.find(v)
	if e != nil {
		privateError(w)
		return
	}
	x.mu.Lock()
	defer x.mu.Unlock()
	if v.Sequence != x.sequence {
		privateError(w)
		return
	}
	if !x.closed {
		ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
		x.exit, e = x.client.StopAndWait(ctx)
		cancel()
		if e != nil || !x.exit.Reaped {
			privateError(w)
			return
		}
		x.closed = true
	}
	privateReply(w, runnerStatus{Binding: runnerBinding{Identity: x.identity, Sequence: x.sequence}, Exit: x.exit, Closed: x.closed})
}
func (s *Supervisor) status(w http.ResponseWriter, r *http.Request) {
	if !privateMethod(w, r) {
		return
	}
	var v runnerBinding
	if decodeRequest(r.Context(), r.Body, &v, 8192) != nil {
		privateError(w)
		return
	}
	x, e := s.find(v)
	if e != nil {
		privateError(w)
		return
	}
	x.mu.Lock()
	defer x.mu.Unlock()
	if v.Sequence != x.sequence {
		privateError(w)
		return
	}
	privateReply(w, runnerStatus{Binding: runnerBinding{Identity: x.identity, Sequence: x.sequence}, Exit: x.exit, Closed: x.closed})
}
func (s *Supervisor) Close() error {
	s.mu.Lock()
	s.closed = true
	xs := make([]*supervised, 0, len(s.children))
	for _, x := range s.children {
		xs = append(xs, x)
	}
	s.mu.Unlock()
	var result error
	for _, x := range xs {
		x.mu.Lock()
		ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
		exit, e := x.client.StopAndWait(ctx)
		cancel()
		x.exit = exit
		x.closed = true
		if e != nil || !exit.Reaped {
			result = errors.Join(result, ipc.ErrUnknownExit, e)
		}
		x.mu.Unlock()
	}
	return result
}

type SupervisorLauncher struct {
	client *http.Client
	life   *http.Client
	config Config
}

func NewSupervisorLauncher(c Config) (*SupervisorLauncher, error) {
	tls, e := TLSConfig(c.TLS, "lua-runner", false)
	if e != nil {
		return nil, e
	}
	client, e := UnixClient(c.SupervisorSocket, tls, c.PeerUID)
	if e != nil {
		return nil, e
	}
	life := *client
	life.Timeout = 0
	transport := client.Transport.(*http.Transport).Clone()
	transport.MaxConnsPerHost = 32
	life.Transport = transport
	return &SupervisorLauncher{client: client, life: &life, config: c}, nil
}
func (l *SupervisorLauncher) Ready(ctx context.Context) error {
	b := bytes.NewReader([]byte("{}"))
	r, e := privateRequest(ctx, l.client, "lua-runner", "/health", b)
	if e != nil {
		return e
	}
	defer r.Body.Close()
	var v struct{ Epoch string }
	if decodeRequest(ctx, r.Body, &v, 8192) != nil || v.Epoch == "" {
		return ErrPrivate
	}
	return VerifyRunner(l.config.Runner, l.config.RunnerHash)
}
func (l *SupervisorLauncher) Close() { l.client.CloseIdleConnections(); l.life.CloseIdleConnections() }
func (l *SupervisorLauncher) Start(ctx context.Context, path string, c profile.Config) (ipc.Runner, error) {
	if ctx == nil || ctx.Err() != nil || path != l.config.Runner || c.Limits != l.config.Limits || VerifyRunner(l.config.Runner, l.config.RunnerHash) != nil {
		return nil, ErrConfiguration
	}
	b, e := encode(openRunner{c})
	if e != nil || len(b) > ipc.MaxFrameBytes {
		return nil, ErrPrivate
	}
	defer clear(b)
	life, cancel := context.WithCancel(context.WithoutCancel(ctx))
	timer := time.AfterFunc(3*time.Second, cancel)
	r, e := privateRequest(life, l.life, "lua-runner", "/open", bytes.NewReader(b))
	if e != nil {
		timer.Stop()
		cancel()
		return nil, e
	}
	raw, e := ipc.ReadFrame(r.Body)
	var v runnerStatus
	limits, _ := encode(c.Limits)
	if e != nil || checkpoint.StrictDecode(raw, &v, 8192) != nil || v.Closed || v.Binding.Sequence != 0 || v.Binding.Identity.Deployment != l.config.DeploymentID || v.Binding.Identity.RunnerHash != l.config.RunnerHash || v.Binding.Identity.Profile != profile.ID || v.Binding.Identity.Runtime != profile.RuntimeVersion || v.Binding.Identity.LimitsHash != object.Hash(limits) || v.Binding.Identity.PID <= 0 || v.Binding.Identity.ParentPID <= 0 || v.Binding.Identity.StartTime == "" || !strings.HasPrefix(v.Binding.Identity.Namespace, "pid:[") || len(v.Binding.Identity.Handle) != 48 || len(v.Binding.Identity.Epoch) != 48 {
		timer.Stop()
		cancel()
		r.Body.Close()
		return nil, ipc.ErrUnknownExit
	}
	timer.Stop()
	remote := &remoteRunner{launcher: l, binding: v.Binding, limits: c.Limits, lifeCancel: cancel, lifeBody: r.Body, lifeDone: make(chan struct{})}
	go func() {
		_, _ = io.Copy(io.Discard, r.Body)
		if !remote.intentional.Load() {
			remote.lost.Store(true)
		}
		close(remote.lifeDone)
	}()
	return remote, nil
}

type remoteRunner struct {
	mu                sync.Mutex
	launcher          *SupervisorLauncher
	binding           runnerBinding
	limits            profile.Limits
	closed            bool
	terminal          error
	exit              ipc.Exit
	lifeCancel        context.CancelFunc
	lifeBody          io.ReadCloser
	lifeDone          chan struct{}
	lost, intentional atomic.Bool
}

func (r *remoteRunner) PID() int                { return r.binding.Identity.PID }
func (r *remoteRunner) Identity() ChildIdentity { return r.binding.Identity }
func (r *remoteRunner) Call(ctx context.Context, q ipc.Request) (ipc.Response, error) {
	return r.CallWithHost(ctx, q, nil)
}
func (r *remoteRunner) CallWithHost(ctx context.Context, q ipc.Request, h profile.HostHandler) (ipc.Response, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	var out ipc.Response
	if r.closed || r.terminal != nil || r.lost.Load() || ctx == nil {
		return out, ipc.ErrRunner
	}
	ctx, cancel := context.WithTimeout(ctx, time.Duration(r.limits.WallMillis)*time.Millisecond+time.Second)
	defer cancel()
	r.binding.Sequence++
	binding := r.binding
	in, writer := io.Pipe()
	defer in.Close()
	defer writer.Close()
	b, e := encode(runnerCall{binding, q})
	if e != nil {
		return out, ipc.ErrProtocol
	}
	wrote := make(chan error, 1)
	go func() { wrote <- ipc.WriteFrame(writer, b) }()
	req, e := http.NewRequestWithContext(ctx, "POST", "https://lua-runner/call", in)
	if e != nil {
		return out, ipc.ErrProtocol
	}
	req.Header.Set("Content-Type", "application/json")
	reply, e := r.launcher.client.Do(req)
	if e != nil {
		_ = in.CloseWithError(e)
		<-wrote
		r.terminal = ipc.ErrUnknownExit
		r.lifeCancel()
		return out, ipc.ErrRunner
	}
	defer reply.Body.Close()
	if e = <-wrote; e != nil || reply.StatusCode != 200 {
		r.terminal = ipc.ErrUnknownExit
		r.lifeCancel()
		return out, ipc.ErrProtocol
	}
	var sequence uint64
	for {
		b, e := ipc.ReadFrame(reply.Body)
		if e != nil {
			r.terminal = ipc.ErrUnknownExit
			r.lifeCancel()
			return out, ipc.ErrRunner
		}
		var f runnerFrame
		if checkpoint.StrictDecode(b, &f, ipc.MaxFrameBytes) != nil || f.Binding != binding {
			r.terminal = ipc.ErrUnknownExit
			r.lifeCancel()
			return out, ipc.ErrProtocol
		}
		if f.Kind == "response" {
			if f.Response == nil || f.Call != nil || f.Value != nil {
				r.terminal = ipc.ErrUnknownExit
				r.lifeCancel()
				return out, ipc.ErrProtocol
			}
			out = *f.Response
			if out.PID != r.PID() || out.Profile != profile.ID || out.Runtime != profile.RuntimeVersion || out.Version != ipc.Version || out.ID != binding.Sequence+1 {
				r.terminal = ipc.ErrUnknownExit
				r.lifeCancel()
				return out, ipc.ErrProtocol
			}
			if f.Code != "" {
				return out, profile.Fail(f.Code)
			}
			return out, nil
		}
		if f.Kind != "callback" || f.Callback != sequence+1 || f.Call == nil || h == nil || q.Operation != "host-invoke" || f.Response != nil || f.Value != nil {
			r.terminal = ipc.ErrUnknownExit
			r.lifeCancel()
			return out, ipc.ErrProtocol
		}
		sequence++
		value, ce := h(ctx, *f.Call)
		if ce == nil {
			ce = checkpoint.Validate(value)
		}
		code := ""
		if ce != nil {
			code = profile.Code(ce)
			value = checkpoint.Value{Kind: "nil"}
		}
		if ctx.Err() != nil {
			r.terminal = ipc.ErrUnknownExit
			r.lifeCancel()
			return out, ctx.Err()
		}
		b, e = encode(runnerFrame{Kind: "reply", Binding: binding, Callback: sequence, Value: &value, Code: code})
		if e != nil || ipc.WriteFrame(writer, b) != nil {
			r.terminal = ipc.ErrUnknownExit
			r.lifeCancel()
			return out, ipc.ErrRunner
		}
	}
}
func (r *remoteRunner) StopAndWait(ctx context.Context) (ipc.Exit, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.closed {
		return r.exit, r.terminal
	}
	if r.lost.Load() {
		r.terminal = ipc.ErrUnknownExit
	}
	b, _ := encode(r.binding)
	reply, e := privateRequest(ctx, r.launcher.client, "lua-runner", "/stop", bytes.NewReader(b))
	if e != nil {
		r.terminal = ipc.ErrUnknownExit
		r.lifeCancel()
		return ipc.Exit{PID: r.PID()}, r.terminal
	}
	defer reply.Body.Close()
	var v runnerStatus
	if decodeRequest(ctx, reply.Body, &v, 8192) != nil || v.Binding != r.binding || !v.Closed || !v.Exit.Reaped || v.Exit.PID != r.PID() {
		r.terminal = ipc.ErrUnknownExit
		r.lifeCancel()
		return ipc.Exit{PID: r.PID()}, r.terminal
	}
	r.exit = v.Exit
	r.closed = true
	r.intentional.Store(true)
	r.lifeCancel()
	r.lifeBody.Close()
	<-r.lifeDone
	return r.exit, r.terminal
}
func (r *remoteRunner) Kill() {
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()
	_, _ = r.StopAndWait(ctx)
}

var _ ipc.Launcher = (*SupervisorLauncher)(nil)
