// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package m2smoke provides a TEST_ONLY_STANDARD_CARRIER for the actual six-service Linux lifecycle. Its policy
// grants are explicit operator data; its package identity confers no permission.
package m2smoke

import (
	"bytes"
	"context"
	"crypto/sha256"
	"crypto/tls"
	"crypto/x509"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/http/cookiejar"
	"net/url"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"syscall"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/budget"
	aicontext "github.com/zyc14588/TRPG_PLATFORM/internal/ai/context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/deployment/m2"
	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	base "github.com/zyc14588/TRPG_PLATFORM/internal/hostapi/hostapitest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/ipc"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	archivefixture "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/httpapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

const PackageID = base.PackageID
const PrivateValue = "m2-b011-test-private-state-marker"

const aiIncrementSource = `local M={}
local function state() return {counter=host.state.get({"counter"}),secret=host.state.get({"secret"})} end
for _,n in ipairs({"on_session_create","on_session_restore","on_session_start","list_legal_actions","restore_checkpoint","resume_continuation","on_safe_migration_boundary","cleanup"}) do M[n]=function(...) return {} end end
M.project_view=function(input)
 if input.seat_id then
  local view={counter=host.state.get({"counter"}),pending_action="choose-"..input.seat_id}
  if input.seat_id=="gm" then view.secret=host.state.get({"secret"}) end
  return view
 end
 return state()
end
M.create_checkpoint=function() return state() end
M.on_session_end=function() host.event.emit("change",state());return {} end
M.validate_command=function(command) return type(command)=="table" and (command.type=="increment" or command.type=="fail") and type(command.payload)=="table" and math.type(command.payload.delta)=="integer" and command.payload.delta>=1 and command.payload.delta<=1000 end
M.execute_command=function(command)
 local before=host.state.get({"counter"}); local next=before+command.payload.delta
 host.state.put({"counter"},next)
 host.db.put("docs","one",{score=next})
 host.event.emit("change",state())
 local task=host.ai.request({seat_id="ai",selection="selected",mode="proposal"});host.task.continuation(task,{value=next})
 assert(host.time.now()>0);local draw=host.random.next(10);assert(draw>=0 and draw<10)
 assert(host.content.get("content.txt")=="fixture content")
 assert(host.rules.call("integer.compare",before,next)==-1)
 host.log.write("synthetic metadata only")
 if command.type=="fail" then error("synthetic callback failure after intents") end
 return next
end
M.resume_continuation=function(command)
 if not command.payload then return {} end
 assert(command.seat_id=="task-system" and command.type=="resume-continuation")
 local result=command.payload.result
 assert(type(result)=="table")
 if result.status=="paused" then
  if result.mode=="narrative" then assert(type(result.narrative)=="string" and #result.narrative>0) end
  host.event.emit("change",state());return {}
 end
 assert(result.status=="complete")
 if result.mode=="proposal" then
  local a=result.action
  assert(a.type=="increment" and type(a.payload)=="table" and math.type(a.payload.delta)=="integer" and a.payload.delta==1)
  local next=host.state.get({"counter"})+a.payload.delta
  assert(next<=1000000000)
  host.state.put({"counter"},next);host.event.emit("change",state())
  local narrative=host.ai.request({seat_id="ai",selection="selected",mode="narrative"});host.task.continuation(narrative,{value=next})
 elseif result.mode=="narrative" then
  assert(type(result.narrative)=="string");host.event.emit("change",state())
 else error("invalid synthetic model result") end
 return {}
end
return M
`

func aiState(counter int64) checkpoint.Value {
	return checkpoint.Object(map[string]checkpoint.Value{"counter": checkpoint.Int(counter), "secret": checkpoint.Text(PrivateValue)})
}

const RegistrationToken = "M2B011SyntheticRegistrationToken00000000000"
const ProviderKey = "m2-b011-test-only-provider-secret-marker"

type Provider struct {
	server                    *http.Server
	listener                  net.Listener
	calls, replies, cancelled atomic.Uint64
	mode                      atomic.Int32
	entered                   chan struct{}
	release                   chan struct{}
	done                      chan struct{}
	markerMu                  sync.Mutex
	markers                   [][]byte
}

func NewProvider(l net.Listener, config *tls.Config) (*Provider, error) {
	if config == nil || config.MinVersion < tls.VersionTLS13 || len(config.Certificates) != 1 {
		return nil, m2.ErrConfiguration
	}
	p := &Provider{listener: l, entered: make(chan struct{}, 1), release: make(chan struct{}, 1), done: make(chan struct{})}
	p.server = &http.Server{Handler: http.HandlerFunc(p.serve), ReadHeaderTimeout: time.Second, ReadTimeout: 2 * time.Second, WriteTimeout: 3 * time.Second}
	go func() { defer close(p.done); _ = p.server.Serve(tls.NewListener(l, config)) }()
	return p, nil
}
func (p *Provider) URL() string   { return "https://" + p.listener.Addr().String() }
func (p *Provider) Close()        { p.server.Close(); <-p.done }
func (p *Provider) Calls() uint64 { return p.calls.Load() }
func (p *Provider) Metadata() map[string]any {
	return map[string]any{"calls": p.calls.Load(), "replies": p.replies.Load(), "cancelled": p.cancelled.Load(), "scheme": "https", "external_network": false}
}
func (p *Provider) serve(w http.ResponseWriter, r *http.Request) {
	if r.Method != "POST" || r.URL.Path != "/v1/chat/completions" || r.Header.Get("Authorization") != "Bearer "+ProviderKey {
		w.WriteHeader(403)
		return
	}
	var request struct {
		Model     string                           `json:"model"`
		Messages  []struct{ Role, Content string } `json:"messages"`
		MaxTokens uint64                           `json:"max_tokens"`
		Stream    bool                             `json:"stream"`
	}
	raw, e := io.ReadAll(io.LimitReader(r.Body, 96<<10+1))
	defer clear(raw)
	if e != nil || checkpoint.StrictDecode(raw, &request, 96<<10) != nil || request.Model != "fixture:small" || request.Stream || len(request.Messages) != 2 || strings.Contains(request.Messages[1].Content, PrivateValue) || strings.Contains(request.Messages[1].Content, ProviderKey) {
		w.WriteHeader(400)
		return
	}
	var payload aicontext.PayloadData
	if checkpoint.StrictDecode([]byte(request.Messages[1].Content), &payload, 96<<10) != nil || payload.Version < 1 || payload.View.Kind != "table" || payload.View.Table["secret"].Kind != "" {
		w.WriteHeader(400)
		return
	}
	p.markerMu.Lock()
	if len(p.markers) < 16 {
		p.markers = append(p.markers, append([]byte(nil), raw...), []byte(request.Messages[1].Content))
	}
	p.markerMu.Unlock()
	p.calls.Add(1)
	switch p.mode.Load() {
	case 1:
		select {
		case p.entered <- struct{}{}:
		default:
		}
		select {
		case <-r.Context().Done():
			p.cancelled.Add(1)
			return
		case <-p.release:
		}
	case 2:
		w.WriteHeader(503)
		return
	}
	answer := "Only committed public facts from the installed carrier."
	if !strings.Contains(request.Messages[0].Content, "Narrate") {
		b, _ := json.Marshal(struct {
			Type    string           `json:"type"`
			Version uint64           `json:"expected_state_version"`
			Payload checkpoint.Value `json:"payload"`
		}{"increment", payload.Version, checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(1)})})
		answer = string(b)
	}
	response := map[string]any{"model": "fixture:small", "choices": []map[string]any{{"index": 0, "message": map[string]string{"role": "assistant", "content": answer}, "finish_reason": "stop"}}, "usage": map[string]uint64{"prompt_tokens": 2, "completion_tokens": 1, "total_tokens": 3}}
	w.Header().Set("Content-Type", "application/json")
	if json.NewEncoder(w).Encode(response) == nil && http.NewResponseController(w).Flush() == nil {
		p.replies.Add(1)
	}
}

type Browser struct {
	client     *http.Client
	origin     string
	csrf       string
	sequence   atomic.Uint64
	peer       *Browser
	pace       *browserPace
	lastStatus int
	lastCode   string
}

// The real existing authority admits ten operations per identity/action and
// sixty globally in a minute. The harness stays within those unchanged limits;
// it never retries an admitted command or an indeterminate provider call.
type browserPace struct {
	mu        sync.Mutex
	last      time.Time
	snapshots map[*Browser]time.Time
}

func (b *Browser) waitAdmission(ctx context.Context, path string) error {
	p := b.pace
	if p == nil {
		return m2.ErrPrivate
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	at := p.last.Add(1250 * time.Millisecond)
	snapshot := strings.HasSuffix(path, "/session/snapshot")
	if snapshot && p.snapshots[b].Add(6100*time.Millisecond).After(at) {
		at = p.snapshots[b].Add(6100 * time.Millisecond)
	}
	if delay := time.Until(at); delay > 0 {
		timer := time.NewTimer(delay)
		defer timer.Stop()
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-timer.C:
		}
	}
	p.last = time.Now()
	if snapshot {
		p.snapshots[b] = p.last
	}
	return nil
}

func NewBrowser(origin, caFile string) (*Browser, error) {
	ca, e := os.ReadFile(caFile)
	if e != nil {
		return nil, e
	}
	pool := x509.NewCertPool()
	if !pool.AppendCertsFromPEM(ca) {
		return nil, m2.ErrConfiguration
	}
	jar, e := cookiejar.New(nil)
	if e != nil {
		return nil, e
	}
	return &Browser{client: &http.Client{Transport: &http.Transport{Proxy: nil, TLSClientConfig: &tls.Config{RootCAs: pool, MinVersion: tls.VersionTLS13}, DisableCompression: true}, Jar: jar, Timeout: 5 * time.Second, CheckRedirect: func(*http.Request, []*http.Request) error { return m2.ErrPrivate }}, origin: origin, pace: &browserPace{snapshots: map[*Browser]time.Time{}}}, nil
}
func (b *Browser) Close() {
	b.client.CloseIdleConnections()
	if b.peer != nil {
		b.peer.Close()
	}
}
func (b *Browser) request(ctx context.Context, method, path, key string, fields map[string]any) (map[string]any, int, error) {
	if e := b.waitAdmission(ctx, path); e != nil {
		return nil, 0, e
	}
	b.lastStatus, b.lastCode = 0, "TRANSPORT_UNAVAILABLE"
	var raw []byte
	var e error
	if fields != nil {
		v := map[string]any{"schema_version": 1}
		for k, x := range fields {
			v[k] = x
		}
		raw, e = json.Marshal(v)
		if e != nil {
			return nil, 0, e
		}
		defer clear(raw)
	}
	r, e := http.NewRequestWithContext(ctx, method, b.origin+path, bytes.NewReader(raw))
	if e != nil {
		return nil, 0, e
	}
	if fields != nil {
		r.Header.Set("Content-Type", "application/json")
		r.Header.Set("Origin", b.origin)
		r.Header.Set("X-CSRF-Token", b.csrf)
		if key == "" {
			key = fmt.Sprintf("m2-b011-request-%08d", b.sequence.Add(1))
		}
		r.Header.Set("Idempotency-Key", key)
	}
	reply, e := b.client.Do(r)
	if e != nil {
		return nil, 0, m2.ErrPrivate
	}
	defer reply.Body.Close()
	b.lastStatus, b.lastCode = reply.StatusCode, "INVALID_ENVELOPE"
	body, e := io.ReadAll(io.LimitReader(reply.Body, 1<<20+1))
	defer clear(body)
	if e != nil || len(body) > 1<<20 {
		return nil, reply.StatusCode, m2.ErrPrivate
	}
	var envelope map[string]any
	if checkpoint.StrictDecode(body, &envelope, 1<<20) != nil {
		return nil, reply.StatusCode, m2.ErrPrivate
	}
	if reply.StatusCode != 200 {
		code := "safe-error"
		if x, ok := envelope["error"].(map[string]any); ok {
			code, _ = x["code"].(string)
		}
		b.lastCode = "API_REJECTED"
		if len(code) > 0 && len(code) <= 64 && strings.Trim(code, "ABCDEFGHIJKLMNOPQRSTUVWXYZ_0123456789") == "" {
			b.lastCode = code
		}
		return envelope, reply.StatusCode, fmt.Errorf("existing API rejected %s: %s", path, code)
	}
	data, ok := envelope["data"].(map[string]any)
	if !ok {
		return nil, 200, m2.ErrPrivate
	}
	if csrf, ok := data["csrf_token"].(string); ok {
		b.csrf = csrf
	}
	b.lastCode = ""
	return data, 200, nil
}
func (b *Browser) API(ctx context.Context, method, path string, fields map[string]any) (map[string]any, error) {
	d, _, e := b.request(ctx, method, path, "", fields)
	return d, e
}
func (b *Browser) Login(ctx context.Context, login, password string) (string, error) {
	if _, e := b.API(ctx, "GET", "/api/v1/auth/context", nil); e != nil {
		return "", e
	}
	d, e := b.API(ctx, "POST", "/api/v1/auth/login", map[string]any{"login_name": login, "password": password})
	if e != nil {
		return "", e
	}
	principal, ok := d["principal"].(map[string]any)
	if !ok {
		return "", m2.ErrPrivate
	}
	id, _ := principal["account_id"].(string)
	return id, nil
}
func (b *Browser) CreateWorkspace(ctx context.Context, name string) (string, error) {
	d, e := b.API(ctx, "POST", "/api/v1/workspaces", map[string]any{"name": name})
	if e != nil {
		return "", e
	}
	id, _ := d["workspace_id"].(string)
	if id == "" {
		return "", m2.ErrPrivate
	}
	return id, nil
}

type Lobby struct {
	Workspace, Room, Game, Host, Player, Session, HostConnection, PlayerConnection string
	Owner, Participant                                                             *Browser
}

