// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package gateway

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"slices"
	"strings"
	"sync"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/budget"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/certification"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

// ProviderTransport is an optional trusted deployment seam. It moves only the
// already filtered provider request. Task tokens, caller and authority remain
// in the in-process guard, which must run immediately before releasing bytes.
type ProviderTransport interface {
	Send(context.Context, Dispatch) ([]byte, error)
}

type Dispatch struct{ data *dispatchData }
type dispatchData struct {
	mu                  sync.Mutex
	closed              bool
	body, authorization []byte
	guard               func(context.Context) error
	binding             string
	bound               budget.Units
}
type ProviderBytes struct {
	Body, Authorization []byte
	Binding             string
	Bound               budget.Units
}

func (ProviderBytes) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private one-call provider bytes>")
}
func (ProviderBytes) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }

func (Dispatch) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<private provider dispatch>") }
func (Dispatch) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }

// Begin lends an owned copy only after the current platform guard succeeds.
// The broker owns one-attempt delivery; this function grants no task mutation.
func (d Dispatch) Begin(ctx context.Context, use func(ProviderBytes) error) error {
	if d.data == nil || ctx == nil || ctx.Err() != nil || use == nil {
		return auth.ErrDenied
	}
	x := d.data
	x.mu.Lock()
	guard, closed := x.guard, x.closed
	x.mu.Unlock()
	if guard == nil || closed {
		return auth.ErrDenied
	}
	if err := guard(ctx); err != nil {
		return auth.SafeError(err)
	}
	x.mu.Lock()
	if ctx.Err() != nil || x.closed {
		x.mu.Unlock()
		return auth.ErrDenied
	}
	v := ProviderBytes{Body: bytes.Clone(x.body), Authorization: bytes.Clone(x.authorization), Binding: x.binding, Bound: x.bound}
	x.mu.Unlock()
	defer clear(v.Body)
	defer clear(v.Authorization)
	return use(v)
}
func (d Dispatch) close() {
	if d.data == nil {
		return
	}
	x := d.data
	x.mu.Lock()
	defer x.mu.Unlock()
	x.closed = true
	clear(x.body)
	clear(x.authorization)
	x.body = nil
	x.authorization = nil
	x.guard = nil
}
func (d Dispatch) Digest() string {
	if d.data == nil {
		return ""
	}
	x := d.data
	x.mu.Lock()
	defer x.mu.Unlock()
	if x.closed {
		return ""
	}
	return certification.Hash(struct {
		Body    []byte
		Binding string
		Bounds  budget.Units
	}{x.body, x.binding, x.bound})
}
func (d Dispatch) Binding() string {
	if d.data == nil {
		return ""
	}
	d.data.mu.Lock()
	defer d.data.mu.Unlock()
	if d.data.closed {
		return ""
	}
	return d.data.binding
}

// TransportBinding pins endpoint, models, egress policy and resource bounds.
func TransportBinding(o AdapterOptions) string {
	return certification.Hash(struct {
		Endpoint any
		Timeout  int64
		Response int
		Price    uint64
		Active   int
	}{o.Endpoint.StorageValue(), int64(o.Timeout), o.ResponseBytes, o.MicrosPerToken, o.MaxActive})
}

// Egress owns the unchanged adapter's DNS/IP/TLS/redirect/time/concurrency
// restrictions. The worker cannot supply an endpoint, method, path or model.
type Egress struct {
	adapter *Adapter
	binding string
}

func NewEgress(o AdapterOptions) (*Egress, error) {
	if o.Transport != nil {
		return nil, auth.ErrInvalid
	}
	a, err := NewAdapter(o)
	if err != nil {
		return nil, err
	}
	return &Egress{a, TransportBinding(o)}, nil
}
func (e *Egress) Binding() string { return e.binding }
func (e *Egress) Close()          { e.adapter.Close() }
func (e *Egress) Execute(ctx context.Context, v ProviderBytes) ([]byte, error) {
	if e == nil || e.adapter.state() == nil || ctx == nil || ctx.Err() != nil || v.Binding != e.binding || len(v.Body) == 0 || len(v.Body) > MaxRequestBytes || len(v.Authorization) < 1 || len(v.Authorization) > 4096 || bytes.ContainsAny(v.Authorization, "\r\n\x00") || !budget.Valid(v.Bound) || v.Bound.Calls != 1 {
		return nil, auth.ErrDenied
	}
	d := e.adapter.state()
	var request providerRequest
	if checkpoint.StrictDecode(v.Body, &request, MaxRequestBytes) != nil || !slices.Contains(d.endpoint.Models, request.Model) || request.Stream || request.MaxTokens != v.Bound.Tokens || len(request.Messages) != 2 || request.Messages[0].Role != "system" || request.Messages[1].Role != "user" || len(request.Messages[0].Content) > 2048 || len(request.Messages[1].Content) > 64<<10 || uint64(len(request.Messages[0].Content)+len(request.Messages[1].Content)) > v.Bound.ContextBytes {
		return nil, auth.ErrDenied
	}
	// Enforce exact canonical request bytes as emitted by Adapter.Call.
	canonical, err := json.Marshal(request)
	defer clear(canonical)
	if err != nil || !bytes.Equal(canonical, v.Body) {
		return nil, auth.ErrDenied
	}
	select {
	case d.active <- struct{}{}:
	default:
		return nil, auth.ErrUnavailable
	}
	defer func() { <-d.active }()
	r, err := http.NewRequestWithContext(ctx, http.MethodPost, strings.TrimRight(d.endpoint.URL, "/")+"/chat/completions", bytes.NewReader(v.Body))
	if err != nil {
		return nil, auth.ErrInvalid
	}
	r.Header.Set("Content-Type", "application/json")
	r.Header.Set("Accept", "application/json")
	r.Header.Set("Authorization", "Bearer "+string(v.Authorization))
	defer r.Header.Del("Authorization")
	reply, err := d.client.Do(r)
	if err != nil {
		return nil, auth.ErrUnavailable
	}
	defer reply.Body.Close()
	if reply.StatusCode != http.StatusOK || reply.ContentLength > int64(d.responseBytes) || reply.Header.Get("Content-Encoding") != "" {
		return nil, auth.ErrUnavailable
	}
	body, err := io.ReadAll(io.LimitReader(reply.Body, int64(d.responseBytes)+1))
	if err != nil || len(body) > d.responseBytes {
		clear(body)
		return nil, auth.ErrUnavailable
	}
	return body, nil
}
