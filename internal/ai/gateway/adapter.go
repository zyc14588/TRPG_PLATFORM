// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package gateway is the bounded server-only provider boundary. OpenAI,
// Ollama and llama.cpp servers use their reviewed OpenAI-compatible endpoint.
package gateway

import (
	"bytes"
	"context"
	"crypto/tls"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/netip"
	"net/url"
	"regexp"
	"slices"
	"strings"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/budget"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/credential"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	store "github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

const MaxResponseBytes = 64 << 10
const MaxRequestBytes = 96 << 10

type AdapterOptions struct {
	Endpoint       model.Endpoint
	Timeout        time.Duration
	ResponseBytes  int
	MicrosPerToken uint64
	MaxActive      int
}
type Adapter struct{ data **adapterData }
type adapterData struct {
	endpoint       model.EndpointData
	client         *http.Client
	transport      *http.Transport
	responseBytes  int
	microsPerToken uint64
	active         chan struct{}
}

func (Adapter) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<bounded server provider adapter>")
}
func (Adapter) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (a *Adapter) state() *adapterData {
	if a == nil || a.data == nil {
		return nil
	}
	return *a.data
}

var namePattern = regexp.MustCompile(`^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,127}$`)

func endpointURL(e model.EndpointData) (*url.URL, error) {
	if !store.ValidID(e.ID) || e.Adapter != "openai-compatible" || len(e.URL) > 512 || len(e.Models) < 1 || len(e.Models) > 128 {
		return nil, auth.ErrInvalid
	}
	seen := map[string]bool{}
	for _, m := range e.Models {
		if !namePattern.MatchString(m) || seen[m] {
			return nil, auth.ErrInvalid
		}
		seen[m] = true
	}
	u, err := url.Parse(e.URL)
	if err != nil || u.User != nil || u.Host == "" || u.RawQuery != "" || u.ForceQuery || u.Fragment != "" || u.RawFragment != "" || u.RawPath != "" || u.Opaque != "" || u.String() != e.URL || u.Hostname() != strings.ToLower(u.Hostname()) {
		return nil, auth.ErrInvalid
	}
	if p := u.Port(); p != "" {
		if _, err := net.LookupPort("tcp", p); err != nil {
			return nil, auth.ErrInvalid
		}
	}
	if u.Scheme == "https" {
		return u, nil
	}
	ip, err := netip.ParseAddr(u.Hostname())
	if u.Scheme != "http" || !e.AllowLANHTTP || err != nil || !allowedAddress(ip, true) {
		return nil, auth.ErrDenied
	}
	return u, nil
}
func allowedAddress(ip netip.Addr, lan bool) bool {
	ip = ip.Unmap()
	if !ip.IsValid() || ip.IsUnspecified() || ip.IsMulticast() || ip.IsLinkLocalUnicast() || ip.IsLinkLocalMulticast() {
		return false
	}
	if ip.IsLoopback() || ip.IsPrivate() {
		return lan
	}
	if !ip.IsGlobalUnicast() {
		return false
	}
	// Shared, benchmark and non-routable IPv4 ranges are not provider egress.
	for _, p := range []string{"0.0.0.0/8", "100.64.0.0/10", "192.0.0.0/24", "192.0.2.0/24", "198.18.0.0/15", "198.51.100.0/24", "203.0.113.0/24", "240.0.0.0/4"} {
		if netip.MustParsePrefix(p).Contains(ip) {
			return false
		}
	}
	return true
}
func NewAdapter(o AdapterOptions) (*Adapter, error) {
	e := model.NewEndpoint(o.Endpoint.StorageValue()).StorageValue()
	u, err := endpointURL(e)
	if err != nil || o.Timeout < time.Millisecond || o.Timeout > 2*time.Second || o.ResponseBytes < 256 || o.ResponseBytes > MaxResponseBytes || o.MicrosPerToken == 0 || o.MicrosPerToken > 1_000_000 || o.MaxActive < 1 || o.MaxActive > 4 {
		return nil, auth.ErrInvalid
	}
	host, port := u.Hostname(), u.Port()
	if port == "" {
		port = "443"
		if u.Scheme == "http" {
			port = "80"
		}
	}
	tr := &http.Transport{Proxy: nil, DisableCompression: true, MaxIdleConns: 4, MaxIdleConnsPerHost: 4, MaxConnsPerHost: o.MaxActive, IdleConnTimeout: 5 * time.Second, ResponseHeaderTimeout: o.Timeout, TLSHandshakeTimeout: o.Timeout, TLSClientConfig: &tls.Config{MinVersion: tls.VersionTLS12}, MaxResponseHeaderBytes: 8192}
	tr.DialContext = func(ctx context.Context, network, address string) (net.Conn, error) {
		if network != "tcp" || address != net.JoinHostPort(host, port) {
			return nil, auth.ErrDenied
		}
		ips, e := net.DefaultResolver.LookupNetIP(ctx, "ip", host)
		if e != nil || len(ips) < 1 || len(ips) > 16 {
			return nil, auth.ErrUnavailable
		}
		for _, ip := range ips {
			if !allowedAddress(ip, o.Endpoint.StorageValue().AllowLANHTTP) {
				return nil, auth.ErrDenied
			}
		}
		// Re-resolve on every new connection and dial the checked literal. TLS
		// still authenticates the original hostname; no proxy or DNS rebind.
		for _, ip := range ips {
			c, e := (&net.Dialer{Timeout: o.Timeout}).DialContext(ctx, "tcp", net.JoinHostPort(ip.String(), port))
			if e == nil {
				return c, nil
			}
			if ctx.Err() != nil {
				break
			}
		}
		return nil, auth.ErrUnavailable
	}
	client := &http.Client{Transport: tr, Timeout: o.Timeout, CheckRedirect: func(*http.Request, []*http.Request) error { return auth.ErrDenied }}
	d := &adapterData{endpoint: e, client: client, transport: tr, responseBytes: o.ResponseBytes, microsPerToken: o.MicrosPerToken, active: make(chan struct{}, o.MaxActive)}
	return &Adapter{data: &d}, nil
}
func (a *Adapter) Close() {
	if a.state() != nil {
		a.state().transport.CloseIdleConnections()
	}
}
func (a *Adapter) Price() uint64 {
	if a.state() == nil {
		return 0
	}
	return a.state().microsPerToken
}