func (l Lobby) Base() string { return "/api/v1/workspaces/" + l.Workspace + "/rooms/" + l.Room }
func (b *Browser) PrepareMixedLobby(ctx context.Context, workspace, configuration, game, grant string) (Lobby, error) {
	l := Lobby{Workspace: workspace, Game: game, Owner: b}
	d, e := b.API(ctx, "POST", "/api/v1/workspaces/"+workspace+"/rooms", map[string]any{"name": "Owned private mixed lobby", "game_id": game})
	if e != nil {
		return l, e
	}
	l.Room, _ = d["room_id"].(string)
	d, e = b.API(ctx, "POST", l.Base()+"/invitations", map[string]any{"approval_required": false, "expires_in_seconds": 3600, "max_uses": 4})
	if e != nil {
		return l, e
	}
	invite, _ := d["invite_token"].(string)
	d, e = b.API(ctx, "POST", "/api/v1/room-admissions", map[string]any{"mode": "account", "invite_token": invite})
	if e != nil {
		return l, e
	}
	l.Host, _ = d["participant_id"].(string)
	if _, e = b.API(ctx, "PUT", l.Base()+"/participants/"+l.Host+"/role", map[string]any{"role": "host", "enabled": true}); e != nil {
		return l, e
	}
	other := b.peer
	if other == nil {
		ca := b.client.Transport.(*http.Transport).TLSClientConfig.Clone()
		jar, err := cookiejar.New(nil)
		if err != nil {
			return l, err
		}
		other = &Browser{client: &http.Client{Transport: &http.Transport{Proxy: nil, TLSClientConfig: ca}, Jar: jar, Timeout: 5 * time.Second, CheckRedirect: func(*http.Request, []*http.Request) error { return m2.ErrPrivate }}, origin: b.origin, pace: b.pace}
		if _, e = other.API(ctx, "GET", "/api/v1/auth/context", nil); e != nil {
			return l, e
		}
		d, e = other.API(ctx, "POST", "/api/v1/auth/register", map[string]any{"login_name": "m2_player", "password": "m2-player-synthetic-password", "display_name": "Owned participant", "registration_token": grant})
		if e != nil {
			return l, e
		}
		b.peer = other
	} else {
		d, e = other.API(ctx, "GET", "/api/v1/auth/context", nil)
		if e != nil {
			return l, e
		}
	}
	l.Participant = other
	principal, ok := d["principal"].(map[string]any)
	if !ok {
		return l, m2.ErrPrivate
	}
	account, _ := principal["account_id"].(string)
	if _, e = b.API(ctx, "PUT", "/api/v1/workspaces/"+workspace+"/members/"+account, map[string]any{"role": "member"}); e != nil {
		return l, e
	}
	d, e = other.API(ctx, "POST", "/api/v1/room-admissions", map[string]any{"mode": "account", "invite_token": invite})
	if e != nil {
		return l, e
	}
	l.Player, _ = d["participant_id"].(string)
	if l.Room == "" || l.Host == "" || l.Player == "" {
		return l, m2.ErrPrivate
	}
	_, e = b.API(ctx, "POST", l.Base()+"/preparation", map[string]any{"configuration_id": configuration, "slots": []map[string]any{{"id": "gm", "mode": "human", "participant_id": l.Host}, {"id": "player", "mode": "human", "participant_id": l.Player}, {"id": "ai", "mode": "ai", "model_selection": "selected"}}})
	return l, e
}
func VerifyTopology(raw []byte, source, image string) (any, error) {
	var c struct {
		Services map[string]struct {
			Image       string
			User        string
			Init        bool
			ReadOnly    bool `json:"read_only"`
			Privileged  bool
			Cpus        float64
			MemLimit    json.Number `json:"mem_limit"`
			PidsLimit   int64       `json:"pids_limit"`
			CapDropJSON []string    `json:"cap_drop"`
			CapAddJSON  []string    `json:"cap_add"`
			SecurityOpt []string    `json:"security_opt"`
			Ports       []struct {
				HostIP    string `json:"host_ip"`
				Published json.Number
				Target    int
				Protocol  string
			}
			NetworkMode string `json:"network_mode"`
			Networks    map[string]json.RawMessage
			Entrypoint  []string
			Volumes     []struct {
				Target   string
				Type     string
				ReadOnly bool `json:"read_only"`
			}
			Labels map[string]string
		}
		Networks map[string]struct {
			External bool
			Name     string
		}
	}
	if json.Unmarshal(raw, &c) != nil || len(c.Services) != 6 || len(c.Networks) != 3 {
		return nil, m2.ErrConfiguration
	}
	for _, name := range []string{"reverse-proxy", "platformd", "workerd", "lua-runner", "postgres", "object-storage"} {
		s, ok := c.Services[name]
		if !ok || s.NetworkMode != "" || s.Labels["trpg.m2.source"] != source || name != "postgres" && s.Image != image || name != "reverse-proxy" && len(s.Ports) != 0 {
			return nil, m2.ErrConfiguration
		}

		u := strings.Split(s.User, ":")
		uid, uidErr := strconv.Atoi(u[0])
		gid := -1
		var gidErr error
		if len(u) == 2 {
			gid, gidErr = strconv.Atoi(u[1])
		}
		memory := int64(512 << 20)
		if name == "lua-runner" {
			memory = 768 << 20
		}
		if name == "postgres" {
			memory = 1 << 30
		}
		actualMemory, memoryErr := s.MemLimit.Int64()
		if len(u) != 2 || uidErr != nil || gidErr != nil || uid <= 0 || gid <= 0 || !s.Init || s.Privileged || s.ReadOnly != (name != "postgres") || s.Cpus != 1 || memoryErr != nil || actualMemory != memory || s.PidsLimit != 128 || len(s.CapAddJSON) != 0 || len(s.CapDropJSON) != 1 || s.CapDropJSON[0] != "ALL" || len(s.SecurityOpt) != 1 || s.SecurityOpt[0] != "no-new-privileges:true" {
			return nil, m2.ErrConfiguration
		}
		expectedNetworks := []string{"private"}
		if name == "reverse-proxy" {
			expectedNetworks = append(expectedNetworks, "edge")
		}
		if name == "workerd" {
			expectedNetworks = append(expectedNetworks, "provider")
		}
		if len(s.Networks) != len(expectedNetworks) {
			return nil, m2.ErrConfiguration
		}
		for _, network := range expectedNetworks {
			if _, ok := s.Networks[network]; !ok || !c.Networks[network].External || c.Networks[network].Name == "" {
				return nil, m2.ErrConfiguration
			}
		}
		entrypoints := map[string]string{"reverse-proxy": "/app/reverse-proxyd", "platformd": "/app/platformd", "workerd": "/app/workerd", "lua-runner": "/app/lua-supervisord", "object-storage": "/app/objectd"}
		if name != "postgres" && (len(s.Entrypoint) != 1 || s.Entrypoint[0] != entrypoints[name]) {
			return nil, m2.ErrConfiguration
		}
		if name == "postgres" && s.Image != "sha256:3a82e1f56c8f0f5616a11103ac3d47e632c3938698946a7ad26da0df1334744a" {
			return nil, m2.ErrConfiguration
		}
		operator := false
		for _, v := range s.Volumes {
			if v.Target == "/run/operator" {
				operator = v.ReadOnly && v.Type == "bind"
			}
			if !v.ReadOnly && !allowedM2WriteMount(name, v.Target) {
				return nil, m2.ErrConfiguration
			}
			if strings.Contains(v.Target, "docker.sock") || v.Target == "/var/lib/trpg/objects" && name != "object-storage" {
				return nil, m2.ErrConfiguration
			}
		}
		if !operator {
			return nil, m2.ErrConfiguration
		}
	}
	if len(c.Services["reverse-proxy"].Ports) != 1 {
		return nil, m2.ErrConfiguration
	}
	port := c.Services["reverse-proxy"].Ports[0]
	published, e := port.Published.Int64()
	if e != nil || published < 1 || published > 65535 || port.HostIP != "127.0.0.1" || port.Target != 8443 || port.Protocol != "tcp" {
		return nil, m2.ErrConfiguration
	}
	return map[string]any{"six_real_services": true, "numeric_nonroot_uid_readonly_caps_and_limits": true, "only_proxy_published": true, "source": source, "fixed_image": image}, nil
}
func ServiceInventory(raw []byte) (any, bool, error) {
	type service struct {
		ID, Name, Service, State, Health string
		Publishers                       []struct {
			URL                       string
			PublishedPort, TargetPort int
			Protocol                  string
		}
	}
	inventory := []service{}
	if len(bytes.TrimSpace(raw)) == 0 {
		return inventory, false, nil
	}
	if bytes.TrimSpace(raw)[0] == '[' {
		if json.Unmarshal(raw, &inventory) != nil {
			return nil, false, m2.ErrPrivate
		}
	} else {
		decoder := json.NewDecoder(bytes.NewReader(raw))
		for decoder.More() {
			var v service
			if decoder.Decode(&v) != nil {
				return nil, false, m2.ErrPrivate
			}
			inventory = append(inventory, v)
		}
	}
	if len(inventory) != 6 {
		return inventory, false, nil
	}
	ready := true
	proxyPublished := false
	seen := map[string]bool{}
	ids := map[string]bool{}
	for _, v := range inventory {
		if seen[v.Service] || !knownM2Service(v.Service) || v.ID == "" || ids[v.ID] || v.Name == "" {
			return nil, false, m2.ErrPrivate
		}
		seen[v.Service] = true
		ids[v.ID] = true
		if v.State != "running" || v.Health != "healthy" {
			ready = false
		}
		for _, p := range v.Publishers {
			if p.PublishedPort != 0 && (v.Service != "reverse-proxy" || p.URL != "127.0.0.1") {
				return nil, false, m2.ErrPrivate
			}
			if v.Service == "reverse-proxy" && p.PublishedPort > 0 {
				if p.TargetPort != 8443 || p.Protocol != "tcp" {
					return nil, false, m2.ErrPrivate
				}
				proxyPublished = true
			}
		}
	}
	if !proxyPublished {
		ready = false
	}
	return inventory, ready, nil
}
func RunOwnerAction(ctx context.Context, root, temp, out, project, source string, c m2.Config, plan m2.OperatorPlan, lobby Lobby, compose func(context.Context, ...string) ([]byte, error), healthy func(context.Context) (any, error)) error {
	if lobby.Owner == nil || lobby.Owner.csrf == "" {
		return m2.ErrConfiguration
	}
	dir := filepath.Join(temp, "operator/platformd")
	cookie := ""
	origin, e := url.Parse(lobby.Owner.origin)
	if e != nil {
		return e
	}
	for _, v := range lobby.Owner.client.Jar.Cookies(origin) {
		if v.Name == httpapi.SessionCookie {
			cookie = v.Value
		}
	}
	if cookie == "" {
		return m2.ErrPrivate
	}
	for name, value := range map[string]string{"owner-cookie": cookie, "owner-csrf": lobby.Owner.csrf, "owner-provider-key": ProviderKey} {
		if e = privateFile(filepath.Join(dir, name), []byte(value)); e != nil {
			return e
		}
	}
	// Data contains no endpoint, model, fallback or authority override. The
	// original service resolves 'selected' against the current preparation.
	action := struct {
		DeploymentID, Source                                                                   string
		ExpiresAt                                                                              time.Time
		Scope                                                                                  core.Scope
		SeatID, Selection, CredentialID, CredentialIdempotencyKey, ConfigurationIdempotencyKey string
		ExpectedVersion                                                                        uint64
		Budget                                                                                 model.Limits
		CookieFile, CSRFFile, KeyFile                                                          string
	}{project, source, time.Now().Add(4 * time.Minute), core.Scope{WorkspaceID: lobby.Workspace, RoomID: lobby.Room, GameID: lobby.Game}, "ai", "selected", "owner-ai-key", "m2-owner-credential-" + lobby.Room, "m2-owner-configuration-" + lobby.Room, 0, CarrierLimits(), "/run/operator/owner-cookie", "/run/operator/owner-csrf", "/run/operator/owner-provider-key"}
	raw, e := json.Marshal(action)
	if e != nil {
		return e
	}
	defer clear(raw)
	if e = privateFile(filepath.Join(dir, "owner-action.json"), raw); e != nil {
		return e
	}
	override := filepath.Join(temp, "owner-action.compose.yaml")
	if e = privateFile(override, []byte("services:\n  platformd:\n    command: [m2-serve, --config=/run/operator/config.json, --owner-model-action=/run/operator/owner-action.json]\n")); e != nil {
		return e
	}
	if _, e = compose(ctx, "stop", "--timeout", "8", "reverse-proxy", "workerd", "platformd"); e != nil {
		return e
	}
	if _, e = compose(ctx, "-f", override, "up", "--detach", "platformd", "workerd", "reverse-proxy"); e != nil {
		return e
	}
	if _, e = healthy(ctx); e != nil {
		return e
	}
	committed, e := modelCredentialMetadata(ctx, lobby, compose)
	if e != nil {
		return e
	}
	// Identical explicit replay must use the original current idempotent result.
	if _, e = compose(ctx, "-f", override, "restart", "--timeout", "8", "platformd", "workerd", "reverse-proxy"); e != nil {
		return e
	}
	if _, e = healthy(ctx); e != nil {
		return e
	}
	repeated, e := modelCredentialMetadata(ctx, lobby, compose)
	if e != nil || repeated != committed {
		return m2.ErrPrivate
	}
	if _, e = compose(ctx, "stop", "--timeout", "8", "reverse-proxy", "workerd", "platformd"); e != nil {
		return e
	}
	if _, e = compose(ctx, "up", "--detach", "platformd", "workerd", "reverse-proxy"); e != nil {
		return e
	}
	if _, e = healthy(ctx); e != nil {
		return e
	}
	restarted, e := modelCredentialMetadata(ctx, lobby, compose)
	if e != nil || restarted != committed {
		return m2.ErrPrivate
	}
	return nil
}

func privateFile(path string, b []byte) error {
	if e := os.MkdirAll(filepath.Dir(path), 0700); e != nil {
		return e
	}
	if e := os.Remove(path); e != nil && !os.IsNotExist(e) {
		return e
	}
	return os.WriteFile(path, b, 0400)
}

type checkFailure struct {
	name  string
	cause error
}

func (e *checkFailure) Error() string      { return e.name + ": private check failed" }
func (e *checkFailure) Unwrap() error      { return e.cause }
func (e *checkFailure) CheckStage() string { return e.name }
func safeStage(name string, e error) error {
	if e == nil {
		e = m2.ErrPrivate
	}
	return &checkFailure{name, e}
}
func privateProjection(v any, expected bool) error {
	raw, e := json.Marshal(v)
	if e != nil {
		return e
	}
	defer clear(raw)
	if bytes.Contains(raw, []byte(PrivateValue)) != expected || bytes.Contains(raw, []byte(ProviderKey)) {
		return m2.ErrPrivate
	}
	return nil
}
func (l *Lobby) Snapshot(ctx context.Context, owner bool) (map[string]any, error) {
	b, id := l.Participant, l.PlayerConnection
	if owner {
		b, id = l.Owner, l.HostConnection
	}
	return b.API(ctx, "POST", l.Base()+"/session/snapshot", map[string]any{"connection_id": id, "after_cursor": "0", "limit": 8})
}
func (l *Lobby) Connect(ctx context.Context) error {
	for _, owner := range []bool{true, false} {
		b := l.Participant
		if owner {
			b = l.Owner
		}
		d, e := b.API(ctx, "POST", l.Base()+"/session/connect", map[string]any{"after_cursor": "0"})
		if e != nil {
			return e
		}
		id, _ := d["connection_id"].(string)
		if id == "" {
			return m2.ErrPrivate
		}
		if owner {
			l.HostConnection = id
		} else {
			l.PlayerConnection = id
		}
	}
	return nil
}
func (l *Lobby) Pause(ctx context.Context) error {
	d, e := l.Snapshot(ctx, true)
	if e != nil {
		return e
	}
	ctl, ok := d["control"].(map[string]any)
	if !ok {
		return m2.ErrPrivate
	}
	d, e = l.Participant.API(ctx, "POST", l.Base()+"/session/pause", map[string]any{"expected_control_revision": ctl["revision"]})
	if e != nil || d["paused"] != true {
		return m2.ErrPrivate
	}
	return nil
}
func (l *Lobby) Resume(ctx context.Context) error {
	d, e := l.Snapshot(ctx, true)
	if e != nil {
		return e
	}
	ctl, ok := d["control"].(map[string]any)
	if !ok || ctl["paused"] != true {
		return m2.ErrPrivate
	}
	ctl, e = l.Owner.API(ctx, "POST", l.Base()+"/session/resume", map[string]any{"expected_control_revision": ctl["revision"]})
	if e != nil || ctl["paused"] != true {
		return m2.ErrPrivate
	}
	ctl, e = l.Participant.API(ctx, "POST", l.Base()+"/session/resume", map[string]any{"expected_control_revision": ctl["revision"]})
	if e != nil || ctl["paused"] != false {
		return m2.ErrPrivate
	}
	return nil
}
func (l *Lobby) Command(ctx context.Context, id, version, key string) (map[string]any, error) {
	fields := map[string]any{"connection_id": l.HostConnection, "command_id": id, "expected_state_version": version, "type": "increment", "payload": checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(1)}), "correlation_id": "m2-owned-correlation"}
	d, _, e := l.Owner.request(ctx, "POST", l.Base()+"/session/commands", key, fields)
	return d, e
}
func waitSnapshot(ctx context.Context, l *Lobby, version string) (map[string]any, error) {
	ctx, cancel := context.WithTimeout(ctx, 8*time.Second)
	defer cancel()
	timeout := time.NewTimer(8 * time.Second)
	defer timeout.Stop()
	ticker := time.NewTicker(50 * time.Millisecond)
	defer ticker.Stop()
	for {
		d, status, e := l.Participant.request(ctx, "POST", l.Base()+"/session/snapshot", "", map[string]any{"connection_id": l.PlayerConnection, "after_cursor": "0", "limit": 8})
		if e != nil {
			// ReadPage and the Actor snapshot can straddle an original Post.
			// Retry only the conflict in this response, within the same bound.
			problem, ok := d["error"].(map[string]any)
			if status != http.StatusConflict || !ok || problem["code"] != "CONFLICT" {
				return nil, e
			}
		}
		if e == nil && d["state_version"] == version {
			return d, nil
		}
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-timeout.C:
			return nil, m2.ErrPrivate
		case <-ticker.C:
		}
	}
}

