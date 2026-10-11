// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package gateway

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/credential"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

type testTransport struct {
	send func(context.Context, Dispatch) ([]byte, error)
}

func (p testTransport) Send(ctx context.Context, d Dispatch) ([]byte, error) { return p.send(ctx, d) }
func TestPrivateTransportRechecksGuardThenUsesOriginalBoundedEgress(t *testing.T) {
	var calls, guards atomic.Int64
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		if r.URL.Path != "/v1/chat/completions" || r.Header.Get("Authorization") != "Bearer synthetic-private-key" {
			t.Error("provider binding changed")
		}
		w.Write(responseJSON("synthetic result"))
	}))
	defer server.Close()
	endpoint := model.NewEndpoint(model.EndpointData{ID: "owned-local", URL: server.URL + "/v1", Adapter: "openai-compatible", Models: []string{"fixture:small"}, AllowLANHTTP: true})
	options := AdapterOptions{Endpoint: endpoint, Timeout: time.Second, ResponseBytes: 4096, MicrosPerToken: 2, MaxActive: 1}
	egress, e := NewEgress(options)
	if e != nil {
		t.Fatal(e)
	}
	defer egress.Close()
	var saved ProviderBytes
	options.Transport = testTransport{func(ctx context.Context, d Dispatch) (body []byte, err error) {
		if _, e := json.Marshal(d); e == nil {
			t.Fatal("dispatch exported")
		}
		if strings.Contains(fmt.Sprintf("%#v", d), "synthetic-private-key") {
			t.Fatal("dispatch diagnostic leaked")
		}
		err = d.Begin(ctx, func(v ProviderBytes) error {
			if guards.Load() != 1 || !bytes.Contains(v.Body, []byte("filtered-only")) || bytes.Contains(v.Body, v.Authorization) {
				t.Fatal("guard or filtering missing")
			}
			saved = v
			var e error
			body, e = egress.Execute(ctx, v)
			return e
		})
		return
	}}
	a, e := NewAdapter(options)
	if e != nil {
		t.Fatal(e)
	}
	defer a.Close()
	key, e := credential.NewKey([]byte("synthetic-private-key"))
	if e != nil {
		t.Fatal(e)
	}
	defer key.Close()
	tuple := model.Tuple{Endpoint: endpoint.StorageValue().URL, Adapter: "openai-compatible", Model: "fixture:small"}
	result, e := a.call(context.Background(), tuple, key, []byte("filtered-only"), "trusted", capFixture(), func(context.Context) error { guards.Add(1); return nil })
	if e != nil || calls.Load() != 1 || result.StorageValue().Usage.CostMicros != 6 {
		t.Fatal("private egress changed accepted accounting", e)
	}
	if len(bytes.Trim(saved.Body, "\x00")) != 0 || len(bytes.Trim(saved.Authorization, "\x00")) != 0 {
		t.Fatal("lent one-call bytes retained")
	}
	if _, e := json.Marshal(saved); e == nil {
		t.Fatal("one-call credentials exported")
	}
	guards.Store(0)
	if _, e := a.call(context.Background(), tuple, key, []byte("filtered-only"), "trusted", capFixture(), func(context.Context) error { return auth.ErrDenied }); e == nil || calls.Load() != 1 {
		t.Fatal("revocation dispatched")
	}
	if _, e := a.Call(context.Background(), tuple, key, []byte("filtered-only"), "trusted", capFixture()); e == nil || calls.Load() != 1 {
		t.Fatal("transport without trusted guard dispatched")
	}
}