type AnswerData struct {
	Text  string
	Usage budget.Units
}
type Answer = auth.Secret[AnswerData]
type message struct {
	Role    string `json:"role"`
	Content string `json:"content"`
}
type providerRequest struct {
	Model     string    `json:"model"`
	Messages  []message `json:"messages"`
	MaxTokens uint64    `json:"max_tokens"`
	Stream    bool      `json:"stream"`
}
type providerResponse struct {
	ID                string `json:"id,omitempty"`
	Object            string `json:"object,omitempty"`
	Created           int64  `json:"created,omitempty"`
	Model             string `json:"model,omitempty"`
	SystemFingerprint string `json:"system_fingerprint,omitempty"`
	Choices           []struct {
		Index   int `json:"index"`
		Message struct {
			Role    string `json:"role"`
			Content string `json:"content"`
		} `json:"message"`
		FinishReason string `json:"finish_reason"`
	} `json:"choices"`
	Usage struct {
		Prompt     uint64 `json:"prompt_tokens"`
		Completion uint64 `json:"completion_tokens"`
		Total      uint64 `json:"total_tokens"`
	} `json:"usage"`
}

func (a *Adapter) Call(ctx context.Context, tuple model.Tuple, key credential.Key, prompt []byte, instruction string, cap budget.Units) (answer Answer, err error) {
	defer func() {
		if recover() != nil {
			answer = Answer{}
			err = auth.ErrUnavailable
		}
	}()
	d := a.state()
	if d == nil || ctx == nil || ctx.Err() != nil || tuple.Endpoint != d.endpoint.URL || tuple.Adapter != d.endpoint.Adapter || !slices.Contains(d.endpoint.Models, tuple.Model) || !budget.Valid(cap) || cap.Calls != 1 || cap.Tokens < 1 || cap.Tokens > 1_000_000 || len(prompt) < 1 || len(prompt) > 64<<10 || len(instruction) > 2048 || uint64(len(prompt)+len(instruction)) > cap.ContextBytes {
		return Answer{}, auth.ErrDenied
	}
	select {
	case d.active <- struct{}{}:
	default:
		return Answer{}, auth.ErrUnavailable
	}
	defer func() { <-d.active }()
	raw, e := json.Marshal(providerRequest{Model: tuple.Model, Messages: []message{{Role: "system", Content: instruction}, {Role: "user", Content: string(prompt)}}, MaxTokens: cap.Tokens, Stream: false})
	if e != nil || len(raw) > MaxRequestBytes {
		return Answer{}, auth.ErrDenied
	}
	defer clear(raw)
	// The trusted transport has exactly one fixed method/path per endpoint.
	request, e := http.NewRequestWithContext(ctx, http.MethodPost, strings.TrimRight(d.endpoint.URL, "/")+"/chat/completions", bytes.NewReader(raw))
	if e != nil {
		return Answer{}, auth.ErrInvalid
	}
	request.Header.Set("Content-Type", "application/json")
	request.Header.Set("Accept", "application/json")
	var value AnswerData
	e = key.Use(func(secret []byte) error {
		request.Header.Set("Authorization", "Bearer "+string(secret))
		defer request.Header.Del("Authorization")
		r, e := d.client.Do(request)
		if e != nil {
			return auth.ErrUnavailable
		}
		defer r.Body.Close()
		if r.StatusCode != http.StatusOK || r.ContentLength > int64(d.responseBytes) || r.Header.Get("Content-Encoding") != "" {
			return auth.ErrUnavailable
		}
		body, e := io.ReadAll(io.LimitReader(r.Body, int64(d.responseBytes)+1))
		defer clear(body)
		if e != nil || len(body) > d.responseBytes {
			return auth.ErrUnavailable
		}
		var response providerResponse
		if checkpoint.StrictDecode(body, &response, d.responseBytes) != nil || len(response.Choices) != 1 || response.Choices[0].Index != 0 || response.Choices[0].Message.Role != "assistant" || response.Choices[0].FinishReason != "stop" || response.Usage.Prompt > cap.Tokens || response.Usage.Completion > cap.Tokens || response.Usage.Total == 0 || response.Usage.Total > cap.Tokens || response.Usage.Prompt+response.Usage.Completion != response.Usage.Total || (response.Model != "" && response.Model != tuple.Model) {
			return auth.ErrUnavailable
		}
		text := response.Choices[0].Message.Content
		if strings.TrimSpace(text) == "" || len(text) > 16<<10 || response.Usage.Total > budget.MaxMetric/d.microsPerToken {
			return auth.ErrUnavailable
		}
		usage := budget.Units{Calls: 1, Tokens: response.Usage.Total, CostMicros: response.Usage.Total * d.microsPerToken, ContextBytes: uint64(len(prompt) + len(instruction))}
		if !budget.Fits(usage, cap) {
			return auth.ErrUnavailable
		}
		value = AnswerData{Text: text, Usage: usage}
		return nil
	})
	if e != nil {
		return Answer{}, auth.SafeError(e)
	}
	return auth.RoomSecret(value), nil
}