// RunDeployment invokes the unchanged authenticated product API through the
// real TLS proxy. Results contain metadata only, never filtered payloads.
func RunDeployment(ctx context.Context, b *Browser, l *Lobby, p *Provider, plan m2.OperatorPlan) (facts any, result error) {
	stage := "presentation"
	defer func() {
		if result != nil {
			facts = map[string]any{"last_host_status": b.lastStatus, "last_host_code": b.lastCode, "last_player_status": l.Participant.lastStatus, "last_player_code": l.Participant.lastCode}
			result = safeStage(stage, result)
		}
	}()
	before := p.Calls()
	for _, path := range []string{"/api/v1/workspaces/" + l.Workspace + "/games/" + plan.Games[0].Configuration + "/presentation", l.Base() + "/presentation"} {
		d, e := b.API(ctx, "GET", path, nil)
		if e != nil {
			return nil, safeStage("production presentation GET", e)
		}
		if privateProjection(d, false) != nil {
			return nil, m2.ErrPrivate
		}
	}

	stage = "tenant presentation authority"
	if len(plan.Games) != 2 {
		return nil, m2.ErrPrivate
	}
	other := plan.Games[1]
	otherPath := "/api/v1/workspaces/" + other.Workspace + "/games/" + other.Configuration + "/presentation"
	d, e := b.API(ctx, "GET", otherPath, nil)
	if e != nil || privateProjection(d, false) != nil {
		return nil, m2.ErrPrivate
	}
	if _, status, e := l.Participant.request(ctx, "GET", otherPath, "", nil); e == nil || status != 403 || p.Calls() != before {
		return nil, m2.ErrPrivate
	}
	stage = "readiness before personal consent"
	d, e = b.API(ctx, "GET", l.Base()+"/readiness", nil)
	if e != nil {
		return nil, e
	}
	readiness, ok := d["readiness"].(map[string]any)
	if !ok || readiness["ready"] != false || p.Calls() != before {
		return nil, m2.ErrPrivate
	}
	// Each current participant submits only their own consent.
	stage = "personal consent"
	for _, browser := range []*Browser{b, l.Participant} {
		if _, e = browser.API(ctx, "POST", l.Base()+"/consent", map[string]any{"revision": "1", "consent": true, "ready": true, "safety_confirmed": true, "boundaries": []string{}}); e != nil {
			return nil, e
		}
	}
	stage = "readiness after personal consent"
	d, e = b.API(ctx, "GET", l.Base()+"/readiness", nil)
	if e != nil {
		return nil, e
	}
	readiness, ok = d["readiness"].(map[string]any)
	if !ok || readiness["ready"] != true || p.Calls() != before {
		return nil, m2.ErrPrivate
	}
	stage = "installed launch"
	d, e = b.API(ctx, "POST", l.Base()+"/launch", map[string]any{"revision": "1"})
	if e != nil {
		return nil, safeStage("actual installed launch", e)
	}
	l.Session, _ = d["session_id"].(string)
	if l.Session == "" {
		return nil, m2.ErrPrivate
	}
	stage = "session connect"
	if e = l.Connect(ctx); e != nil {
		return nil, e
	}
	stage = "host snapshot read"
	gm, e := l.Snapshot(ctx, true)
	if e != nil {
		return nil, e
	}
	stage = "personal snapshot read"
	personal, e := l.Snapshot(ctx, false)
	if e != nil {
		return nil, e
	}
	stage = "host marker projection"
	if privateProjection(gm, true) != nil {
		return nil, safeStage("host marker projection", nil)
	}
	stage = "personal marker projection"
	if privateProjection(personal, false) != nil {
		return nil, safeStage("personal marker projection", nil)
	}
	stage = "unanimous resume"
	if e = l.Resume(ctx); e != nil {
		return nil, e
	}
	stage = "human source command"
	first, e := l.Command(ctx, "first-once", "1", "m2-exact-source-receipt")
	if e != nil {
		return nil, safeStage("human source command", e)
	}
	if first["state_version"] != "2" || privateProjection(first, false) != nil {
		return nil, m2.ErrPrivate
	}
	stage = "exact lost-result replay"
	replay, e := l.Command(ctx, "first-once", "1", "m2-exact-source-receipt")
	if e != nil || replay["replayed"] != true || replay["state_version"] != "2" {
		return nil, safeStage("exact lost-result replay", e)
	}
	stage = "provider to Actor"
	personal, e = waitSnapshot(ctx, l, "4")
	if e != nil {
		return nil, safeStage("real provider to sole Actor", e)
	}
	if p.Calls() != before+2 || privateProjection(personal, false) != nil {
		return nil, m2.ErrPrivate
	}
	for _, kind := range []string{"public", "personal", "host", "administrator"} {
		stage = kind + " export read"
		browser := b
		if kind == "personal" {
			browser = l.Participant
		}
		d, e = browser.API(ctx, "POST", l.Base()+"/session/export", map[string]any{"kind": kind, "after_cursor": "0", "limit": 8})
		if e != nil {
			return nil, e
		}
		stage = kind + " export projection"
		if privateProjection(d, kind == "host" || kind == "administrator") != nil {
			return nil, m2.ErrPrivate
		}
	}
	if _, status, e := l.Participant.request(ctx, "POST", l.Base()+"/session/export", "", map[string]any{"kind": "host", "after_cursor": "0", "limit": 8}); e == nil || status != 403 {
		return nil, m2.ErrPrivate
	}
	if _, status, e := l.Participant.request(ctx, "POST", l.Base()+"/session/commands", "", map[string]any{"connection_id": l.HostConnection, "command_id": "stolen-connection", "expected_state_version": "4", "type": "increment", "payload": checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(1)}), "correlation_id": "m2-denied"}); e == nil || status != 403 {
		return nil, m2.ErrPrivate
	}
	stage = "pause fencing"
	if e = l.Pause(ctx); e != nil {
		return nil, e
	}
	if _, e = l.Command(ctx, "paused-no-effect", "4", ""); e == nil {
		return nil, m2.ErrPrivate
	}
	time.Sleep(150 * time.Millisecond)
	if p.Calls() != before+2 {
		return nil, m2.ErrPrivate
	}
	// Cursor pagination must make bounded progress with personal filtering.
	stage = "cursor pagination"
	after := "0"
	for i := 0; i < 8; i++ {
		d, e = l.Participant.API(ctx, "POST", l.Base()+"/session/snapshot", map[string]any{"connection_id": l.PlayerConnection, "after_cursor": after, "limit": 1})
		if e != nil || privateProjection(d, false) != nil {
			return nil, m2.ErrPrivate
		}
		next, _ := d["event_cursor"].(string)
		if d["more"] != true {
			break
		}
		if next == after || i == 7 {
			return nil, m2.ErrPrivate
		}
		after = next
	}
	// The proxy does not turn forwarding headers or plaintext into identity.
	stage = "proxy guards"
	request, _ := http.NewRequestWithContext(ctx, "GET", b.origin+"/api/v1/auth/context", nil)
	request.Header.Set("X-Forwarded-Proto", "https")
	response, e := b.client.Do(request)
	if e != nil {
		return nil, e
	}
	response.Body.Close()
	if response.StatusCode != 403 {
		return nil, m2.ErrPrivate
	}
	plain, _ := url.Parse(b.origin)
	plain.Scheme = "http"
	request, _ = http.NewRequestWithContext(ctx, "GET", plain.String()+"/api/v1/auth/context", nil)
	response, e = b.client.Do(request)
	if e == nil {
		response.Body.Close()
		if response.StatusCode == 200 {
			return nil, m2.ErrPrivate
		}
	}
	return map[string]any{"presentation_actual_listener": true, "personal_consent": true, "provider_calls": p.Calls() - before, "synthetic_provider": true, "Actor_version": "4", "exact_receipt_replay": true, "pause_no_dispatch": true, "cursor_export_privacy": true, "forged_forwarding_and_plaintext_rejected": true}, nil
}

type Compose func(context.Context, ...string) ([]byte, error)
type Command func(context.Context, string, []byte, ...string) ([]byte, error)
type kernelChild struct {
	HostPID, HostParent int
	Identity            m2.ChildIdentity
	CgroupHash          string
}

func observeChild(ctx context.Context, project string, id m2.ChildIdentity, compose Compose, command Command) (kernelChild, error) {
	raw, e := compose(ctx, "ps", "--quiet", "lua-runner")
	if e != nil {
		return kernelChild{}, e
	}
	container := strings.TrimSpace(string(raw))
	if len(container) != 64 {
		return kernelChild{}, m2.ErrPrivate
	}
	identity, e := command(ctx, "kernel-container-binding", nil, "docker", "inspect", "--format", `{"id":{{json .Id}},"project":{{json (index .Config.Labels "trpg.m2.project")}},"init_pid":{{.State.Pid}}}`, container)
	var owner struct {
		ID, Project string
		InitPID     int `json:"init_pid"`
	}
	if e != nil || checkpoint.StrictDecode(identity, &owner, 1024) != nil || owner.ID != container || owner.Project != project || owner.InitPID < 1 {
		return kernelChild{}, m2.ErrPrivate
	}
	raw, e = command(ctx, "kernel-docker-top", nil, "docker", "top", container, "-eo", "pid,ppid,args")
	if e != nil {
		return kernelChild{}, e
	}
	for _, line := range strings.Split(string(raw), "\n") {
		f := strings.Fields(line)
		if len(f) < 3 || f[2] != "/app/lua-runner" {
			continue
		}
		pid, e := strconv.Atoi(f[0])
		if e != nil {
			continue
		}
		parent, e := strconv.Atoi(f[1])
		if e != nil {
			continue
		}
		status, e := os.ReadFile(fmt.Sprintf("/proc/%d/status", pid))
		if e != nil {
			continue
		}
		nspid := 0
		for _, line := range strings.Split(string(status), "\n") {
			if strings.HasPrefix(line, "NSpid:") {
				n := strings.Fields(line)
				nspid, _ = strconv.Atoi(n[len(n)-1])
			}
		}
		if nspid != id.PID {
			continue
		}
		ns, e := os.Readlink(fmt.Sprintf("/proc/%d/ns/pid", pid))
		if e != nil || ns != id.Namespace {
			return kernelChild{}, m2.ErrPrivate
		}
		stat, e := os.ReadFile(fmt.Sprintf("/proc/%d/stat", pid))
		if e != nil {
			return kernelChild{}, e
		}
		i := strings.LastIndexByte(string(stat), ')')
		if i < 0 {
			return kernelChild{}, m2.ErrPrivate
		}
		fields := strings.Fields(string(stat[i+1:]))
		if len(fields) < 20 || fields[19] != id.StartTime {
			return kernelChild{}, m2.ErrPrivate
		}
		bin, e := os.ReadFile(fmt.Sprintf("/proc/%d/exe", pid))
		if e != nil || object.Hash(bin) != id.RunnerHash {
			return kernelChild{}, m2.ErrPrivate
		}
		parentStatus, e := os.ReadFile(fmt.Sprintf("/proc/%d/status", parent))
		if e != nil {
			return kernelChild{}, e
		}
		parentNSPID := 0
		for _, line := range strings.Split(string(parentStatus), "\n") {
			if strings.HasPrefix(line, "NSpid:") {
				n := strings.Fields(line)
				parentNSPID, _ = strconv.Atoi(n[len(n)-1])
			}
		}
		if parentNSPID != id.ParentPID {
			return kernelChild{}, m2.ErrPrivate
		}
		ownNS, e := os.Readlink("/proc/self/ns/pid")
		if e != nil || ownNS == ns {
			return kernelChild{}, m2.ErrPrivate
		}
		parentNS, e := os.Readlink(fmt.Sprintf("/proc/%d/ns/pid", parent))
		if e != nil || parentNS != ns {
			return kernelChild{}, m2.ErrPrivate
		}
		childGroup, e := os.ReadFile(fmt.Sprintf("/proc/%d/cgroup", pid))
		parentGroup, parentError := os.ReadFile(fmt.Sprintf("/proc/%d/cgroup", parent))
		initGroup, initError := os.ReadFile(fmt.Sprintf("/proc/%d/cgroup", owner.InitPID))
		ownGroup, ownError := os.ReadFile("/proc/self/cgroup")
		if e != nil || parentError != nil || initError != nil || ownError != nil || !bytes.Equal(childGroup, parentGroup) || !bytes.Equal(childGroup, initGroup) || bytes.Equal(childGroup, ownGroup) || !bytes.Contains(childGroup, []byte(container)) {
			return kernelChild{}, m2.ErrPrivate
		}
		return kernelChild{HostPID: pid, HostParent: parent, Identity: id, CgroupHash: object.Hash(childGroup)}, nil
	}
	return kernelChild{}, m2.ErrPrivate
}
func childGone(ctx context.Context, k kernelChild) error {
	return kernelBirthGone(ctx, k.HostPID, k.Identity.StartTime)
}
func kernelBirthGone(ctx context.Context, pid int, start string) error {
	timer := time.NewTimer(3 * time.Second)
	defer timer.Stop()
	tick := time.NewTicker(10 * time.Millisecond)
	defer tick.Stop()
	for {
		raw, e := os.ReadFile(fmt.Sprintf("/proc/%d/stat", pid))
		if os.IsNotExist(e) {
			return nil
		}
		if e != nil {
			return e
		}
		i := strings.LastIndexByte(string(raw), ')')
		if i < 0 {
			return m2.ErrPrivate
		}
		fields := strings.Fields(string(raw[i+1:]))
		if len(fields) < 20 {
			return m2.ErrPrivate
		}
		if fields[19] != start {
			return nil
		}
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-timer.C:
			return m2.ErrPrivate
		case <-tick.C:
		}
	}
}
func runnerIdentity(r ipc.Runner) (m2.ChildIdentity, error) {
	v, ok := r.(interface{ Identity() m2.ChildIdentity })
	if !ok {
		return m2.ChildIdentity{}, m2.ErrPrivate
	}
	return v.Identity(), nil
}
func RunKernelFaults(ctx context.Context, c m2.Config, project string, compose Compose, command Command) (result any, failure error) {
	facts := map[string]any{}
	stage := "kernel-launcher"
	defer func() {
		if failure != nil {
			result = facts
			failure = safeStage(stage, failure)
		}
	}()
	launcher, e := m2.NewSupervisorLauncher(c)
	if e != nil {
		return nil, e
	}
	defer launcher.Close()
	start := func(config profile.Config) (ipc.Runner, kernelChild, error) {
		r, e := launcher.Start(ctx, c.Runner, config)
		if e != nil {
			return nil, kernelChild{}, e
		}
		id, e := runnerIdentity(r)
		if e != nil {
			r.Kill()
			return nil, kernelChild{}, e
		}
		k, e := observeChild(ctx, project, id, compose, command)
		if e != nil {
			r.Kill()
			return nil, kernelChild{}, e
		}
		return r, k, nil
	}
	stop := func(r ipc.Runner, k kernelChild, unknown bool) error {
		s, done := context.WithTimeout(ctx, 2*time.Second)
		defer done()
		exit, e := r.StopAndWait(s)
		if unknown {
			if !errors.Is(e, ipc.ErrUnknownExit) {
				return m2.ErrPrivate
			}
		} else if e != nil || !exit.Reaped || exit.PID != k.Identity.PID {
			return m2.ErrPrivate
		}
		return childGone(ctx, k)
	}
	cfg := profile.Config{Limits: c.Limits}
	stage = "kernel-known-parent-wait"
	r, k, e := start(cfg)
	if e != nil {
		return nil, e
	}
	out, e := r.Call(ctx, ipc.Request{Operation: "execute", Source: []byte("return 42")})
	if e != nil || out.PID != k.Identity.PID || len(out.Result.Values) != 1 || out.Result.Values[0].Number != "42" {
		r.Kill()
		return nil, m2.ErrPrivate
	}
	if e = stop(r, k, false); e != nil {
		return nil, e
	}
	facts["real_parent_wait_kernel_binding"] = k
	// Counterfeit metadata never names an authorized child. Only the original
	// bound owner can request actual parent cleanup.
	tlsConfig, e := m2.TLSConfig(c.TLS, "lua-runner", false)
	if e != nil {
		return nil, e
	}
	private, e := m2.UnixClient(c.SupervisorSocket, tlsConfig, c.PeerUID)
	if e != nil {
		return nil, e
	}
	defer private.CloseIdleConnections()
	for _, fault := range []string{"pid", "epoch", "sequence"} {
		stage = "kernel-forged-" + fault
		r, k, e = start(cfg)
		if e != nil {
			return nil, e
		}
		id := k.Identity
		seq := uint64(1)
		switch fault {
		case "pid":
			id.PID++
		case "epoch":
			id.Epoch = strings.Repeat("0", 48)
		case "sequence":
			seq++
		}
		v := struct {
			Binding struct {
				Identity m2.ChildIdentity
				Sequence uint64
			}
			Request ipc.Request
		}{}
		v.Binding.Identity = id
		v.Binding.Sequence = seq
		v.Request = ipc.Request{Operation: "execute", Source: []byte("return 1")}
		raw, _ := json.Marshal(v)
		var frame bytes.Buffer
		if e = ipc.WriteFrame(&frame, raw); e != nil {
			return nil, e
		}
		request, _ := http.NewRequestWithContext(ctx, "POST", "https://lua-runner/call", &frame)
		reply, e := private.Do(request)
		if e != nil {
			return nil, e
		}
		reply.Body.Close()
		if reply.StatusCode == 200 {
			return nil, m2.ErrPrivate
		}
		if e = stop(r, k, false); e != nil {
			return nil, e
		}
		facts["rejected_"+fault] = true
	}
	// A resource failure is confined to one real VM; another remains usable.
	stage = "kernel-resource-isolation"
	other, otherK, e := start(cfg)
	if e != nil {
		return nil, e
	}
	r, k, e = start(cfg)
	if e != nil {
		other.Kill()
		return nil, e
	}
	_, e = r.Call(ctx, ipc.Request{Operation: "execute", Source: []byte("while true do end")})
	if e == nil {
		r.Kill()
		other.Kill()
		return nil, m2.ErrPrivate
	}
	if e = stop(r, k, false); e != nil {
		other.Kill()
		return nil, e
	}
	out, e = other.Call(ctx, ipc.Request{Operation: "execute", Source: []byte("return 43")})
	if e != nil || out.PID != otherK.Identity.PID {
		return nil, m2.ErrPrivate
	}
	if e = stop(other, otherK, false); e != nil {
		return nil, e
	}
	facts["one_vm_resource_failure_isolated"] = true
	// The callback remains in the platform caller; cancel joins it and fences
	// this lifetime even if a later status acknowledgement becomes available.
	hostConfig := profile.Config{Limits: c.Limits, Modules: map[string][]byte{"main.lua": []byte("return {validate_command=function(c) return true end,execute_command=function(c) return host.time.now() end}")}, Host: &profile.HostConfig{Entry: "main.lua", Required: []string{"validate_command", "execute_command"}, CallbackLimit: 8}}
	for _, fault := range []string{"cancel-callback", "audit-denial"} {
		stage = "kernel-" + fault + "-load"
		r, k, e = start(hostConfig)
		if e != nil {
			return nil, e
		}
		// ipc.Init configures the Host profile. The original VM then performs
		// host-load before invoking its verified callback table; this direct
		// kernel exercise must follow that same protocol sequence.
		if _, e = r.Call(ctx, ipc.Request{Operation: "host-load"}); e != nil {
			r.Kill()
			return nil, e
		}
		stage = "kernel-" + fault + "-call"
		call, done := context.WithCancel(ctx)
		entered := make(chan struct{}, 1)
		finished := make(chan error, 1)
		go func() {
			_, e := r.CallWithHost(call, ipc.Request{Operation: "host-invoke", Callback: "command", Arguments: []checkpoint.Value{checkpoint.Object(map[string]checkpoint.Value{})}}, func(callback context.Context, _ profile.HostCall) (checkpoint.Value, error) {
				entered <- struct{}{}
				if fault == "audit-denial" {
					return checkpoint.Value{}, profile.Fail(profile.ErrCapability)
				}
				<-callback.Done()
				return checkpoint.Value{}, callback.Err()
			})
			finished <- e
		}()
		select {
		case <-entered:
		case <-time.After(2 * time.Second):
			done()
			r.Kill()
			return nil, m2.ErrPrivate
		}
		if fault == "cancel-callback" {
			done()
		}
		select {
		case e = <-finished:
			if e == nil {
				done()
				return nil, m2.ErrPrivate
			}
		case <-time.After(3 * time.Second):
			done()
			return nil, m2.ErrPrivate
		}
		done()
		if e = stop(r, k, fault == "cancel-callback"); e != nil {
			return nil, e
		}
		facts[fault] = true
	}

	stage = "kernel-lifetime-connection-eof"
	lifetime := *private
	lifetime.Timeout = 0
	lifeCtx, closeLife := context.WithCancel(ctx)
	openBytes, _ := json.Marshal(struct{ Config profile.Config }{cfg})
	openRequest, _ := http.NewRequestWithContext(lifeCtx, "POST", "https://lua-runner/open", bytes.NewReader(openBytes))
	openRequest.Header.Set("Content-Type", "application/json")
	openReply, e := lifetime.Do(openRequest)
	if e != nil {
		closeLife()
		return nil, e
	}
	frame, e := ipc.ReadFrame(openReply.Body)
	var opened struct {
		Binding struct {
			Identity m2.ChildIdentity
			Sequence uint64
		}
		Exit   ipc.Exit
		Closed bool
	}
	if e != nil || checkpoint.StrictDecode(frame, &opened, 8192) != nil || opened.Closed || opened.Binding.Sequence != 0 || opened.Binding.Identity.RunnerHash != c.RunnerHash {
		closeLife()
		openReply.Body.Close()
		return nil, m2.ErrPrivate
	}
	disconnected, e := observeChild(ctx, project, opened.Binding.Identity, compose, command)
	if e != nil {
		closeLife()
		openReply.Body.Close()
		return nil, e
	}
	closeLife()
	openReply.Body.Close()
	if e = childGone(ctx, disconnected); e != nil {
		return nil, e
	}
	statusBytes, _ := json.Marshal(opened.Binding)
	statusRequest, _ := http.NewRequestWithContext(ctx, "POST", "https://lua-runner/status", bytes.NewReader(statusBytes))
	statusRequest.Header.Set("Content-Type", "application/json")
	statusReply, e := private.Do(statusRequest)
	if e != nil {
		return nil, e
	}
	statusRaw, e := io.ReadAll(io.LimitReader(statusReply.Body, 8193))
	statusReply.Body.Close()
	var joined struct {
		Binding struct {
			Identity m2.ChildIdentity
			Sequence uint64
		}
		Exit   ipc.Exit
		Closed bool
	}
	if e != nil || statusReply.StatusCode != 200 || checkpoint.StrictDecode(statusRaw, &joined, 8192) != nil || joined.Binding != opened.Binding || !joined.Closed || !joined.Exit.Reaped || joined.Exit.PID != opened.Binding.Identity.PID {
		return nil, m2.ErrPrivate
	}
	facts["lifetime_eof_true_parent_wait"] = disconnected
	stage = "kernel-supervisor-death"
	r, k, e = start(cfg)
	if e != nil {
		return nil, e
	}
	if _, e = compose(ctx, "kill", "--signal", "SIGKILL", "lua-runner"); e != nil {
		return nil, e
	}
	if e = childGone(ctx, k); e != nil {
		return nil, e
	}
	if e = stop(r, k, true); e != nil {
		return nil, e
	}
	if _, e = r.Call(ctx, ipc.Request{Operation: "execute", Source: []byte("return 1")}); e == nil {
		return nil, m2.ErrPrivate
	}
	facts["supervisor_death_sticky_unknown"] = true
	stage = "kernel-supervisor-restart"
	if _, e = compose(ctx, "up", "--detach", "lua-runner"); e != nil {
		return nil, e
	}
	stage = "kernel-supervisor-restart-ready"
	deadline := time.Now().Add(15 * time.Second)
	for launcher.Ready(ctx) != nil {
		if time.Now().After(deadline) {
			return nil, m2.ErrPrivate
		}
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-time.After(50 * time.Millisecond):
		}
	}
	stage = "kernel-supervisor-restart-child"
	r2, k2, e := start(cfg)
	if e != nil {
		return nil, e
	}
	facts["restart_kernel_identity"] = map[string]any{"old": k.Identity, "new": k2.Identity, "new_host_pid": k2.HostPID, "new_host_parent": k2.HostParent}
	stage = "kernel-supervisor-restart-identity"
	// A Docker container restart may retain its namespace inode. The complete
	// observed identity includes the fresh epoch/handle and kernel birth, rather
	// than assuming an inode or a namespace-relative PID is globally unique.
	if k2.Identity.Epoch == k.Identity.Epoch || k2.Identity.Handle == k.Identity.Handle || k2.HostPID == k.HostPID || k2.HostParent == k.HostParent || k2.Identity.StartTime == k.Identity.StartTime {
		r2.Kill()
		return nil, m2.ErrPrivate
	}

	stage = "kernel-old-full-identity-after-restart"
	oldCall := struct {
		Binding struct {
			Identity m2.ChildIdentity
			Sequence uint64
		}
		Request ipc.Request
	}{}
	oldCall.Binding.Identity = k.Identity
	oldCall.Binding.Sequence = 1
	oldCall.Request = ipc.Request{Operation: "execute", Source: []byte("return 1")}
	oldRaw, _ := json.Marshal(oldCall)
	var oldFrame bytes.Buffer
	if e = ipc.WriteFrame(&oldFrame, oldRaw); e != nil {
		return nil, e
	}
	oldRequest, _ := http.NewRequestWithContext(ctx, "POST", "https://lua-runner/call", &oldFrame)
	oldRequest.Header.Set("Content-Type", "application/json")
	oldReply, e := private.Do(oldRequest)
	if e != nil {
		return nil, e
	}
	oldReply.Body.Close()
	if oldReply.StatusCode == 200 {
		return nil, m2.ErrPrivate
	}
	fresh, e := r2.Call(ctx, ipc.Request{Operation: "execute", Source: []byte("return 45")})
	if e != nil || fresh.PID != k2.Identity.PID || len(fresh.Result.Values) != 1 || fresh.Result.Values[0].Number != "45" {
		return nil, m2.ErrPrivate
	}
	facts["old_composite_identity_denied_new_child_usable"] = true
	stage = "kernel-supervisor-restart-wait"
	if e = stop(r2, k2, false); e != nil {
		return nil, e
	}
	if _, e = r.StopAndWait(ctx); !errors.Is(e, ipc.ErrUnknownExit) {
		return nil, m2.ErrPrivate
	}
	facts["old_epoch_cannot_promote_unknown"] = true
	// platformd exits its lost lifetimes before restoring from durable data.
	if _, e = compose(ctx, "stop", "--timeout", "8", "reverse-proxy", "workerd", "platformd"); e != nil {
		return nil, e
	}
	if _, e = compose(ctx, "up", "--detach", "platformd", "workerd", "reverse-proxy"); e != nil {
		return nil, e
	}
	return facts, nil
}

