// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package m2

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"net/http"
	"os"
	"sync"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/gateway"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

// Broker owns ephemeral single-call handles. The task runtime, cookie identity,
// current lease, filtered context and all reservations remain in platformd.
type Broker struct {
	mu                     sync.Mutex
	epoch, binding, worker string
	seen                   time.Time
	calls                  map[string]*providerAttempt
	queue                  chan string
	closed                 bool
}
type providerAttempt struct {
	ctx      context.Context
	dispatch gateway.Dispatch
	wire     workerHandle
	state    string
	digest   string
	body     []byte
	done     chan struct{}
	once     sync.Once
}
type workerHandle struct {
	Handle, BrokerEpoch, WorkerEpoch, Binding, Digest string
	Deadline                                          time.Time
}
type workerRequest struct {
	WorkerEpoch, Binding string
	Handle               *workerHandle
	Body                 []byte
	Failed               bool
}
type workerReply struct {
	Epoch, State string
	Handle       *workerHandle
	Payload      *providerPayload
}

// This wire type is confined to the authenticated finite begin response.
// Ordinary gateway diagnostics/JSON cannot export ProviderBytes.
type providerPayload gateway.ProviderBytes

func NewBroker(binding string) (*Broker, error) {
	if binding == "" {
		return nil, ErrConfiguration
	}
	e, err := nonce()
	if err != nil {
		return nil, err
	}
	return &Broker{epoch: e, binding: binding, calls: map[string]*providerAttempt{}, queue: make(chan string, 4)}, nil
}
func (b *Broker) Send(ctx context.Context, d gateway.Dispatch) ([]byte, error) {
	deadline, ok := ctx.Deadline()
	if !ok || ctx.Err() != nil || d.Binding() != b.binding {
		return nil, auth.ErrDenied
	}
	id, e := nonce()
	if e != nil {
		return nil, e
	}
	x := &providerAttempt{ctx: ctx, dispatch: d, state: "queued", done: make(chan struct{})}
	// Digest is computed in platformd from the immutable approved request,
	// never replaced by a worker claim or provider reply.
	x.wire = workerHandle{Handle: id, BrokerEpoch: b.epoch, Binding: b.binding, Digest: d.Digest(), Deadline: deadline.UTC()}
	b.mu.Lock()
	if b.closed || len(b.calls) >= 4096 {
		b.mu.Unlock()
		return nil, ErrPrivate
	}
	b.calls[id] = x
	b.mu.Unlock()
	select {
	case b.queue <- id:
	case <-ctx.Done():
		b.cancel(x)
		return nil, ErrPrivate
	default:
		b.cancel(x)
		return nil, ErrPrivate
	}
	select {
	case <-x.done:
	case <-ctx.Done():
		b.cancel(x)
	}
	b.mu.Lock()
	defer b.mu.Unlock()
	if x.state != "complete" {
		return nil, ErrPrivate
	}
	out := bytes.Clone(x.body)
	clear(x.body)
	x.body = nil
	x.dispatch = gateway.Dispatch{}
	return out, nil
}
func (b *Broker) cancel(x *providerAttempt) {
	b.mu.Lock()
	defer b.mu.Unlock()
	if x.state != "complete" {
		x.state = "unknown"
	}
	x.dispatch = gateway.Dispatch{}
	x.once.Do(func() { close(x.done) })
}
func (b *Broker) Ready() bool {
	b.mu.Lock()
	defer b.mu.Unlock()
	return !b.closed && b.worker != "" && time.Since(b.seen) < 3*time.Second
}
func (b *Broker) Close() {
	b.mu.Lock()
	b.closed = true
	for _, x := range b.calls {
		if x.state != "complete" {
			x.state = "unknown"
		}
		clear(x.body)
		x.dispatch = gateway.Dispatch{}
		x.once.Do(func() { close(x.done) })
	}
	b.mu.Unlock()
}
func (b *Broker) Handler() http.Handler {
	m := http.NewServeMux()
	for _, op := range []string{"claim", "begin", "complete", "cancel", "status"} {
		m.HandleFunc("/"+op, func(w http.ResponseWriter, r *http.Request) { b.handle(op, w, r) })
	}
	return m
}
func (b *Broker) handle(op string, w http.ResponseWriter, r *http.Request) {
	if !privateMethod(w, r) {
		return
	}
	var v workerRequest
	defer func() { clear(v.Body) }()
	if decodeRequest(r.Context(), r.Body, &v, (gateway.MaxResponseBytes*2)+8192) != nil || (len(v.WorkerEpoch) != 48 && !(op == "status" && v.Handle == nil && v.WorkerEpoch == "")) || v.Binding != b.binding {
		privateError(w)
		return
	}
	if op == "status" && v.Handle == nil && v.WorkerEpoch == "" && v.Binding == b.binding {
		b.mu.Lock()
		ready := !b.closed && b.worker != "" && time.Since(b.seen) < 3*time.Second
		b.mu.Unlock()
		if !ready {
			privateError(w)
			return
		}
		privateReply(w, workerReply{Epoch: b.epoch, State: "ready"})
		return
	}
	b.mu.Lock()
	if b.closed {
		b.mu.Unlock()
		privateError(w)
		return
	}
	// A new worker incarnation cannot acquire or replay the previous worker's
	// claimed/begun requests. Those obligations stay unknown and held.
	if b.worker != "" && b.worker != v.WorkerEpoch {
		for _, x := range b.calls {
			if x.wire.WorkerEpoch == b.worker && x.state != "complete" {
				x.state = "unknown"
				x.dispatch = gateway.Dispatch{}
				x.once.Do(func() { close(x.done) })
			}
		}
	}
	b.worker = v.WorkerEpoch
	b.seen = time.Now()
	if op == "status" && v.Handle == nil {
		b.mu.Unlock()
		privateReply(w, workerReply{Epoch: b.epoch, State: "ready"})
		return
	}
	if op == "claim" {
		if v.Handle != nil || len(v.Body) != 0 {
			b.mu.Unlock()
			privateError(w)
			return
		}
		for {
			select {
			case id := <-b.queue:
				x := b.calls[id]
				if x == nil || x.state != "queued" || x.ctx.Err() != nil {
					continue
				}
				x.wire.WorkerEpoch = v.WorkerEpoch
				x.state = "claimed"
				wire := x.wire
				b.mu.Unlock()
				privateReply(w, workerReply{Epoch: b.epoch, State: "claimed", Handle: &wire})
				return
			default:
				b.mu.Unlock()
				privateReply(w, workerReply{Epoch: b.epoch, State: "idle"})
				return
			}
		}
	}
	if v.Handle == nil {
		b.mu.Unlock()
		privateError(w)
		return
	}
	x := b.calls[v.Handle.Handle]
	if x == nil || x.wire != *v.Handle || x.wire.WorkerEpoch != v.WorkerEpoch || x.wire.BrokerEpoch != b.epoch {
		b.mu.Unlock()
		privateError(w)
		return
	}
	if x.ctx.Err() != nil && x.state != "complete" {
		x.state = "unknown"
		x.dispatch = gateway.Dispatch{}
		x.once.Do(func() { close(x.done) })
	}
	if op == "status" {
		state := x.state
		b.mu.Unlock()
		privateReply(w, workerReply{Epoch: b.epoch, State: state})
		return
	}
	if op == "cancel" {
		if x.state != "complete" {
			x.state = "unknown"
			x.dispatch = gateway.Dispatch{}
			x.once.Do(func() { close(x.done) })
		}
		state := x.state
		b.mu.Unlock()
		privateReply(w, workerReply{Epoch: b.epoch, State: state})
		return
	}
	if op == "begin" {
		if x.state != "claimed" {
			state := x.state
			b.mu.Unlock()
			privateReply(w, workerReply{Epoch: b.epoch, State: state})
			return
		}
		x.state = "authorizing"
		d := x.dispatch
		b.mu.Unlock()
		var payload gateway.ProviderBytes
		e := d.Begin(x.ctx, func(v gateway.ProviderBytes) error {
			payload = gateway.ProviderBytes{Body: bytes.Clone(v.Body), Authorization: bytes.Clone(v.Authorization), Binding: v.Binding, Bound: v.Bound}
			return nil
		})
		defer clear(payload.Body)
		defer clear(payload.Authorization)
		b.mu.Lock()
		if e != nil || x.ctx.Err() != nil || x.state != "authorizing" {
			fmt.Fprintln(os.Stderr, "M2 live provider begin rejected")
			x.state = "unknown"
			x.dispatch = gateway.Dispatch{}
			x.once.Do(func() { close(x.done) })
			b.mu.Unlock()
			privateError(w)
			return
		}
		x.state = "begun"
		x.dispatch = gateway.Dispatch{}
		wire := x.wire
		b.mu.Unlock()
		wirePayload := providerPayload(payload)
		privateReply(w, workerReply{Epoch: b.epoch, State: "begun", Handle: &wire, Payload: &wirePayload})
		return
	}
	if op == "complete" {
		hash := sha256.Sum256(append([]byte{boolByte(v.Failed)}, v.Body...))
		digest := hex.EncodeToString(hash[:])
		if x.state == "complete" && x.digest == digest {
			b.mu.Unlock()
			privateReply(w, workerReply{Epoch: b.epoch, State: "complete"})
			return
		}
		if x.state != "begun" || len(v.Body) > gateway.MaxResponseBytes || (!v.Failed && len(v.Body) == 0) {
			b.mu.Unlock()
			privateError(w)
			return
		}
		x.digest = digest
		if v.Failed {
			x.state = "unknown"
		} else {
			x.state = "complete"
			x.body = bytes.Clone(v.Body)
		}
		x.once.Do(func() { close(x.done) })
		state := x.state
		b.mu.Unlock()
		privateReply(w, workerReply{Epoch: b.epoch, State: state})
		return
	}
	b.mu.Unlock()
	privateError(w)
}
func boolByte(b bool) byte {
	if b {
		return 1
	}
	return 0
}

