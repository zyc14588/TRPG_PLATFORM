// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package gateway

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"net/netip"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/budget"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/credential"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
)

func responseJSON(text string) []byte {
	r := providerResponse{Model: "fixture:small"}
	r.Choices = append(r.Choices, struct {
		Index   int `json:"index"`
		Message struct {
			Role    string `json:"role"`
			Content string `json:"content"`
		} `json:"message"`
		FinishReason string `json:"finish_reason"`
	}{FinishReason: "stop"})
	r.Choices[0].Message.Role = "assistant"
	r.Choices[0].Message.Content = text
	r.Usage.Prompt = 2
	r.Usage.Completion = 1
	r.Usage.Total = 3
	b, _ := json.Marshal(r)
	return b
}
func adapterFixture(t *testing.T, h http.HandlerFunc) (*Adapter, model.Tuple, credential.Key) {
	t.Helper()
	server := httptest.NewServer(h)
	t.Cleanup(server.Close)
	endpoint := model.NewEndpoint(model.EndpointData{ID: "approved-local", URL: server.URL + "/v1", Adapter: "openai-compatible", Models: []string{"fixture:small"}, AllowLANHTTP: true})
	a, e := NewAdapter(AdapterOptions{Endpoint: endpoint, Timeout: 100 * time.Millisecond, ResponseBytes: 4096, MicrosPerToken: 2, MaxActive: 1})
	if e != nil {
		t.Fatal("approved adapter rejected")
	}
	t.Cleanup(a.Close)
	key, e := credential.NewKey([]byte("synthetic-private-provider-key"))
	if e != nil {
		t.Fatal("synthetic key rejected")
	}
	t.Cleanup(key.Close)
	return a, model.Tuple{Model: "fixture:small", Endpoint: endpoint.StorageValue().URL, Adapter: "openai-compatible"}, key
}
func capFixture() budget.Units {
	return budget.Units{Calls: 1, Tokens: 100, CostMicros: 1000, ContextBytes: 4096, LatencyMillis: 500, LocalComputeMillis: 500, Tools: 1}
}
func TestCompatibleServersUseFixedPathAndSeparateCredentialHeader(t *testing.T) {
	for _, serverKind := range []string{"openai-compatible", "ollama-compatible", "llama-cpp-compatible"} {
		t.Run(serverKind, func(t *testing.T) {
			var calls atomic.Int64
			a, tuple, key := adapterFixture(t, func(w http.ResponseWriter, r *http.Request) {
				calls.Add(1)
				if r.Method != "POST" || r.URL.Path != "/v1/chat/completions" || r.Header.Get("Authorization") != "Bearer synthetic-private-provider-key" {
					t.Error("fixed provider boundary incorrect")
				}
				var request providerRequest
				if json.NewDecoder(r.Body).Decode(&request) != nil || request.Stream || len(request.Messages) != 2 || strings.Contains(request.Messages[1].Content, "synthetic-private-provider-key") {
					t.Error("key entered provider payload or unbounded mode")
				}
				_, _ = w.Write(responseJSON("synthetic narrative"))
			})
			answer, e := a.Call(context.Background(), tuple, key, []byte("already filtered seat context"), "trusted instructions", capFixture())
			if e != nil || answer.StorageValue().Usage.CostMicros != 6 || calls.Load() != 1 {
				t.Fatal("compatible bounded transport failed")
			}
		})
	}
}
func TestRedirectOversizeMalformedAndTimeoutRemainCanonical(t *testing.T) {
	for _, name := range []string{"redirect", "oversize", "malformed", "usage", "timeout", "encoding", "status"} {
		t.Run(name, func(t *testing.T) {
			var redirected atomic.Int64
			destination := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { redirected.Add(1) }))
			defer destination.Close()
			a, tuple, key := adapterFixture(t, func(w http.ResponseWriter, r *http.Request) {
				switch name {
				case "redirect":
					http.Redirect(w, r, destination.URL, 302)
				case "oversize":
					_, _ = w.Write([]byte(strings.Repeat("x", 4097)))
				case "malformed":
					_, _ = w.Write([]byte(`{"choices":[]}`))
				case "usage":
					_, _ = w.Write([]byte(strings.Replace(string(responseJSON("ok")), `"total_tokens":3`, `"total_tokens":999`, 1)))
				case "timeout":
					<-r.Context().Done()
				case "encoding":
					w.Header().Set("Content-Encoding", "gzip")
					_, _ = w.Write(responseJSON("ok"))
				case "status":
					w.WriteHeader(429)
					_, _ = w.Write([]byte("synthetic-private-provider-diagnostic"))
				}
			})
			_, e := a.Call(context.Background(), tuple, key, []byte("filtered"), "trusted", capFixture())
			if e == nil || strings.Contains(e.Error(), "private") || redirected.Load() != 0 {
				t.Fatal("failed provider escaped canonical boundary")
			}
		})
	}
}
func TestServerEndpointAndResolvedAddressRejectMetadataAndImplicitLAN(t *testing.T) {
	for _, url := range []string{"http://127.0.0.1:8081/v1", "http://169.254.169.254/latest", "https://user:private@example.test/v1", "https://example.test/v1?key=private", "https://example.test/v1#fragment"} {
		e := model.EndpointData{ID: "owned-endpoint", URL: url, Adapter: "openai-compatible", Models: []string{"fixture:small"}}
		if _, err := endpointURL(e); err == nil {
			t.Fatal("invalid endpoint admitted")
		}
	}
	for _, raw := range []string{"169.254.169.254", "::", "0.0.0.0", "224.0.0.1", "100.64.0.1", "198.18.0.1"} {
		if allowedAddress(netip.MustParseAddr(raw), true) {
			t.Fatal("non-provider egress address admitted")
		}
	}
	if allowedAddress(netip.MustParseAddr("127.0.0.1"), false) || !allowedAddress(netip.MustParseAddr("127.0.0.1"), true) {
		t.Fatal("explicit LAN boundary incorrect")
	}
}
func TestProviderBackpressureAndOpaqueAdapterDoNotLeakOrDispatchTwice(t *testing.T) {
	entered, release := make(chan struct{}), make(chan struct{})
	var calls atomic.Int64
	a, tuple, key := adapterFixture(t, func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		close(entered)
		<-release
		_, _ = w.Write(responseJSON("ok"))
	})
	done := make(chan error, 1)
	go func() {
		_, e := a.Call(context.Background(), tuple, key, []byte("filtered"), "trusted", capFixture())
		done <- e
	}()
	<-entered
	if _, e := a.Call(context.Background(), tuple, key, []byte("filtered"), "trusted", capFixture()); e == nil {
		t.Error("saturated adapter admitted another call")
	}
	close(release)
	if e := <-done; e != nil || calls.Load() != 1 {
		t.Fatal("bounded adapter dispatch changed")
	}
	for _, format := range []string{"%v", "%#v", "%q", "%.*s"} {
		text := fmt.Sprintf(format, *a)
		if strings.Contains(text, tuple.Endpoint) || strings.Contains(text, "fixture:small") {
			t.Fatal("adapter details in diagnostics")
		}
	}
	if _, e := json.Marshal(a); e == nil {
		t.Fatal("private adapter exported")
	}
}