func sqlMetadata(ctx context.Context, compose Compose, query string) (string, error) {
	b, e := compose(ctx, "exec", "--no-TTY", "postgres", "psql", "--no-psqlrc", "--set=ON_ERROR_STOP=1", "--username=trpg_operator", "--dbname=trpg_m2", "--tuples-only", "--no-align", "--command", query)
	if e != nil {
		return "", e
	}
	return strings.TrimSpace(string(b)), nil
}
func bindingQuote(value string) (string, error) {
	if !regexpID(value) {
		return "", m2.ErrPrivate
	}
	return "'" + value + "'", nil
}
func regexpID(s string) bool {
	if s == "" || len(s) > 128 {
		return false
	}
	for _, r := range s {
		if !(r >= 'a' && r <= 'z' || r >= 'A' && r <= 'Z' || r >= '0' && r <= '9' || strings.ContainsRune("_.-", r)) {
			return false
		}
	}
	return true
}
func stableMetadata(ctx context.Context, l Lobby, compose Compose) (map[string]string, error) {
	w, e := bindingQuote(l.Workspace)
	if e != nil {
		return nil, e
	}
	s, e := bindingQuote(l.Session)
	if e != nil {
		return nil, e
	}
	values := map[string]string{}
	for name, q := range map[string]string{
		"version": "SELECT version FROM host_command.sessions WHERE workspace=" + w + " AND session=" + s,
		"tasks":   "SELECT coalesce(md5(string_agg(status||':'||attempt::text||':'||encode(body,'hex'),',' ORDER BY task_id)),'empty') FROM platform_task.jobs WHERE workspace=" + w + " AND session=" + s,
		"billing": "SELECT coalesce(md5(string_agg(encode(used,'hex')||encode(held,'hex'),',' ORDER BY level,node_id)),'empty') FROM platform_budget.counters WHERE workspace_id=" + w,
		"model":   "SELECT coalesce(md5(string_agg(version::text||revoked::text||encode(body,'hex'),',' ORDER BY room_id,seat_id,selection)),'empty') FROM platform_model.configurations WHERE workspace_id=" + w,
	} {
		v, e := sqlMetadata(ctx, compose, q)
		if e != nil {
			return nil, e
		}
		values[name] = v
	}
	return values, nil
}
func equalMetadata(a, b map[string]string) bool {
	if len(a) != len(b) {
		return false
	}
	for k, v := range a {
		if b[k] != v {
			return false
		}
	}
	return true
}

// Summing the original used metrics ignores new zero-valued counter rows.
// Unknown settlement records conservative exposure in Spent while preserving
// both used counters and their five held upper bounds.
func usedBudgetMetadata(ctx context.Context, l Lobby, compose Compose) (string, error) {
	w, e := bindingQuote(l.Workspace)
	if e != nil {
		return "", e
	}
	metrics := []string{"Calls", "Tokens", "CostMicros", "LatencyMillis", "Tools", "Subagents", "ContextBytes", "LocalComputeMillis"}
	parts := make([]string, len(metrics))
	for i, metric := range metrics {
		parts[i] = "coalesce(sum((convert_from(used,'UTF8')::jsonb->>'" + metric + "')::numeric),0)::text"
	}
	return sqlMetadata(ctx, compose, "SELECT md5("+strings.Join(parts, "||','||")+") FROM platform_budget.counters WHERE workspace_id="+w)
}

// A diagnostic boolean applies the unchanged CanReserve arithmetic to the
// actual shared workspace cell. No counter, cap, task or pause is modified.
func workspaceReservationFits(ctx context.Context, l Lobby, plan m2.OperatorPlan, compose Compose) (string, error) {
	w, e := bindingQuote(l.Workspace)
	cap, ok := plan.Caps[l.Workspace]
	if e != nil || !ok || !budget.ValidCaps(cap) || !budget.Valid(plan.Amount) {
		return "", m2.ErrPrivate
	}
	amounts := []uint64{plan.Amount.Calls, plan.Amount.Tokens, plan.Amount.CostMicros, plan.Amount.LatencyMillis, plan.Amount.Tools, plan.Amount.Subagents, plan.Amount.ContextBytes, plan.Amount.LocalComputeMillis}
	limits := []uint64{cap.Workspace.Calls, cap.Workspace.Tokens, cap.Workspace.CostMicros, cap.Workspace.LatencyMillis, cap.Workspace.Tools, cap.Workspace.Subagents, cap.Workspace.ContextBytes, cap.Workspace.LocalComputeMillis}
	metrics := []string{"Calls", "Tokens", "CostMicros", "LatencyMillis", "Tools", "Subagents", "ContextBytes", "LocalComputeMillis"}
	parts := make([]string, len(metrics))
	valid := []string{}
	for i, metric := range metrics {
		parts[i] = "(convert_from(used,'UTF8')::jsonb->>'" + metric + "')::numeric+(convert_from(held,'UTF8')::jsonb->>'" + metric + "')::numeric+" + strconv.FormatUint(amounts[i], 10) + "<=" + strconv.FormatUint(limits[i], 10)
		for _, column := range []string{"used", "held"} {
			value := "convert_from(" + column + ",'UTF8')::jsonb"
			valid = append(valid, "jsonb_typeof("+value+"->'"+metric+"')='number' AND ("+value+"->>'"+metric+"') ~ '^(0|[1-9][0-9]*)$'")
		}
	}
	v, e := sqlMetadata(ctx, compose, "SELECT CASE WHEN count(*)=1 AND coalesce(bool_and("+strings.Join(valid, " AND ")+"),false) THEN bool_and("+strings.Join(parts, " AND ")+") END FROM platform_budget.counters WHERE workspace_id="+w+" AND level='workspace' AND node_id="+w)
	if e != nil || v != "t" && v != "f" {
		return "", m2.ErrPrivate
	}
	return v, nil
}

func uncertainHoldMetadata(ctx context.Context, l Lobby, compose Compose) (string, error) {
	w, e := bindingQuote(l.Workspace)
	if e != nil {
		return "", e
	}
	s, e := bindingQuote(l.Session)
	if e != nil {
		return "", e
	}
	r, e := bindingQuote(l.Room)
	if e != nil {
		return "", e
	}
	g, e := bindingQuote(l.Game)
	if e != nil {
		return "", e
	}
	return sqlMetadata(ctx, compose, `WITH reservation AS (
 SELECT b.*,convert_from(b.body,'UTF8')::jsonb AS value,convert_from(b.spent,'UTF8')::jsonb AS exposure
 FROM platform_budget.reservations b
 WHERE workspace_id=`+w+` AND session_id=`+s+` AND room_id=`+r+` AND game_id=`+g+` AND status='uncertain'
), expected AS (
 SELECT r.*,v.level,v.node FROM reservation r CROSS JOIN LATERAL (VALUES
 ('workspace',r.workspace_id),('room',r.room_id||'/'||r.game_id),('session',r.session_id),
 ('seat',r.session_id||'/'||r.seat_id),('task',r.session_id||'/'||r.seat_id||'/'||r.task_id)) v(level,node)
), held AS (
 SELECT e.*,convert_from(c.held,'UTF8')::jsonb AS upper_bound
 FROM expected e JOIN platform_budget.counters c ON c.workspace_id=e.workspace_id AND c.level=e.level AND c.node_id=e.node
)
 SELECT (SELECT count(*)=1 AND bool_and(
 value->'Task'->>'ID'=task_id AND value->'Task'->'Subject'->'Scope'->>'WorkspaceID'=workspace_id
 AND value->'Task'->'Subject'->'Scope'->>'RoomID'=room_id AND value->'Task'->'Subject'->'Scope'->>'GameID'=game_id
 AND value->'Task'->'Subject'->'Binding'->>'session'=session_id AND value->'Task'->'Subject'->>'SeatID'=seat_id
 AND exposure=value->'Amount' AND (value->'Amount'->>'Calls')::numeric=1) FROM reservation)
 AND (SELECT count(*)=5 AND count(DISTINCT level)=5 FROM held)
 AND (SELECT count(*)=40 AND bool_and((upper_bound->>metric)::numeric >= (value->'Amount'->>metric)::numeric)
 FROM held CROSS JOIN (VALUES('Calls'),('Tokens'),('CostMicros'),('LatencyMillis'),('Tools'),('Subagents'),('ContextBytes'),('LocalComputeMillis')) metrics(metric))
 AND EXISTS(SELECT 1 FROM reservation r JOIN platform_budget.tasks t USING(workspace_id,room_id,game_id,session_id,seat_id)
 WHERE t.id=r.task_id AND t.status='uncertain')
 AND EXISTS(SELECT 1 FROM reservation r JOIN platform_budget.pauses p USING(workspace_id,session_id,seat_id) WHERE p.reason='uncertain')`)
}

// Only safe aggregate facts cross this diagnostic boundary. These observations
// identify the actual original rows; they never replace the original guards.
func CommandGuardMetadata(ctx context.Context, l Lobby, compose Compose) (map[string]string, error) {
	w, e := bindingQuote(l.Workspace)
	if e != nil {
		return nil, e
	}
	s, e := bindingQuote(l.Session)
	if e != nil {
		return nil, e
	}
	host, e := bindingQuote(l.HostConnection)
	if e != nil {
		return nil, e
	}
	result := map[string]string{}
	queries := map[string]string{
		"control_unpaused":         "SELECT NOT paused FROM platform_player.control WHERE workspace_id=" + w + " AND session_id=" + s,
		"live_human_lease_count":   "SELECT count(*) FROM platform_player.leases WHERE workspace_id=" + w + " AND session_id=" + s + " AND NOT closed AND expires_at>clock_timestamp()",
		"host_connection_current":  "SELECT count(*)=1 FROM platform_player.leases WHERE workspace_id=" + w + " AND session_id=" + s + " AND id=" + host + " AND NOT closed AND expires_at>clock_timestamp()",
		"lease_identities_current": "SELECT coalesce(bool_and(NOT a.retired AND NOT a.revoked AND a.expires_at>clock_timestamp() AND p.active AND NOT ac.disabled AND p.account_id=a.account_id),false) FROM platform_player.leases l JOIN platform_player.control c ON c.workspace_id=l.workspace_id AND c.session_id=l.session_id JOIN platform_auth.sessions a ON a.token_hash=l.cookie_hash JOIN platform_core.accounts ac ON ac.id=a.account_id JOIN platform_room.participants p ON p.workspace_id=c.workspace_id AND p.room_id=c.room_id AND p.game_id=c.game_id AND p.id=l.participant_id WHERE l.workspace_id=" + w + " AND l.session_id=" + s,
		"original_state_version":   "SELECT version FROM host_command.sessions WHERE workspace=" + w + " AND session=" + s,
	}
	for _, name := range []string{"control_unpaused", "live_human_lease_count", "host_connection_current", "lease_identities_current", "original_state_version"} {
		v, e := sqlMetadata(ctx, compose, queries[name])
		if e != nil {
			return result, e
		}
		result[name] = v
	}
	return result, nil
}
func startMixedGame(ctx context.Context, l *Lobby) error {
	for _, b := range []*Browser{l.Owner, l.Participant} {
		if _, e := b.API(ctx, "POST", l.Base()+"/consent", map[string]any{"revision": "1", "consent": true, "ready": true, "safety_confirmed": true, "boundaries": []string{}}); e != nil {
			return e
		}
	}
	d, e := l.Owner.API(ctx, "POST", l.Base()+"/launch", map[string]any{"revision": "1"})
	if e != nil {
		return e
	}
	l.Session, _ = d["session_id"].(string)
	if l.Session == "" {
		return m2.ErrPrivate
	}
	if e = l.Connect(ctx); e != nil {
		return e
	}
	return l.Resume(ctx)
}
func waitAtomic(ctx context.Context, value *atomic.Uint64, want uint64) error {
	timer := time.NewTimer(3 * time.Second)
	defer timer.Stop()
	for value.Load() < want {
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-timer.C:
			return m2.ErrPrivate
		case <-time.After(10 * time.Millisecond):
		}
	}
	return nil
}