type WorkerClient struct {
	client         *http.Client
	epoch, binding string
}

func NewWorkerClient(c Config) (*WorkerClient, error) {
	tls, e := TLSConfig(c.TLS, "platformd", false)
	if e != nil {
		return nil, e
	}
	client, e := UnixClient(c.WorkerSocket, tls, c.PeerUID)
	if e != nil {
		return nil, e
	}
	epoch, e := nonce()
	if e != nil {
		return nil, e
	}
	return &WorkerClient{client, epoch, gateway.TransportBinding(c.Provider.Adapter())}, nil
}
func (c *WorkerClient) Close() { c.client.CloseIdleConnections() }
func (c *WorkerClient) operation(ctx context.Context, op string, v workerRequest) (workerReply, error) {
	v.WorkerEpoch = c.epoch
	v.Binding = c.binding
	b, e := encode(v)
	if e != nil {
		return workerReply{}, ErrPrivate
	}
	defer clear(b)
	r, e := privateRequest(ctx, c.client, "platformd", "/"+op, bytes.NewReader(b))
	if e != nil {
		return workerReply{}, e
	}
	defer r.Body.Close()
	var reply workerReply
	if decodeRequest(ctx, r.Body, &reply, (gateway.MaxRequestBytes*2)+16384) != nil || reply.Epoch == "" {
		return workerReply{}, ErrPrivate
	}
	return reply, nil
}
func (c *WorkerClient) Ready(ctx context.Context) error {
	r, e := c.operation(ctx, "status", workerRequest{})
	if e != nil || r.State != "ready" {
		return ErrPrivate
	}
	return nil
}
func (c *WorkerClient) Run(ctx context.Context, egress *gateway.Egress) error {
	if egress == nil || egress.Binding() != c.binding {
		return ErrConfiguration
	}
	start, stop := context.WithTimeout(ctx, 30*time.Second)
	defer stop()
	for c.Ready(start) != nil {
		timer := time.NewTimer(50 * time.Millisecond)
		select {
		case <-start.Done():
			timer.Stop()
			return start.Err()
		case <-timer.C:
		}
	}
	slots := make(chan struct{}, 4)
	var wg sync.WaitGroup
	defer wg.Wait()
	t := time.NewTicker(50 * time.Millisecond)
	defer t.Stop()
	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-t.C:
			select {
			case slots <- struct{}{}:
			default:
				continue
			}
			r, e := c.operation(ctx, "claim", workerRequest{})
			if e != nil || r.State != "claimed" || r.Handle == nil {
				<-slots
				continue
			}
			handle := *r.Handle
			if handle.WorkerEpoch != c.epoch || handle.Binding != c.binding || !handle.Deadline.After(time.Now()) {
				<-slots
				continue
			}
			wg.Add(1)
			go func() { defer wg.Done(); defer func() { <-slots }(); c.execute(ctx, handle, egress) }()
		}
	}
}
func (c *WorkerClient) execute(parent context.Context, h workerHandle, egress *gateway.Egress) {
	ctx, cancel := context.WithDeadline(parent, h.Deadline)
	defer cancel()
	r, e := c.operation(ctx, "begin", workerRequest{Handle: &h})
	if e != nil || r.State != "begun" || r.Handle == nil || *r.Handle != h || r.Payload == nil {
		fmt.Fprintln(os.Stderr, "M2 worker begin unavailable")
		return
	}
	v := gateway.ProviderBytes(*r.Payload)
	defer clear(v.Body)
	defer clear(v.Authorization)
	// Never repeat begin or provider I/O, including an unknown acknowledgement.
	done := make(chan struct{})
	watchDone := make(chan struct{})
	defer func() { cancel(); close(done); <-watchDone }()
	go func() {
		defer close(watchDone)
		t := time.NewTicker(50 * time.Millisecond)
		defer t.Stop()
		for {
			select {
			case <-done:
				return
			case <-ctx.Done():
				return
			case <-t.C:
				s, e := c.operation(ctx, "status", workerRequest{Handle: &h})
				if e != nil || s.State != "begun" {
					cancel()
					return
				}
			}
		}
	}()
	body, e := egress.Execute(ctx, v)
	if e != nil {
		fmt.Fprintln(os.Stderr, "M2 fixed egress failed:", auth.SafeError(e).Error())
	}
	defer clear(body)
	finish, stop := context.WithTimeout(context.WithoutCancel(parent), time.Second)
	defer stop()
	_, _ = c.operation(finish, "complete", workerRequest{Handle: &h, Body: body, Failed: e != nil || ctx.Err() != nil})
}

var _ gateway.ProviderTransport = (*Broker)(nil)

func (c *WorkerClient) Probe(ctx context.Context) error {
	b, _ := encode(workerRequest{Binding: c.binding})
	r, e := privateRequest(ctx, c.client, "platformd", "/status", bytes.NewReader(b))
	if e != nil {
		return e
	}
	defer r.Body.Close()
	var v workerReply
	if decodeRequest(ctx, r.Body, &v, 8192) != nil || v.State != "ready" {
		return ErrPrivate
	}
	return nil
}