func RunRestartAndFaults(ctx context.Context, b *Browser, l Lobby, p *Provider, plan m2.OperatorPlan, c m2.Config, ownerAction func(context.Context, Lobby) error, compose Compose, healthy func(context.Context) (any, error), command Command, stopWorkerd func(context.Context, WorkerdStopBinding) (WorkerdStopFacts, error)) (result any, failure error) {
	facts := map[string]any{}
	stage := "durable-restart-ready"
	defer func() {
		if failure != nil {
			result = facts
			failure = safeStage(stage, failure)
		}
	}()
	if _, e := healthy(ctx); e != nil {
		return nil, e
	}
	before, e := stableMetadata(ctx, l, compose)
	if e != nil {
		return nil, e
	}
	calls := p.Calls()
	if _, e = compose(ctx, "stop", "--timeout", "8"); e != nil {
		return nil, e
	}
	if _, e = compose(ctx, "up", "--detach"); e != nil {
		return nil, e
	}
	if _, e = healthy(ctx); e != nil {
		return nil, e
	}
	if e = l.Connect(ctx); e != nil {
		return nil, e
	}
	d, e := l.Snapshot(ctx, true)
	if e != nil {
		return nil, e
	}
	ctl, ok := d["control"].(map[string]any)
	if !ok || ctl["paused"] != true {
		return nil, m2.ErrPrivate
	}
	after, e := stableMetadata(ctx, l, compose)
	if e != nil || !equalMetadata(before, after) || p.Calls() != calls {
		return nil, m2.ErrPrivate
	}
	facts["same_volumes_durable_restart"] = true
	facts["no_auto_resume_or_model_replay"] = true
	facts["state_task_billing_model_digests"] = after
	stage = "active-platform-death"
	platform, e := observeDaemon(ctx, c, "platformd", "/app/platformd", compose, command)
	if e != nil {
		return nil, e
	}
	supervisor, e := observeDaemon(ctx, c, "lua-runner", "/app/lua-supervisord", compose, command)
	if e != nil {
		return nil, e
	}
	children, e := observeOwnedRunners(ctx, supervisor, c.RunnerHash, command)
	if e != nil {
		return nil, e
	}
	if _, e = revalidateDaemon(ctx, platform, command); e != nil {
		return nil, e
	}
	if _, e = command(ctx, "kill-bound-active-platform", nil, "docker", "kill", "--signal", "SIGKILL", platform.Container); e != nil {
		return nil, e
	}
	for _, child := range children {
		if e = kernelBirthGone(ctx, child.PID, child.StartTime); e != nil {
			return nil, e
		}
	}
	if _, e = revalidateDaemon(ctx, supervisor, command); e != nil {
		return nil, e
	}
	facts["active_platform_death"] = map[string]any{"observed_real_children": len(children), "all_original_kernel_births_gone": true, "same_actual_parent_still_alive": true, "dead_caller_wait_acknowledgement": "UNKNOWN; not promoted to success", "independent_parent_Wait_proof": "original lifecycle EOF check"}
	if _, e = compose(ctx, "up", "--detach", "platformd", "workerd", "reverse-proxy"); e != nil {
		return nil, e
	}
	if _, e = healthy(ctx); e != nil {
		return nil, e
	}
	if e = l.Connect(ctx); e != nil {
		return nil, e
	}
	if _, e = l.Snapshot(ctx, true); e != nil {
		return nil, e
	}
	after, e = stableMetadata(ctx, l, compose)
	if e != nil || !equalMetadata(before, after) || p.Calls() != calls {
		return nil, m2.ErrPrivate
	}
	facts["active_platform_restart_no_replay"] = true
	// These faults kill the actual fixed workerd. Every room has an independent
	// preparation/configuration; an uncertain seat cannot be silently revived.
	// Prove known SaveResult/Post before the three uncertainty cases retain
	// their shared workspace holds. The original finite caps stay unchanged.
	for _, fault := range []string{"after-save-and-post", "before-begin", "during-io", "after-reply"} {
		stage = fault + "-prepare"
		next, e := b.PrepareMixedLobby(ctx, l.Workspace, plan.Games[0].Configuration, l.Game, RegistrationToken)
		if e != nil {
			return nil, safeStage(fault+" prepare", e)
		}
		stage = fault + "-owner-action"
		if e = ownerAction(ctx, next); e != nil {
			return nil, e
		}
		stage = fault + "-start-mixed"
		if e = startMixedGame(ctx, &next); e != nil {
			return nil, e
		}
		var stopped daemonBirth
		if fault == "after-reply" {
			stopped, e = observeDaemon(ctx, c, "workerd", "/app/workerd", compose, command)
			if e != nil {
				return nil, e
			}
		}
		usedBefore, e := usedBudgetMetadata(ctx, next, compose)
		if e != nil {
			return nil, e
		}
		if fault == "after-save-and-post" {
			fits, fitError := workspaceReservationFits(ctx, next, plan, compose)
			facts["known_fault_original_workspace_capacity"] = map[string]any{"fits_next_original_bound": fits == "t", "query_succeeded": fitError == nil}
			if fitError != nil || fits != "t" {
				return nil, m2.ErrPrivate
			}
		}
		calls = p.Calls()
		drainProviderEvents(p)
		p.mode.Store(0)
		replyCount := p.replies.Load()
		cancelCount := p.cancelled.Load()
		if fault == "before-begin" {
			if fault == "after-reply" {
				if _, e = command(ctx, "kill-exact-stopped-workerd", nil, "docker", "kill", "--signal", "SIGKILL", stopped.Container); e != nil {
					return nil, e
				}
			} else if _, e = compose(ctx, "kill", "--signal", "SIGKILL", "workerd"); e != nil {
				return nil, e
			}
		} else if fault != "after-save-and-post" {
			p.mode.Store(1)
		}
		stage = fault + "-source"
		if _, e = next.Command(ctx, "fault-"+fault, "1", ""); e != nil {
			facts["failed_source_"+fault] = map[string]any{"status": next.Owner.lastStatus, "code": next.Owner.lastCode, "request_kind": "human-source-command"}
			guard, guardError := CommandGuardMetadata(ctx, next, compose)
			facts["failed_source_guard_"+fault] = map[string]any{"metadata": guard, "query_succeeded": guardError == nil}
			return nil, safeStage(fault+" source", e)
		}
		if fault == "during-io" || fault == "after-reply" {
			select {
			case <-p.entered:
			case <-time.After(3 * time.Second):
				return nil, m2.ErrPrivate
			}
			if fault == "after-reply" {
				stage = fault + "-actual-daemon-stopped"
				binding, bindingError := workerdStopBinding(stopped)
				if bindingError != nil || stopWorkerd == nil {
					return nil, m2.ErrPrivate
				}
				stopReceipt, stopError := stopWorkerd(ctx, binding)
				facts["actual_daemon_stop_receipt"] = stopReceipt
				if stopError == nil {
					state, proofError := revalidateDaemon(ctx, stopped, command)
					if proofError != nil || state != "T" {
						return nil, m2.ErrPrivate
					}
				}
				if stopError != nil {
					return nil, stopError
				}
				facts["after_reply_signal_target"] = map[string]any{"host_pid": stopped.PID, "start_time": stopped.StartTime, "namespace": stopped.Namespace, "cgroup_hash": stopped.CgroupHash, "binary_hash": stopped.BinaryHash, "owned_source_container": true, "actual_daemon_state_T_before_send": true}
				select {
				case p.release <- struct{}{}:
				case <-ctx.Done():
					return nil, ctx.Err()
				}
				if e = waitAtomic(ctx, &p.replies, replyCount+1); e != nil {
					return nil, e
				}
				state, proofError := revalidateDaemon(ctx, stopped, command)
				if proofError != nil || state != "T" {
					return nil, m2.ErrPrivate
				}
				facts["after_reply_actual_stage"] = map[string]any{"provider_complete_response_sent": true, "daemon_stopped_before_send": true, "daemon_read_or_parsed": "NOT_PROVEN", "platform_complete_save_post_at_fault": "NOT_PROVEN"}
			}
			if _, e = compose(ctx, "kill", "--signal", "SIGKILL", "workerd"); e != nil {
				return nil, e
			}
			if fault == "during-io" {
				if e = waitAtomic(ctx, &p.cancelled, cancelCount+1); e != nil {
					return nil, e
				}
			}
		}
		if fault == "after-save-and-post" {
			stage = "after-save-and-post-await-original-version"
			if _, e = waitSnapshot(ctx, &next, "4"); e != nil {
				current, metadataError := stableMetadata(ctx, next, compose)
				facts["failed_after_save_post_version"] = map[string]any{"metadata": current, "query_succeeded": metadataError == nil, "provider_attempts": p.Calls() - calls, "last_player_status": next.Participant.lastStatus, "last_player_code": next.Participant.lastCode}
				return nil, e
			}
			stage = "after-save-and-post-provider-attempts"
			if p.Calls() != calls+2 {
				facts["failed_after_save_post_provider_attempts"] = map[string]any{"actual": p.Calls() - calls, "expected": 2}
				return nil, m2.ErrPrivate
			}
			stage = "after-save-and-post-original-known-settlement"
			w, qe := bindingQuote(next.Workspace)
			s, se := bindingQuote(next.Session)
			if qe != nil || se != nil {
				return nil, m2.ErrPrivate
			}
			settled, qe := sqlMetadata(ctx, compose, "SELECT (SELECT count(*)=2 AND bool_and(status='settled') FROM platform_budget.reservations WHERE workspace_id="+w+" AND session_id="+s+") AND (SELECT count(*)=2 AND bool_and(status='settled') FROM platform_budget.tasks WHERE workspace_id="+w+" AND session_id="+s+") AND EXISTS(SELECT 1 FROM host_command.sessions h WHERE h.workspace="+w+" AND h.session="+s+" AND h.version=4 AND (h.state->'table'->'counter'->>'number')='3') AND (SELECT count(DISTINCT (convert_from(receipt,'UTF8')::jsonb->>'version'))=2 FROM host_command.requests WHERE workspace="+w+" AND session="+s+" AND principal='task-system' AND (convert_from(receipt,'UTF8')::jsonb->>'version') IN ('3','4') AND (convert_from(receipt,'UTF8')::jsonb->'inputs'->>'callback')='resume_continuation')")
			facts["after_save_post_actual_stage"] = map[string]any{"proof": settled, "query_succeeded": qe == nil, "complete_save_two_original_Post_receipts": settled == "t", "known_settlement_preserved": settled == "t"}
			if qe != nil || settled != "t" {
				return nil, m2.ErrPrivate
			}
			if e = next.Pause(ctx); e != nil {
				return nil, e
			}
			before, e = stableMetadata(ctx, next, compose)
			if e != nil {
				return nil, e
			}
			if _, e = compose(ctx, "kill", "--signal", "SIGKILL", "workerd", "platformd"); e != nil {
				return nil, e
			}
		} else {
			stage = fault + "-uncertain"
			deadline := time.Now().Add(6 * time.Second)
			w, _ := bindingQuote(next.Workspace)
			s, _ := bindingQuote(next.Session)
			for {
				status, e := sqlMetadata(ctx, compose, "SELECT coalesce(string_agg(status,',' ORDER BY task_id),'empty') FROM platform_budget.reservations WHERE workspace_id="+w+" AND session_id="+s)
				if e != nil {
					return nil, e
				}
				if strings.Contains(status, "uncertain") {
					break
				}
				if time.Now().After(deadline) {
					return nil, m2.ErrPrivate
				}
				select {
				case <-ctx.Done():
					return nil, ctx.Err()
				case <-time.After(50 * time.Millisecond):
				}
			}
			if fault == "before-begin" && p.Calls() != calls || fault != "before-begin" && p.Calls() != calls+1 {
				return nil, m2.ErrPrivate
			}
			stage = fault + "-uncertain-five-held-bounds"
			hold, holdError := uncertainHoldMetadata(ctx, next, compose)
			usedAfter, usedError := usedBudgetMetadata(ctx, next, compose)
			facts["observed_uncertain_holds_"+fault] = map[string]any{"proof": hold, "query_succeeded": holdError == nil && usedError == nil, "used_unchanged": usedBefore == usedAfter}
			if holdError != nil || usedError != nil || hold != "t" || usedBefore != usedAfter {
				return nil, m2.ErrPrivate
			}

			stage = fault + "-original-paused-receipt"
			// Observe the unchanged SaveResult/Post before the explicit human pause;
			// pausing an in-flight control lifetime can itself prevent its original Post.
			proof := ""
			until := time.Now().Add(8 * time.Second)
			for {
				proof, e = sqlMetadata(ctx, compose, "SELECT h.version=3 AND (h.state->'table'->'counter'->>'number')='2' AND EXISTS(SELECT 1 FROM host_command.requests q WHERE q.workspace=h.workspace AND q.session=h.session AND q.principal='task-system' AND (convert_from(q.receipt,'UTF8')::jsonb->>'version')='3' AND (convert_from(q.receipt,'UTF8')::jsonb->'inputs'->>'callback')='resume_continuation' AND (convert_from(q.receipt,'UTF8')::jsonb->'inputs'->'tool_results'->0->'table'->'status'->>'string')='paused') FROM host_command.sessions h WHERE h.workspace="+w+" AND h.session="+s)
				if e != nil || proof == "t" || time.Now().After(until) {
					break
				}
				select {
				case <-ctx.Done():
					return nil, ctx.Err()
				case <-time.After(50 * time.Millisecond):
				}
			}
			facts["observed_paused_receipt_"+fault] = map[string]any{"proof": proof, "query_succeeded": e == nil}
			if e != nil || proof != "t" {
				current, _ := stableMetadata(ctx, next, compose)
				facts["failed_original_post_metadata_"+fault] = current
				return nil, m2.ErrPrivate
			}
			if e = next.Pause(ctx); e != nil {
				return nil, e
			}
			before, e = stableMetadata(ctx, next, compose)
			if e != nil || before["version"] != "3" {
				return nil, m2.ErrPrivate
			}

			facts["original_paused_receipt_"+fault] = map[string]any{"version": before["version"], "counter": "2", "original_receipt_verified": true, "no_ai_effect": true}
		}
		p.mode.Store(0)
		wantCalls := p.Calls()
		stage = fault + "-restart-fencing"
		if _, e = compose(ctx, "up", "--detach", "platformd", "workerd", "reverse-proxy"); e != nil {
			return nil, e
		}
		if _, e = healthy(ctx); e != nil {
			return nil, e
		}
		if e = next.Connect(ctx); e != nil {
			return nil, e
		}
		time.Sleep(200 * time.Millisecond)
		after, e = stableMetadata(ctx, next, compose)
		if e != nil || !equalMetadata(before, after) || p.Calls() != wantCalls {
			return nil, safeStage(fault+" replay fencing", e)
		}
		if fault != "after-save-and-post" {
			hold, holdError := uncertainHoldMetadata(ctx, next, compose)
			usedAfter, usedError := usedBudgetMetadata(ctx, next, compose)
			if holdError != nil || usedError != nil || hold != "t" || usedAfter != usedBefore {
				return nil, m2.ErrPrivate
			}
		}
		facts["workerd_"+fault] = map[string]any{"provider_attempts": wantCalls - calls, "no_retry_or_fallback": true, "original_task_billing_receipts": after}
	}
	remaining, e := workspaceReservationFits(ctx, l, plan, compose)
	if e != nil {
		return nil, e
	}
	facts["shared_workspace_after_all_unknown_holds"] = map[string]any{"fits_next_original_bound": remaining == "t", "query_succeeded": true, "original_caps_unchanged": true, "original_holds_preserved": true, "extra_dispatch": false}
	return facts, nil
}

// Object faults are independent of model process faults. Running this complete
// group before that matrix exposes each actual consumer failure without waiting
// for unrelated room preparation. Neither group can be skipped in formal mode.
func RunObjectFaults(ctx context.Context, p *Provider, plan m2.OperatorPlan, objectRoot string, compose Compose, healthy func(context.Context) (any, error), objectProbe func(context.Context, string) (any, error)) (result any, failure error) {
	facts := map[string]any{}
	stage := "object-baseline-three-consumers"
	defer func() {
		if failure != nil {
			result = facts
			failure = safeStage(stage, failure)
		}
	}()
	if objectProbe == nil {
		return nil, m2.ErrPrivate
	}
	baseline, e := objectProbe(ctx, "baseline")
	facts["object_baseline_three_consumers"] = baseline
	if e != nil {
		return nil, e
	}
	ownerCalls := p.Calls()
	stage = "object-unavailable-three-consumers"
	if _, e = compose(ctx, "stop", "--timeout", "8", "object-storage"); e != nil {
		return nil, e
	}
	unavailable, e := objectProbe(ctx, "unavailable")
	facts["object_unavailable_three_consumers"] = unavailable
	if e != nil {
		return nil, e
	}
	if p.Calls() != ownerCalls {
		return nil, m2.ErrPrivate
	}
	// Startup Ready rejection is separate from all three actual byte consumers.
	stage = "object-unavailable-startup-ready"
	if _, e = compose(ctx, "stop", "--timeout", "8", "platformd", "workerd", "reverse-proxy"); e != nil {
		return nil, e
	}
	if _, e = compose(ctx, "up", "--detach", "--no-deps", "platformd"); e != nil {
		return nil, e
	}
	deadline := time.Now().Add(8 * time.Second)
	rejected := false
	for {
		raw, pe := compose(ctx, "ps", "--all", "--format", "json", "platformd")
		if pe != nil {
			return nil, pe
		}
		if bytes.Contains(raw, []byte(`"State":"exited"`)) && !bytes.Contains(raw, []byte(`"ExitCode":0`)) {
			rejected = true
			break
		}
		if time.Now().After(deadline) {
			break
		}
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-time.After(50 * time.Millisecond):
		}
	}
	if !rejected {
		return nil, m2.ErrPrivate
	}
	logs, e := compose(ctx, "logs", "--no-color", "--tail", "8", "platformd")
	if e != nil || !bytes.Contains(logs, []byte("M2 platform unavailable at objects readiness")) {
		return nil, m2.ErrPrivate
	}
	facts["object_unavailable_startup_ready_rejected_separately"] = true
	if _, e = compose(ctx, "up", "--detach", "object-storage", "platformd", "workerd", "reverse-proxy"); e != nil {
		return nil, e
	}
	if _, e = healthy(ctx); e != nil {
		return nil, e
	}
	if _, e = objectProbe(ctx, "baseline"); e != nil {
		return nil, e
	}
	stage = "object-exact-committed-root-corruption"
	// The key is obtained from the actual committed exact root artifact. A map,
	// filename scan or package identity alone cannot select corruption evidence.
	g := plan.Games[1]
	w, qe := bindingQuote(g.Workspace)
	if qe != nil || !checkpoint.IsDigest(g.Root) {
		return nil, m2.ErrPrivate
	}
	key, e := sqlMetadata(ctx, compose, "SELECT archive_key FROM package_install.artifacts WHERE workspace="+w+" AND identity='"+g.Root+"'")
	if e != nil || !checkpoint.IsDigest(key) {
		return nil, m2.ErrPrivate
	}
	file := filepath.Join(objectRoot, strings.TrimPrefix(key, "sha256:"))
	info, e := os.Lstat(file)
	if e != nil || !info.Mode().IsRegular() || info.Mode().Perm()&0222 != 0 || info.Size() <= 0 || info.Size() > 80<<20 {
		return nil, m2.ErrPrivate
	}
	original, e := os.ReadFile(file)
	if e != nil || object.Hash(original) != key {
		return nil, m2.ErrPrivate
	}
	defer clear(original)
	restored := false
	restore := func() error {
		if e := privateFile(file, original); e != nil {
			return e
		}
		if e := os.Chmod(file, info.Mode().Perm()); e != nil {
			return e
		}
		raw, e := os.ReadFile(file)
		defer clear(raw)
		if e != nil || !bytes.Equal(raw, original) {
			return m2.ErrPrivate
		}
		restored = true
		return nil
	}
	defer func() {
		if !restored {
			failure = errors.Join(failure, restore())
		}
	}()
	if e = privateFile(file, []byte("owned exact root corruption")); e != nil {
		return nil, e
	}
	if e = os.Chmod(file, info.Mode().Perm()); e != nil {
		return nil, e
	}
	corrupt, e := objectProbe(ctx, "corrupt")
	facts["object_corrupt_three_consumers"] = corrupt
	if e != nil {
		return nil, e
	}
	if p.Calls() != ownerCalls {
		return nil, m2.ErrPrivate
	}
	if e = restore(); e != nil {
		return nil, e
	}
	stage = "object-restored-three-consumers"
	recovered, e := objectProbe(ctx, "baseline")
	facts["object_restored_three_consumers"] = recovered
	if e != nil {
		return nil, e
	}
	facts["object_owner_shutdown_exact_root_corruption_no_fallback"] = true
	return facts, nil
}

func CarrierPlan(pkg *archive.Package, policy install.PolicyConfig, workspace, endpoint string) (m2.OperatorPlan, error) {
	lock, e := pkg.ExactLock().Digest()
	if e != nil {
		return m2.OperatorPlan{}, e
	}
	root := string(pkg.ArtifactIdentity().Digest())
	seats := []launch.SeatRule{{ID: "gm", Required: true, Modes: []string{"human"}}, {ID: "player", Required: true, Modes: []string{"human"}}, {ID: "ai", Required: true, Modes: []string{"ai"}, ModelCapabilities: []string{"structured-actions"}}}
	ref := func(name string, seed checkpoint.Value) install.SchemaReference {
		path := "schemas/" + name + ".schema.json"
		entry, _ := pkg.Entry(path)
		return install.SchemaReference{PackageID: PackageID, Path: path, Digest: checkpoint.Hash(entry.Bytes()), Seed: seed}
	}
	g := m2.GamePlan{Workspace: workspace, Operator: "explicit-test-operator", InstallID: "m2-test-carrier-install", InstallCredentialFile: "/run/operator/install-credential", ArchiveFile: "/run/operator/carrier.zip", Root: root, PackageID: PackageID, GraphHash: string(lock), Configuration: "m2-carrier", Game: "game", Title: "Installed Linux deployment test carrier", Dependencies: []string{}, ContentTags: []string{}, SafetyTags: []string{}, Evidence: map[string]install.Evidence{}, Seats: seats, Views: map[string]command.ViewPolicy{}, Commands: map[string]install.SchemaReference{"increment": ref("command", checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(1)})), "fail": ref("command", checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(1)}))}, ContextViews: map[string]command.ViewPolicy{}}
	for _, seat := range []string{"public", "gm", "player", "ai"} {
		fields := []string{"counter", "pending_action"}
		if seat == "gm" {
			fields = append(fields, "secret")
		}
		g.Views[seat] = command.ViewPolicy{ViewFields: fields, EventFields: map[string][]string{PackageID + "/change": fields}, ScalarResult: true}
		g.ContextViews[seat] = command.ViewPolicy{ViewFields: []string{"counter"}, EventFields: map[string][]string{PackageID + "/change": {"counter"}}}
	}
	raw, _ := json.Marshal(struct {
		ID, Workspace, Root string
		Dependencies        []string
		Seats               []launch.SeatRule
		Content, Safety     []string
	}{g.Configuration, g.Workspace, g.Root, g.Dependencies, g.Seats, g.ContentTags, g.SafetyTags})
	h := sha256.Sum256(append([]byte("platform-launch-configuration/v1\x00"), raw...))
	g.ConfigurationHash = "sha256:" + hex.EncodeToString(h[:])
	tuple := model.Tuple{Model: "fixture:small", Endpoint: endpoint, Adapter: "openai-compatible", PromptTemplate: "safe-v1", ToolMode: "structured", TestVersion: "m2-synthetic-v1"}
	cert := model.CertificationData{ID: "default-model", WorkspaceID: workspace, Tuple: tuple, Level: 3, Capabilities: []string{"structured-actions", "ai-player"}, Games: []model.GameEvidence{{GraphHash: g.GraphHash, TestVersion: tuple.TestVersion, EvidenceHash: strings.TrimPrefix(checkpoint.Hash([]byte("synthetic carrier Actor conformance")), "sha256:"), Capabilities: []string{"structured-actions", "ai-player"}}}, EvidenceHash: strings.TrimPrefix(checkpoint.Hash([]byte("TEST_ONLY offline compatible provider")), "sha256:"), ExpiresAt: time.Now().Add(time.Hour)}
	units := budget.Units{Calls: 32, Tokens: 20000, CostMicros: 40000, ContextBytes: 65536, LatencyMillis: 8000, LocalComputeMillis: 8000, Tools: 16, Subagents: 4}
	return m2.OperatorPlan{Classification: "TEST_ONLY_STANDARD_CARRIER", InstallPolicy: policy, Games: []m2.GamePlan{g}, Certificates: []model.CertificationData{cert}, Defaults: map[string]string{workspace: "default-model"}, Labels: map[string]string{workspace + "/selected": "Owned synthetic local model"}, WorkspaceLimits: map[string]model.Limits{workspace: CarrierLimits()}, Caps: map[string]budget.Caps{workspace: {Workspace: units, Room: units, Session: units, Seat: units, Task: units}}, Amount: budget.Units{Calls: 1, Tokens: 100, CostMicros: 1000, ContextBytes: 4096, LatencyMillis: 2000, LocalComputeMillis: 2000, Tools: 1}, MaxCalls: 1, TaskCredentialFile: "/run/operator/task-credential"}, nil
}
func CarrierLimits() model.Limits {
	return model.Limits{Calls: 8, Tokens: 8192, CostMicros: 10000, LatencyMillis: 4000, Tools: 8, Subagents: 1, ContextBytes: 16384, LocalComputeMillis: 4000}
}
func aiCommandInput(typ string, delta int64) checkpoint.Value {
	return checkpoint.Object(map[string]checkpoint.Value{"type": checkpoint.Text(typ), "seat_id": checkpoint.Text("gm"), "correlation_id": checkpoint.Text("loopFixture-case"), "payload": checkpoint.Object(map[string]checkpoint.Value{"delta": checkpoint.Int(delta)})})
}
func BuildCarrier(runtime install.RuntimeConfig, source string) (*archive.Package, install.PolicyConfig, error) {
	if source == "" {
		source = aiIncrementSource
	}
	stateSchema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"counter":{"type":"integer","minimum":0,"maximum":1000000000},"secret":{"type":"string","maxLength":128}},"required":["counter","secret"],"additionalProperties":false}`)
	inputs := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"delta":{"type":"integer","minimum":1,"maximum":1000}},"required":["delta"],"additionalProperties":false}`)
	// The default also validates recorded external ToolResults. Human command
	// integer results retain their exact original branch in this private Schema.
	pkg, err := base.Package("", source, map[string][]byte{"schemas/state.schema.json": stateSchema, "schemas/event.schema.json": stateSchema, "schemas/command.schema.json": inputs, "schemas/view.schema.json": []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"counter":{"type":"integer"},"secret":{"type":"string","maxLength":128},"pending_action":{"type":"string","maxLength":128}},"required":["counter"],"additionalProperties":false}`), "schemas/ai-intent.schema.json": []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"seat_id":{"type":"string"},"selection":{"type":"string"},"mode":{"type":"string","enum":["proposal","narrative"]}},"required":["seat_id","selection","mode"],"additionalProperties":false}`), "schemas/lifecycle.schema.json": []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false}`), "schemas/model-result.schema.json": []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","anyOf":[{"type":"integer"},{"type":"object","properties":{"mode":{"enum":["proposal","narrative"]},"status":{"enum":["complete","paused"]},"action":{"type":"object","properties":{"type":{"const":"increment"},"expected_state_version":{"type":"integer","minimum":1,"maximum":9007199254740991},"payload":{"type":"object","properties":{"delta":{"type":"integer","const":1}},"required":["delta"],"additionalProperties":false}},"required":["type","expected_state_version","payload"],"additionalProperties":false},"narrative":{"type":"string","minLength":1,"maxLength":16384}},"required":["mode","status"],"additionalProperties":false,"oneOf":[{"properties":{"mode":{"const":"proposal"},"status":{"const":"complete"}},"required":["action"],"not":{"required":["narrative"]}},{"properties":{"mode":{"const":"narrative"},"status":{"const":"complete"}},"required":["narrative"],"not":{"required":["action"]}},{"properties":{"status":{"const":"paused"}},"not":{"required":["action"]}}]}]}`)})
	if err != nil {
		return nil, install.PolicyConfig{}, err
	}
	ref := func(name string, seed checkpoint.Value) install.SchemaReference {
		path := "schemas/" + name + ".schema.json"
		entry, _ := pkg.Entry(path)
		return install.SchemaReference{PackageID: PackageID, Path: path, Digest: checkpoint.Hash(entry.Bytes()), Seed: seed}
	}
	state := ref("state", aiState(1))
	empty := checkpoint.Object(map[string]checkpoint.Value{})
	result := ref("model-result", checkpoint.Object(map[string]checkpoint.Value{"mode": checkpoint.Text("proposal"), "status": checkpoint.Text("paused")}))
	row := ref("row", checkpoint.Object(map[string]checkpoint.Value{"score": checkpoint.Int(1)}))
	intent := ref("intent", checkpoint.Object(map[string]checkpoint.Value{"value": checkpoint.Int(1)}))
	object := ref("lifecycle", empty)
	contract := &install.HostContract{State: state, Result: result, Results: map[string]install.SchemaReference{}, Namespaces: map[string]install.NamespaceContract{PackageID + "/docs": {Schema: row, Indices: map[string][]string{"score": {"score"}}, MaxRows: 128, MaxBytes: 256 << 10}}, Events: map[string]install.SchemaReference{PackageID + "/change": ref("event", aiState(1))}, Intents: map[string]install.SchemaReference{PackageID + "/task": intent, PackageID + "/continuation": intent, PackageID + "/ai": ref("ai-intent", checkpoint.Object(map[string]checkpoint.Value{"seat_id": checkpoint.Text("ai"), "selection": checkpoint.Text("selected"), "mode": checkpoint.Text("proposal")}))}, Named: map[string]install.NamedContract{}, Budget: hostapi.DefaultBudget()}
	tests := []install.Test{{Name: "raw-pure", Source: []byte("return true")}}
	counter := int64(1)
	added := false
	for _, name := range profile.StandardCallbackNames() {
		if name == "validate_command" || name == "execute_command" {
			if added {
				continue
			}
			added = true
			name = "command"
		}
		input := empty
		expected := empty
		if name == "command" {
			counter++
			input = aiCommandInput("increment", 1)
			expected = checkpoint.Int(counter)
		} else if name == "project_view" || name == "create_checkpoint" {
			expected = aiState(counter)
			contract.Results[name] = state
			if name == "project_view" {
				contract.Results[name] = ref("view", aiState(counter))
			}
		} else {
			contract.Results[name] = object
		}
		tests = append(tests, install.Test{Name: "case-" + name, Host: &install.HostCase{Callback: name, Input: input, Expected: expected, Time: 1000, Random: []int64{7}}})
	}
	doc, err := pkg.Manifest()
	if err != nil {
		return nil, install.PolicyConfig{}, err
	}
	policy := install.PolicyConfig{Context: "ci", HostMajor: profile.HostMajor, HostMinor: profile.HostMinor, Artifacts: map[string]install.Approval{string(pkg.ArtifactIdentity().Digest()): {RightsDigest: install.RightsDigest(*doc.Package), Retention: "synthetic", Safety: "ACTIVE", Tests: tests, Host: contract}}, Host: &install.HostAuthorization{Trust: map[capability.TrustLevel][]string{}, Execution: append([]string(nil), base.AllCapabilities...), RunnerHash: runtime.SHA256, Limits: runtime.Limits}}
	for _, level := range []capability.TrustLevel{capability.TrustOfficial, capability.TrustSigned, capability.TrustPrivateUnverified, capability.TrustDevelopment} {
		policy.Host.Trust[level] = append([]string(nil), base.AllCapabilities...)
	}
	if policy.Host.RunnerHash == "" {
		return nil, install.PolicyConfig{}, fmt.Errorf("loopFixture requires actual loopRunner hash")
	}
	return pkg, policy, nil
}

// A real standalone dependency proves that the private plan cannot substitute
// a preinstalled object or an artifact name for the original archive graph.
func BuildCarrierWithDependency(runtime install.RuntimeConfig) (*archive.Package, *archive.Package, install.PolicyConfig, error) {
	root, policy, e := BuildCarrier(runtime, "")
	if e != nil {
		return nil, nil, policy, e
	}
	dep, e := archivefixture.Build(archivefixture.Files("test.publisher/m2rules", "library", ""))
	if e != nil {
		return nil, nil, policy, e
	}
	files := map[string][]byte{}
	for _, entry := range root.Entries() {
		files[entry.Path()] = entry.Bytes()
	}
	files[archive.ManifestPath] = append(files[archive.ManifestPath], []byte("\n[[dependencies]]\npackage_id = \"test.publisher/m2rules\"\nversion = \"1.0.0\"\noptional = false\nfeatures = []\n")...)
	changed, e := archivefixture.Build(files, dep.ExactLock().Packages()...)
	if e != nil {
		return nil, nil, policy, e
	}
	approval := policy.Artifacts[string(root.ArtifactIdentity().Digest())]
	delete(policy.Artifacts, string(root.ArtifactIdentity().Digest()))
	policy.Artifacts[string(changed.ArtifactIdentity().Digest())] = approval
	doc, e := dep.Manifest()
	if e != nil {
		return nil, nil, policy, e
	}
	policy.Artifacts[string(dep.ArtifactIdentity().Digest())] = install.Approval{RightsDigest: install.RightsDigest(*doc.Package), Retention: "synthetic", Safety: "ACTIVE"}
	return changed, dep, policy, nil
}
func AttachCarrierDependency(plan *m2.OperatorPlan, dep *archive.Package) error {
	if plan == nil || len(plan.Games) != 1 || dep == nil {
		return m2.ErrConfiguration
	}
	g := &plan.Games[0]
	id := string(dep.ArtifactIdentity().Digest())
	g.Dependencies = []string{id}
	g.DependencyArchiveFiles = map[string]string{id: "/run/operator/dependency.zip"}
	raw, e := json.Marshal(struct {
		ID, Workspace, Root string
		Dependencies        []string
		Seats               []launch.SeatRule
		Content, Safety     []string
	}{g.Configuration, g.Workspace, g.Root, g.Dependencies, g.Seats, g.ContentTags, g.SafetyTags})
	if e != nil {
		return e
	}
	h := sha256.Sum256(append([]byte("platform-launch-configuration/v1\x00"), raw...))
	g.ConfigurationHash = "sha256:" + hex.EncodeToString(h[:])
	return g.ValidateDependencyArchives()
}

func drainProviderEvents(p *Provider) {
	for {
		select {
		case <-p.entered:
		default:
			return
		}
	}
}

func (p *Provider) PrivateMarkers() [][]byte {
	p.markerMu.Lock()
	defer p.markerMu.Unlock()
	result := [][]byte{}
	for _, b := range p.markers {
		result = append(result, append([]byte(nil), b...))
		clear(b)
	}
	p.markers = nil
	return result
}
func (b *Browser) PrivateMarkers() [][]byte {
	result := [][]byte{[]byte(b.csrf)}
	u, e := url.Parse(b.origin)
	if e == nil {
		for _, c := range b.client.Jar.Cookies(u) {
			if c.Name == httpapi.SessionCookie {
				result = append(result, []byte(c.Value))
			}
		}
	}
	if b.peer != nil {
		result = append(result, b.peer.PrivateMarkers()...)
	}
	return result
}
func modelCredentialMetadata(ctx context.Context, l Lobby, compose Compose) (string, error) {
	w, e := bindingQuote(l.Workspace)
	if e != nil {
		return "", e
	}
	r, e := bindingQuote(l.Room)
	if e != nil {
		return "", e
	}
	return sqlMetadata(ctx, compose, "SELECT md5((SELECT coalesce(string_agg(row_to_json(v)::text,'' ORDER BY row_to_json(v)::text),'') FROM platform_model.configurations v WHERE workspace_id="+w+" AND room_id="+r+") || (SELECT coalesce(string_agg(row_to_json(v)::text,'' ORDER BY row_to_json(v)::text),'') FROM platform_model.credentials v WHERE workspace_id="+w+" AND room_id="+r+"))")
}
func VerifyInstalledTenants(ctx context.Context, plan m2.OperatorPlan, compose Compose) (any, error) {
	if len(plan.Games) != 2 || len(plan.Games[0].Dependencies) != 1 || plan.Games[0].Root != plan.Games[1].Root || plan.Games[0].Dependencies[0] != plan.Games[1].Dependencies[0] || plan.Games[0].Operator == plan.Games[1].Operator || plan.Games[0].InstallCredentialFile == plan.Games[1].InstallCredentialFile {
		return nil, m2.ErrPrivate
	}
	a, e := bindingQuote(plan.Games[0].Workspace)
	if e != nil {
		return nil, e
	}
	b, e := bindingQuote(plan.Games[1].Workspace)
	if e != nil || a == b {
		return nil, m2.ErrPrivate
	}
	proof, e := sqlMetadata(ctx, compose, "SELECT (SELECT count(*)=4 AND count(DISTINCT identity)=2 AND count(DISTINCT archive_key)=2 FROM package_install.artifacts WHERE workspace IN("+a+","+b+")) AND (SELECT count(*)=4 AND count(DISTINCT principal)=2 FROM package_install.grants WHERE workspace IN("+a+","+b+")) AND NOT EXISTS(SELECT identity,path FROM package_install.objects WHERE workspace IN("+a+","+b+") GROUP BY identity,path HAVING count(DISTINCT object_key)<>1 OR count(DISTINCT workspace)<>2)")
	if e != nil || proof != "t" {
		return nil, m2.ErrPrivate
	}
	return map[string]any{"actual_authenticated_workspaces": 2, "root_and_dependency_per_workspace": 2, "separate_operator_grants_and_credentials": true, "same_immutable_CAS_objects": true}, nil
}

func knownM2Service(s string) bool {
	switch s {
	case "reverse-proxy", "platformd", "workerd", "lua-runner", "postgres", "object-storage":
		return true
	}
	return false
}
func allowedM2WriteMount(service, target string) bool {
	switch service {
	case "lua-runner":
		return target == "/run/m2/supervisor"
	case "platformd":
		return target == "/run/m2/worker"
	case "object-storage":
		return target == "/run/m2/object" || target == "/var/lib/trpg/objects"
	case "postgres":
		return target == "/var/lib/postgresql"
	}
	return false
}
func VerifyRuntimeInspect(raw []byte, source, image, project string, uid int) (any, error) {
	var containers []struct {
		ID     string `json:"Id"`
		Image  string
		Config struct {
			User   string
			Labels map[string]string
		}
		State struct {
			Running, Paused bool
			Pid             int
		}
		HostConfig struct {
			ReadonlyRootfs, Privileged    bool
			Memory, NanoCpus              int64
			PidsLimit                     *int64
			CapDrop, CapAdd, SecurityOpt  []string
			NetworkMode, PidMode, IpcMode string
		}
		Mounts []struct {
			Source, Destination string
			RW                  bool
		}
	}
	if json.Unmarshal(raw, &containers) != nil || len(containers) != 6 {
		return nil, m2.ErrPrivate
	}
	seen := map[string]bool{}
	facts := []map[string]any{}
	for _, c := range containers {
		name := c.Config.Labels["com.docker.compose.service"]
		memory := int64(512 << 20)
		if name == "lua-runner" {
			memory = 768 << 20
		}
		if name == "postgres" {
			memory = 1 << 30
		}
		expectedImage := image
		if name == "postgres" {
			expectedImage = "sha256:3a82e1f56c8f0f5616a11103ac3d47e632c3938698946a7ad26da0df1334744a"
		}
		if !knownM2Service(name) || seen[name] || c.ID == "" || !c.State.Running || c.State.Paused || c.State.Pid <= 0 || c.Image != expectedImage || c.Config.Labels["trpg.m2.source"] != source || c.Config.Labels["trpg.m2.project"] != project || c.Config.User != strconv.Itoa(uid)+":"+strconv.Itoa(uid) || c.HostConfig.ReadonlyRootfs != (name != "postgres") || c.HostConfig.Privileged || c.HostConfig.Memory != memory || c.HostConfig.NanoCpus != 1000000000 || c.HostConfig.PidsLimit == nil || *c.HostConfig.PidsLimit != 128 || len(c.HostConfig.CapAdd) != 0 || len(c.HostConfig.CapDrop) != 1 || c.HostConfig.CapDrop[0] != "ALL" || len(c.HostConfig.SecurityOpt) != 1 || c.HostConfig.SecurityOpt[0] != "no-new-privileges:true" || c.HostConfig.NetworkMode == "host" || c.HostConfig.PidMode != "" || c.HostConfig.IpcMode != "" && c.HostConfig.IpcMode != "private" {
			return nil, m2.ErrPrivate
		}
		operator := false
		for _, m := range c.Mounts {
			if m.Destination == "/run/operator" {
				operator = !m.RW
			}
			if strings.Contains(m.Source, "docker.sock") || strings.Contains(m.Destination, "docker.sock") || m.RW && !allowedM2WriteMount(name, m.Destination) {
				return nil, m2.ErrPrivate
			}
		}
		if !operator {
			return nil, m2.ErrPrivate
		}
		seen[name] = true
		status, e := os.ReadFile(fmt.Sprintf("/proc/%d/status", c.State.Pid))
		if e != nil {
			return nil, m2.ErrPrivate
		}
		effective := ""
		for _, line := range strings.Split(string(status), "\n") {
			if strings.HasPrefix(line, "Uid:") {
				parts := strings.Fields(line)
				if len(parts) == 5 && parts[1] == strconv.Itoa(uid) {
					effective = parts[2]
				}
			}
		}
		if effective != strconv.Itoa(uid) {
			return nil, m2.ErrPrivate
		}
		facts = append(facts, map[string]any{"service": name, "container": c.ID, "host_init_pid": c.State.Pid, "actual_uid": uid, "bounded_resources_and_private_mounts": true})
	}
	return facts, nil
}

// ObjectProbeFacts contains only fixed operation names and aggregate proof. No
// artifact, credential, query result body or provider material crosses stdout.
type ObjectProbeFacts struct {
	Operation                                                                           string
	Passed, ConsumersValidated, InstallerIO, ReaderIO, ResolveIO, BeforeObjectPersist   bool
	ConsumersSucceeded, ConsumersRefused, PackageDigestUnchanged, DomainDigestUnchanged bool
	Stage                                                                               string
}

// Original safe object diagnostics share the exec output stream with one JSON
// receipt. Admit only those fixed codes; any other text, duplicate receipt or
// malformed/unknown field fails. The original diagnostics remain preserved.
func DecodeObjectProbeOutput(raw []byte) (ObjectProbeFacts, error) {
	var facts ObjectProbeFacts
	if len(raw) > 4096 {
		return facts, m2.ErrPrivate
	}
	lines := bytes.Split(bytes.TrimSpace(raw), []byte("\n"))
	if len(lines) > 16 {
		return facts, m2.ErrPrivate
	}
	found := false
	for _, line := range lines {
		switch string(line) {
		case "M2_OBJECT_OWNER_BUSY", "M2_OBJECT_READ_UNAVAILABLE_OR_DENIED", "M2_OBJECT_VERIFY_UNAVAILABLE_OR_DENIED":
			continue
		}
		if found || checkpoint.StrictDecode(line, &facts, 4096) != nil {
			return ObjectProbeFacts{}, m2.ErrPrivate
		}
		found = true
	}
	if !found || (facts.Operation != "baseline" && facts.Operation != "unavailable" && facts.Operation != "corrupt") {
		return ObjectProbeFacts{}, m2.ErrPrivate
	}
	return facts, nil
}

type probeObjects struct {
	*m2.ObjectClient
	puts, reads, verifies int
}

func (o *probeObjects) Put(ctx context.Context, b []byte) (string, error) {
	o.puts++
	return o.ObjectClient.Put(ctx, b)
}
func (o *probeObjects) Read(ctx context.Context, key string) ([]byte, error) {
	o.reads++
	return o.ObjectClient.Read(ctx, key)
}
func (o *probeObjects) Verify(ctx context.Context, key string) error {
	o.verifies++
	return o.ObjectClient.Verify(ctx, key)
}

// The probe uses the unchanged original installation APIs. Its observer only
// counts actual calls into one real authenticated client, and cannot substitute
// objects, waive policy, or declare an earlier readiness failure consumer I/O.
func RunObjectConsumerProbe(ctx context.Context, config, source, project, operation string) (facts ObjectProbeFacts, failure error) {
	facts.Operation = operation
	facts.Stage = "binding"
	ctx, cancel := context.WithTimeout(ctx, 20*time.Second)
	defer cancel()
	if config != "/run/operator/config.json" || (operation != "baseline" && operation != "unavailable" && operation != "corrupt") || !strings.HasPrefix(project, "trpg-m2-") || !regexpID(project) || !checkpoint.IsDigest(source) {
		return facts, m2.ErrConfiguration
	}
	c, e := m2.LoadConfig(config)
	if e != nil || c.Source != source || c.DeploymentID != project || c.PackagesFile != "/run/operator/packages.json" || c.DSNFile != "/run/operator/dsn" || c.StagingRoot != "/run/staging" {
		return facts, m2.ErrConfiguration
	}
	plan, e := m2.ReadOperatorPlan(c.PackagesFile)
	if e != nil || plan.Classification != "TEST_ONLY_STANDARD_CARRIER" || len(plan.Games) != 2 || plan.Games[0].Workspace == plan.Games[1].Workspace {
		return facts, m2.ErrConfiguration
	}
	g := plan.Games[1]
	if g.ValidateDependencyArchives() != nil || len(g.Dependencies) != 1 || !store.ValidID(g.Workspace) || !store.ValidID(g.Operator) || !store.ValidID(g.InstallID) || g.InstallCredentialFile != "/run/operator/install-credential-second" || !checkpoint.IsDigest(g.Root) {
		return facts, m2.ErrConfiguration
	}
	credentialBytes, e := m2.ReadSecret(g.InstallCredentialFile, 256)
	if e != nil {
		return facts, e
	}
	credential := store.Credential(string(credentialBytes))
	clear(credentialBytes)
	access, e := store.NewAccess(map[store.Credential][]store.Membership{credential: {{Workspace: g.Workspace, Principal: g.Operator, Install: true, Read: true}}})
	if e != nil {
		return facts, e
	}
	policy, e := install.NewPolicy(plan.InstallPolicy)
	if e != nil {
		return facts, e
	}
	client, e := m2.NewObjectClient(c)
	if e != nil {
		return facts, e
	}
	defer client.Close()
	objects := &probeObjects{ObjectClient: client}
	dsn, e := m2.ReadSecret(c.DSNFile, 16384)
	if e != nil {
		return facts, e
	}
	defer clear(dsn)
	repo, e := postgres.OpenInstallationRepository(ctx, string(dsn), objects, extension.DefaultSupport, nil)
	if e != nil {
		return facts, e
	}
	defer func() { failure = errors.Join(failure, repo.Close()) }()
	// Lookups retain the repository's original current workspace/principal
	// guard. Exact request recovery below proves the committed fingerprint via
	// actual object I/O; no raw SQL handle is opened in this test helper.
	facts.Stage = "committed-proof"
	committed, e := repo.Lookup(ctx, g.Workspace, g.Operator, g.Root)
	if e != nil || committed.Identity != g.Root || !checkpoint.IsDigest(committed.ArchiveKey) {
		return facts, m2.ErrPrivate
	}
	rootRaw, e := probeArchive(ctx, g.ArchiveFile, g.Root)
	if e != nil {
		return facts, e
	}
	defer clear(rootRaw)
	depRaw, e := probeArchive(ctx, g.DependencyArchiveFiles[g.Dependencies[0]], g.Dependencies[0])
	if e != nil {
		return facts, e
	}
	defer clear(depRaw)
	rootPkg, e := archive.ImportBytes(rootRaw, extension.DefaultSupport)
	if e != nil {
		return facts, e
	}
	depPkg, e := archive.ImportBytes(depRaw, extension.DefaultSupport)
	if e != nil {
		return facts, e
	}
	rootSource, ok := rootPkg.SourceArchiveHash()
	if !ok {
		return facts, m2.ErrPrivate
	}
	depSource, ok := depPkg.SourceArchiveHash()
	if !ok {
		return facts, m2.ErrPrivate
	}
	encoded, e := json.Marshal([]string{policy.Digest(), g.Workspace, g.Operator, string(rootSource), g.Root, string(depSource), g.Dependencies[0]})
	if e != nil || string(rootSource) != committed.SourceArchiveHash {
		return facts, m2.ErrPrivate
	}
	fingerprint := object.Hash(encoded)
	launcher, e := m2.NewSupervisorLauncher(c)
	if e != nil {
		return facts, e
	}
	defer launcher.Close()
	observedPersist := false
	installer, e := install.New(install.Options{StagingRoot: c.StagingRoot, Policy: policy, Access: access, Objects: objects, Repository: repo, Support: extension.DefaultSupport, Runtime: install.RuntimeConfig{Runner: c.Runner, SHA256: c.RunnerHash, Limits: c.Limits, Launcher: launcher}, Observe: func(stage string) error {
		if stage == "before-object-persist" {
			observedPersist = true
		}
		return nil
	}, Execution: func(install.Execution) error { return nil }})
	if e != nil {
		return facts, e
	}
	reader, e := store.NewReader(repo, objects, access, extension.DefaultSupport)
	if e != nil {
		return facts, e
	}
	facts.Stage = "installer"
	request := install.Request{Credential: credential, Workspace: g.Workspace, ID: g.InstallID, Root: install.Input{Archive: bytes.NewReader(rootRaw), Evidence: g.Evidence[g.Root]}, Dependencies: []install.Input{{Archive: bytes.NewReader(depRaw), Evidence: g.Evidence[g.Dependencies[0]]}}}
	if operation == "unavailable" {
		request.ID = "m2-probe-unavailable"
		request.Root = install.Input{Archive: bytes.NewReader(depRaw), Evidence: g.Evidence[g.Dependencies[0]]}
		request.Dependencies = nil
	}
	puts, reads := objects.puts, objects.reads
	_, installError := installer.Install(ctx, request)
	facts.BeforeObjectPersist = observedPersist
	facts.InstallerIO = objects.puts > puts || objects.reads > reads
	if operation == "unavailable" && (!observedPersist || objects.puts != puts+1) {
		return facts, m2.ErrPrivate
	}
	facts.Stage = "reader"
	reads = objects.reads
	_, readerError := reader.Load(ctx, credential, g.Workspace, g.Root)
	facts.ReaderIO = objects.reads > reads
	facts.Stage = "resolve"
	reads = objects.reads
	_, resolveError := repo.Resolve(ctx, g.Workspace, g.Operator, g.InstallID, fingerprint)
	facts.ResolveIO = objects.reads > reads
	facts.ConsumersSucceeded = installError == nil && readerError == nil && resolveError == nil
	facts.ConsumersRefused = installError != nil && readerError != nil && resolveError != nil
	// The outer owned lifecycle takes bounded original PostgreSQL aggregate
	// digests before/after this exec and proves no domain/provider writes. This
	// helper claims only the actual original consumer results.
	facts.ConsumersValidated = facts.InstallerIO && facts.ReaderIO && facts.ResolveIO && (operation == "baseline" && facts.ConsumersSucceeded || operation != "baseline" && facts.ConsumersRefused) && ctx.Err() == nil

	if !facts.ConsumersValidated {
		return facts, m2.ErrPrivate
	}
	facts.Stage = "complete"
	return facts, nil
}
func probeArchive(ctx context.Context, path, id string) ([]byte, error) {
	if ctx.Err() != nil || !filepath.IsAbs(path) || !strings.HasPrefix(path, "/run/operator/") || !checkpoint.IsDigest(id) {
		return nil, m2.ErrConfiguration
	}
	for parent := filepath.Dir(path); ; parent = filepath.Dir(parent) {
		v, e := os.Lstat(parent)
		if e != nil || !v.IsDir() || v.Mode()&os.ModeSymlink != 0 {
			return nil, m2.ErrConfiguration
		}
		if parent == "/" {
			break
		}
	}
	info, e := os.Lstat(path)
	if e != nil || !info.Mode().IsRegular() || info.Mode().Perm() != 0400 && info.Mode().Perm() != 0440 || info.Size() <= 0 || info.Size() > 80<<20 {
		return nil, m2.ErrConfiguration
	}
	f, e := os.Open(path)
	if e != nil {
		return nil, e
	}
	defer f.Close()
	opened, e := f.Stat()
	if e != nil || !os.SameFile(info, opened) || info.Mode() != opened.Mode() || info.Size() != opened.Size() {
		return nil, m2.ErrConfiguration
	}
	raw, e := io.ReadAll(io.LimitReader(f, 80<<20+1))
	after, ae := os.Lstat(path)
	current, ce := f.Stat()
	if e != nil || ae != nil || ce != nil || int64(len(raw)) != info.Size() || !os.SameFile(info, after) || !os.SameFile(info, current) || after.Mode() != info.Mode() || current.Mode() != info.Mode() || !after.ModTime().Equal(info.ModTime()) || !current.ModTime().Equal(info.ModTime()) || ctx.Err() != nil {
		clear(raw)
		return nil, m2.ErrConfiguration
	}
	pkg, e := archive.ImportBytes(raw, extension.DefaultSupport)
	if e != nil || string(pkg.ArtifactIdentity().Digest()) != id {
		clear(raw)
		return nil, m2.ErrConfiguration
	}
	return raw, nil
}

// ObjectProbeMetadata reads only bounded aggregate evidence through the
// existing owned PostgreSQL CLI. It does not create a new SQL import/storage
// authority or repair missing/occupied fixture state.
func ObjectProbeMetadata(ctx context.Context, plan m2.OperatorPlan, compose Compose) (string, string, error) {
	if plan.Classification != "TEST_ONLY_STANDARD_CARRIER" || len(plan.Games) != 2 || plan.Games[0].Workspace == plan.Games[1].Workspace {
		return "", "", m2.ErrPrivate
	}
	w, e := bindingQuote(plan.Games[1].Workspace)
	if e != nil {
		return "", "", e
	}
	unused, e := sqlMetadata(ctx, compose, "SELECT NOT EXISTS(SELECT 1 FROM host_command.sessions WHERE workspace="+w+") AND NOT EXISTS(SELECT 1 FROM platform_task.jobs WHERE workspace="+w+") AND NOT EXISTS(SELECT 1 FROM package_install.history_pins WHERE workspace="+w+") AND NOT EXISTS(SELECT 1 FROM package_install.data_targets WHERE workspace="+w+")")
	if e != nil || unused != "t" {
		return "", "", m2.ErrPrivate
	}
	groups := [][]string{{"package_install.workspaces", "package_install.data_targets", "package_install.history_pins", "package_install.artifacts", "package_install.objects", "package_install.grants", "package_install.requests", "package_install.request_artifacts"}, {"host_command.sessions", "host_command.requests", "host_command.events", "host_command.tasks", "host_command.continuations", "platform_task.jobs", "platform_budget.tasks", "platform_budget.reservations", "platform_budget.counters", "platform_budget.pauses", "platform_model.configurations"}}
	digests := []string{}
	for _, tables := range groups {
		queries := []string{}
		for _, table := range tables {
			queries = append(queries, "SELECT '"+table+"' AS n,md5(coalesce(jsonb_agg(j ORDER BY j::text),'[]'::jsonb)::text) AS d FROM (SELECT to_jsonb(t) AS j FROM "+table+" t) v WHERE j->>'workspace'="+w+" OR j->>'workspace_id'="+w)
		}
		digest, e := sqlMetadata(ctx, compose, "SELECT md5(string_agg(d,'' ORDER BY n)) FROM ("+strings.Join(queries, " UNION ALL ")+") proofs")
		if e != nil || len(digest) != 32 {
			return "", "", m2.ErrPrivate
		}
		digests = append(digests, digest)
	}
	return digests[0], digests[1], nil
}

// A daemon birth is observed from the kernel, not a relay PID or Wait claim.
// Its full owned container and immutable image are revalidated before signal.
type daemonBirth struct {
	Container, Image, Project, Source, Executable, Namespace, StartTime, CgroupHash, BinaryHash string
	PID, Parent, InitPID                                                                        int
}

func daemonContainer(ctx context.Context, container string, command Command) (daemonBirth, error) {
	raw, e := command(ctx, "daemon-container-binding", nil, "docker", "inspect", "--format", `{"container":{{json .Id}},"image":{{json .Image}},"project":{{json (index .Config.Labels "trpg.m2.project")}},"source":{{json (index .Config.Labels "trpg.m2.source")}},"init_pid":{{.State.Pid}},"running":{{.State.Running}}}`, container)
	var v struct {
		Container, Image, Project, Source string
		InitPID                           int `json:"init_pid"`
		Running                           bool
	}
	if e != nil || checkpoint.StrictDecode(raw, &v, 2048) != nil || v.Container != container || !v.Running || v.InitPID < 1 || !checkpoint.IsDigest(v.Image) {
		return daemonBirth{}, m2.ErrPrivate
	}
	return daemonBirth{Container: v.Container, Image: v.Image, Project: v.Project, Source: v.Source, InitPID: v.InitPID}, nil
}
func daemonKernel(v daemonBirth) (daemonBirth, string, error) {
	stat, e := os.ReadFile(fmt.Sprintf("/proc/%d/stat", v.PID))
	i := strings.LastIndexByte(string(stat), ')')
	if e != nil || i < 0 {
		return v, "", m2.ErrPrivate
	}
	f := strings.Fields(string(stat[i+1:]))
	if len(f) < 20 {
		return v, "", m2.ErrPrivate
	}
	parent, e := strconv.Atoi(f[1])
	if e != nil {
		return v, "", m2.ErrPrivate
	}
	ns, e := os.Readlink(fmt.Sprintf("/proc/%d/ns/pid", v.PID))
	initNS, ie := os.Readlink(fmt.Sprintf("/proc/%d/ns/pid", v.InitPID))
	ownNS, oe := os.Readlink("/proc/self/ns/pid")
	childGroup, ce := os.ReadFile(fmt.Sprintf("/proc/%d/cgroup", v.PID))
	initGroup, ge := os.ReadFile(fmt.Sprintf("/proc/%d/cgroup", v.InitPID))
	parentGroup, pe := os.ReadFile(fmt.Sprintf("/proc/%d/cgroup", parent))
	bin, be := os.ReadFile(fmt.Sprintf("/proc/%d/exe", v.PID))
	expected, ee := os.ReadFile(fmt.Sprintf("/proc/%d/root%s", v.InitPID, v.Executable))
	status, se := os.ReadFile(fmt.Sprintf("/proc/%d/status", v.PID))
	uidOK := false
	for _, line := range strings.Split(string(status), "\n") {
		if strings.HasPrefix(line, "Uid:") {
			u := strings.Fields(line)
			uidOK = len(u) == 5
			for _, id := range u[1:] {
				uidOK = uidOK && id == strconv.Itoa(os.Geteuid())
			}
		}
	}
	if e != nil || ie != nil || oe != nil || ce != nil || ge != nil || pe != nil || be != nil || ee != nil || se != nil || !uidOK || ns != initNS || ns == ownNS || !bytes.Equal(childGroup, initGroup) || !bytes.Equal(childGroup, parentGroup) || !bytes.Contains(childGroup, []byte(v.Container)) || !bytes.Equal(bin, expected) {
		return v, "", m2.ErrPrivate
	}
	v.Parent = parent
	v.Namespace = ns
	v.StartTime = f[19]
	v.CgroupHash = object.Hash(childGroup)
	v.BinaryHash = object.Hash(bin)
	return v, f[0], nil
}
func observeDaemon(ctx context.Context, c m2.Config, service, executable string, compose Compose, command Command) (daemonBirth, error) {
	expected := map[string]string{"workerd": "/app/workerd", "platformd": "/app/platformd", "lua-runner": "/app/lua-supervisord"}
	if expected[service] == "" || expected[service] != executable {
		return daemonBirth{}, m2.ErrPrivate
	}
	raw, e := compose(ctx, "ps", "--quiet", service)
	container := strings.TrimSpace(string(raw))
	if e != nil || len(container) != 64 || !checkpoint.IsDigest("sha256:"+container) {
		return daemonBirth{}, m2.ErrPrivate
	}
	v, e := daemonContainer(ctx, container, command)
	if e != nil || v.Project != c.DeploymentID || v.Source != c.Source {
		return daemonBirth{}, m2.ErrPrivate
	}
	v.Executable = executable
	raw, e = command(ctx, "daemon-kernel-top", nil, "docker", "top", container, "-eo", "pid,ppid,args")
	if e != nil {
		return v, e
	}
	count := 0
	for _, line := range strings.Split(string(raw), "\n") {
		f := strings.Fields(line)
		if len(f) < 3 || f[2] != executable {
			continue
		}
		pid, e := strconv.Atoi(f[0])
		if e != nil {
			return v, m2.ErrPrivate
		}
		v.PID = pid
		count++
	}
	if count != 1 {
		return v, m2.ErrPrivate
	}
	v, _, e = daemonKernel(v)
	return v, e
}
func revalidateDaemon(ctx context.Context, v daemonBirth, command Command) (string, error) {
	current, e := daemonContainer(ctx, v.Container, command)
	if e != nil || current.Image != v.Image || current.Source != v.Source || current.Project != v.Project || current.InitPID != v.InitPID {
		return "", m2.ErrPrivate
	}
	current.PID = v.PID
	current.Executable = v.Executable
	current, state, e := daemonKernel(current)
	if e != nil || current.Parent != v.Parent || current.Namespace != v.Namespace || current.StartTime != v.StartTime || current.CgroupHash != v.CgroupHash || current.BinaryHash != v.BinaryHash {
		return "", m2.ErrPrivate
	}
	return state, nil
}

func observeOwnedRunners(ctx context.Context, supervisor daemonBirth, runnerHash string, command Command) ([]daemonBirth, error) {
	if _, e := revalidateDaemon(ctx, supervisor, command); e != nil {
		return nil, e
	}
	raw, e := command(ctx, "active-platform-runner-kernel-top", nil, "docker", "top", supervisor.Container, "-eo", "pid,ppid,args")
	if e != nil {
		return nil, e
	}
	children := []daemonBirth{}
	for _, line := range strings.Split(string(raw), "\n") {
		f := strings.Fields(line)
		if len(f) < 3 || f[2] != "/app/lua-runner" {
			continue
		}
		pid, e := strconv.Atoi(f[0])
		if e != nil {
			return nil, m2.ErrPrivate
		}
		v := supervisor
		v.PID = pid
		v.Executable = "/app/lua-runner"
		v, _, e = daemonKernel(v)
		if e != nil || v.Parent != supervisor.PID || v.Namespace != supervisor.Namespace || v.CgroupHash != supervisor.CgroupHash || v.BinaryHash != runnerHash {
			return nil, m2.ErrPrivate
		}
		children = append(children, v)
		if len(children) > 32 {
			return nil, m2.ErrPrivate
		}
	}
	if len(children) == 0 {
		return nil, m2.ErrPrivate
	}
	return children, nil
}

// This operation is available only in the private same-source owned test helper.
// It is not part of workerd's CLI or any authenticated service protocol.
type WorkerdStopBinding struct {
	NamespacePID                     int
	Namespace, StartTime, BinaryHash string
}
type WorkerdStopFacts struct {
	Code                                 string
	Passed, ObservedT, CurrentBirthBound bool
}

func RunTestOnlyWorkerdStop(ctx context.Context, config, source, project string, input io.Reader) (facts WorkerdStopFacts, failure error) {
	facts.Code = "BINDING_REJECTED"
	ctx, cancel := context.WithTimeout(ctx, 2*time.Second)
	defer cancel()
	if m2.RequireLinux() != nil || config != "/run/operator/config.json" || !checkpoint.IsDigest(source) || !strings.HasPrefix(project, "trpg-m2-") || !regexpID(project) {
		return facts, m2.ErrPrivate
	}
	c, e := m2.LoadConfig(config)
	if e != nil || c.DeploymentID != project || c.Source != source || c.PeerUID != uint32(os.Geteuid()) {
		return facts, m2.ErrPrivate
	}
	classification, e := m2.ReadSecret("/run/operator/test-only-classification", 128)
	defer clear(classification)
	if e != nil || string(classification) != "TEST_ONLY_STANDARD_CARRIER" {
		return facts, m2.ErrPrivate
	}
	raw, e := io.ReadAll(io.LimitReader(input, 4097))
	if e != nil || len(raw) > 4096 {
		return facts, m2.ErrPrivate
	}
	var binding WorkerdStopBinding
	if checkpoint.StrictDecode(raw, &binding, 4096) != nil || binding.NamespacePID <= 1 || binding.NamespacePID > 1<<22 || binding.StartTime == "" || len(binding.StartTime) > 20 || !checkpoint.IsDigest(binding.BinaryHash) {
		return facts, m2.ErrPrivate
	}
	ownNS, e := os.Readlink("/proc/self/ns/pid")
	if e != nil || ownNS != binding.Namespace {
		return facts, m2.ErrPrivate
	}
	expected, e := os.ReadFile("/app/workerd")
	if e != nil || object.Hash(expected) != binding.BinaryHash {
		return facts, m2.ErrPrivate
	}
	verify := func() (string, error) {
		stat, e := os.ReadFile(fmt.Sprintf("/proc/%d/stat", binding.NamespacePID))
		i := strings.LastIndexByte(string(stat), ')')
		if e != nil || i < 0 {
			return "", m2.ErrPrivate
		}
		fields := strings.Fields(string(stat[i+1:]))
		if len(fields) < 20 || fields[19] != binding.StartTime || fields[1] != "1" {
			return "", m2.ErrPrivate
		}
		ns, e := os.Readlink(fmt.Sprintf("/proc/%d/ns/pid", binding.NamespacePID))
		bin, be := os.ReadFile(fmt.Sprintf("/proc/%d/exe", binding.NamespacePID))
		exe, ee := os.Readlink(fmt.Sprintf("/proc/%d/exe", binding.NamespacePID))
		group, ge := os.ReadFile(fmt.Sprintf("/proc/%d/cgroup", binding.NamespacePID))
		ownGroup, oe := os.ReadFile("/proc/self/cgroup")
		initGroup, ie := os.ReadFile("/proc/1/cgroup")
		status, se := os.ReadFile(fmt.Sprintf("/proc/%d/status", binding.NamespacePID))
		uidOK := false
		for _, line := range strings.Split(string(status), "\n") {
			if strings.HasPrefix(line, "Uid:") {
				u := strings.Fields(line)
				uidOK = len(u) == 5
				for _, id := range u[1:] {
					uidOK = uidOK && id == strconv.Itoa(os.Geteuid())
				}
			}
		}
		if e != nil || be != nil || ee != nil || ge != nil || oe != nil || ie != nil || se != nil || !uidOK || exe != "/app/workerd" || ns != binding.Namespace || !bytes.Equal(group, ownGroup) || !bytes.Equal(group, initGroup) || object.Hash(bin) != binding.BinaryHash {
			return "", m2.ErrPrivate
		}
		return fields[0], nil
	}
	process, e := os.FindProcess(binding.NamespacePID)
	if e != nil {
		return facts, m2.ErrPrivate
	}
	defer process.Release()
	if _, e = verify(); e != nil {
		return facts, e
	}
	facts.CurrentBirthBound = true
	if e = process.Signal(syscall.SIGSTOP); e != nil {
		facts.Code = "SIGSTOP_FAILED"
		if errors.Is(e, syscall.EACCES) {
			facts.Code = "EACCES"
		}
		return facts, e
	}
	until := time.Now().Add(500 * time.Millisecond)
	for {
		state, e := verify()
		if e != nil {
			return facts, e
		}
		if state == "T" {
			facts.Code = "OBSERVED_T"
			facts.ObservedT = true
			facts.Passed = true
			return facts, nil
		}
		if time.Now().After(until) {
			facts.Code = "STATE_NOT_T"
			return facts, m2.ErrPrivate
		}
		select {
		case <-ctx.Done():
			facts.Code = "CONTEXT_EXPIRED"
			return facts, ctx.Err()
		case <-time.After(5 * time.Millisecond):
		}
	}
}

func workerdStopBinding(v daemonBirth) (WorkerdStopBinding, error) {
	if v.Executable != "/app/workerd" || v.Parent != v.InitPID {
		return WorkerdStopBinding{}, m2.ErrPrivate
	}
	status, e := os.ReadFile(fmt.Sprintf("/proc/%d/status", v.PID))
	if e != nil {
		return WorkerdStopBinding{}, e
	}
	pid := 0
	for _, line := range strings.Split(string(status), "\n") {
		if strings.HasPrefix(line, "NSpid:") {
			f := strings.Fields(line)
			if len(f) < 2 {
				return WorkerdStopBinding{}, m2.ErrPrivate
			}
			pid, _ = strconv.Atoi(f[len(f)-1])
		}
	}
	if pid <= 1 {
		return WorkerdStopBinding{}, m2.ErrPrivate
	}
	return WorkerdStopBinding{NamespacePID: pid, Namespace: v.Namespace, StartTime: v.StartTime, BinaryHash: v.BinaryHash}, nil
}
