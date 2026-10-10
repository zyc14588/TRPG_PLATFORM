//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player_test

import (
	"context"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/gorilla/websocket"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/httpapi"
	playerapi "github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
)

type browserServer struct {
	*playerFixture
	drop          atomic.Bool
	commands      atomic.Int32
	resumeLimited atomic.Int32
}

// The browser trace deliberately records no body, query, identity, cookie or
// request headers. It is sufficient to diagnose production rate windows.
type tracedResponse struct {
	http.ResponseWriter
	status int
	code   string
}

func (w *tracedResponse) Unwrap() http.ResponseWriter { return w.ResponseWriter }

type capturedResponse struct {
	*httptest.ResponseRecorder
	underlying http.ResponseWriter
}

func (w *capturedResponse) Unwrap() http.ResponseWriter { return w.underlying }

func (w *tracedResponse) WriteHeader(status int) {
	w.status = status
	w.ResponseWriter.WriteHeader(status)
}

func (w *tracedResponse) Write(body []byte) (int, error) {
	if w.status == 0 {
		w.WriteHeader(http.StatusOK)
	}
	if w.status >= 400 {
		var envelope struct {
			Error struct {
				Code string `json:"code"`
			} `json:"error"`
		}
		if json.Unmarshal(body, &envelope) == nil {
			switch envelope.Error.Code {
			case "INVALID_REQUEST", "UNAUTHENTICATED", "DENIED", "CONFLICT", "CLAIM_REQUIRED", "RATE_LIMITED", "UNAVAILABLE", "OUTCOME_UNKNOWN", "PLAYER_PAUSED", "PLAYER_CONNECTION_EXPIRED":
				w.code = envelope.Error.Code
			}
		}
	}
	return w.ResponseWriter.Write(body)
}

func tracePath(path string) string {
	parts := strings.Split(path, "/")
	for i := 1; i < len(parts); i++ {
		if i > 0 && (parts[i-1] == "workspaces" || parts[i-1] == "rooms" || parts[i-1] == "games" || parts[i-1] == "admissions" || parts[i-1] == "room-admissions" || parts[i-1] == "participants") {
			parts[i] = "{id}"
		}
	}
	return strings.Join(parts, "/")
}

func serveBrowser(t *testing.T, n *playerFixture) *browserServer {
	t.Helper()
	if _, e := os.Stat("../../../apps/web-player/dist/index.html"); e != nil {
		t.Fatal("built player required; browser NOT_RUN")
	}
	n.server.Close()
	m, ok := n.models.(*model.Service)
	if !ok {
		t.Fatal("real model composition required")
	}
	source, e := model.NewPresentationSource(m, map[string]string{n.w + "/selected": "本机测试 AI"})
	need(t, e)
	schema, e := os.ReadFile("../../../schemas/platform/platform-player-presentation-api-v1.schema.json")
	need(t, e)
	facade, e := playerapi.NewPresentationService(n.players, source, schema)
	need(t, e)
	s := httptest.NewUnstartedServer(nil)
	handler, e := httpapi.NewPlayerPresentationHandler(n.players, n.rooms, facade, "https://"+s.Listener.Addr().String())
	need(t, e)
	b := &browserServer{playerFixture: n}
	var requests atomic.Int32
	started := time.Now()
	static := http.FileServer(http.Dir("../../../apps/web-player/dist"))
	s.Config.Handler = http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if strings.HasPrefix(r.URL.Path, "/api/") {
			count := requests.Add(1)
			traced := &tracedResponse{ResponseWriter: w}
			w = traced
			defer func() {
				if strings.HasSuffix(r.URL.Path, "/session/resume") && traced.status == http.StatusTooManyRequests && traced.code == "RATE_LIMITED" {
					b.resumeLimited.Add(1)
				}
				t.Logf("browser-http method=%s path=%s status=%d code=%s count=%d elapsed_ms=%d", r.Method, tracePath(r.URL.Path), traced.status, traced.code, count, time.Since(started).Milliseconds())
			}()
			if strings.HasSuffix(r.URL.Path, "/session/commands") {
				b.commands.Add(1)
				if b.drop.CompareAndSwap(true, false) {
					rec := &capturedResponse{ResponseRecorder: httptest.NewRecorder(), underlying: w}
					handler.ServeHTTP(rec, r)
					for k, values := range rec.Header() {
						for _, v := range values {
							w.Header().Add(k, v)
						}
					}
					w.WriteHeader(rec.Code)
					_, _ = w.Write([]byte("{"))
					return
				}
			}
			handler.ServeHTTP(w, r)
			return
		}
		w.Header().Set("Cache-Control", "no-store")
		w.Header().Set("Referrer-Policy", "no-referrer")
		w.Header().Set("Content-Security-Policy", "default-src 'self'; connect-src 'self'; img-src 'self' data:; style-src 'self'; script-src 'self'; frame-ancestors 'none'; base-uri 'none'")
		static.ServeHTTP(w, r)
	})
	s.Config.ErrorLog = log.New(io.Discard, "", 0)
	s.StartTLS()
	n.server = s
	t.Cleanup(s.Close)
	return b
}

type chrome struct {
	ctx     context.Context
	socket  *websocket.Conn
	session string
	id      int
	url     string
}

func newChrome(t *testing.T, s *httptest.Server) *chrome {
	t.Helper()
	path := os.Getenv("M2B010_CHROME")
	if path == "" {
		path = "/usr/bin/google-chrome"
	}
	version, e := exec.Command(path, "--version").Output()
	if e != nil || !strings.Contains(string(version), "Google Chrome") {
		t.Fatal("installed Linux Chrome required; browser NOT_RUN")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Minute)
	t.Cleanup(cancel)
	dir := t.TempDir()
	pin := sha256.Sum256(s.Certificate().RawSubjectPublicKeyInfo)
	cmd := exec.CommandContext(ctx, path, "--headless=new", "--no-sandbox", "--disable-dev-shm-usage", "--disable-gpu", "--no-first-run", "--no-default-browser-check", "--disable-background-networking", "--disable-features=Translate", "--remote-debugging-port=0", "--user-data-dir="+dir, "--ignore-certificate-errors-spki-list="+base64.StdEncoding.EncodeToString(pin[:]), "about:blank")
	cmd.Stdout = io.Discard
	cmd.Stderr = io.Discard
	if e = cmd.Start(); e != nil {
		t.Fatal("Chrome start failed; browser NOT_RUN")
	}
	t.Cleanup(func() { _ = cmd.Process.Kill(); _ = cmd.Wait() })
	var lines []string
	deadline := time.Now().Add(12 * time.Second)
	for time.Now().Before(deadline) {
		raw, e := os.ReadFile(filepath.Join(dir, "DevToolsActivePort"))
		if e == nil {
			lines = strings.Split(strings.TrimSpace(string(raw)), "\n")
			if len(lines) == 2 {
				break
			}
		}
		time.Sleep(25 * time.Millisecond)
	}
	if len(lines) != 2 {
		t.Fatal("Chrome debugging transport unavailable; browser NOT_RUN")
	}
	ws, _, e := websocket.DefaultDialer.DialContext(ctx, "ws://127.0.0.1:"+lines[0]+lines[1], nil)
	if e != nil {
		t.Fatal("Chrome transport failed")
	}
	t.Cleanup(func() { _ = ws.Close() })
	c := &chrome{ctx: ctx, socket: ws, url: s.URL}
	var created struct {
		TargetID string `json:"targetId"`
	}
	c.call(t, "Target.createTarget", map[string]any{"url": "about:blank"}, &created)
	var attached struct {
		SessionID string `json:"sessionId"`
	}
	c.call(t, "Target.attachToTarget", map[string]any{"targetId": created.TargetID, "flatten": true}, &attached)
	c.session = attached.SessionID
	c.call(t, "Page.enable", map[string]any{}, nil)
	c.call(t, "Runtime.enable", map[string]any{}, nil)
	c.call(t, "Page.navigate", map[string]any{"url": s.URL}, nil)
	c.wait(t, "document.querySelector('button') !== null")
	c.waitText(t, "登录账户")
	t.Logf("actual browser: %s; TLS certificate pinned; own profile", strings.TrimSpace(string(version)))
	return c
}
func (c *chrome) call(t *testing.T, method string, params any, out any) {
	t.Helper()
	c.id++
	id := c.id
	request := map[string]any{"id": id, "method": method, "params": params}
	if c.session != "" {
		request["sessionId"] = c.session
	}
	_ = c.socket.SetWriteDeadline(time.Now().Add(15 * time.Second))
	if c.socket.WriteJSON(request) != nil {
		t.Fatal("Chrome command write failed")
	}
	_ = c.socket.SetReadDeadline(time.Now().Add(20 * time.Second))
	for {
		var reply struct {
			ID     int             `json:"id"`
			Result json.RawMessage `json:"result"`
			Error  json.RawMessage `json:"error"`
		}
		if c.socket.ReadJSON(&reply) != nil {
			t.Fatal("Chrome command response failed")
		}
		if reply.ID != id {
			continue
		}
		if len(reply.Error) > 0 {
			t.Fatal("Chrome command rejected; private details withheld")
		}
		if out != nil && json.Unmarshal(reply.Result, out) != nil {
			t.Fatal("Chrome response invalid")
		}
		return
	}
}
func (c *chrome) evaluate(t *testing.T, expression string) json.RawMessage {
	t.Helper()
	var reply struct {
		Result struct {
			Value json.RawMessage `json:"value"`
		} `json:"result"`
		Exception json.RawMessage `json:"exceptionDetails"`
	}
	c.call(t, "Runtime.evaluate", map[string]any{"expression": expression, "awaitPromise": true, "returnByValue": true}, &reply)
	if len(reply.Exception) > 0 {
		t.Fatal("browser operation failed; private details withheld")
	}
	return reply.Result.Value
}
func (c *chrome) truth(t *testing.T, expression string) bool {
	var ok bool
	_ = json.Unmarshal(c.evaluate(t, expression), &ok)
	return ok
}
func (c *chrome) wait(t *testing.T, condition string) {
	t.Helper()
	deadline := time.Now().Add(14 * time.Second)
	for time.Now().Before(deadline) {
		if c.truth(t, condition) {
			return
		}
		time.Sleep(60 * time.Millisecond)
	}
	t.Fatalf("browser condition timed out: %s (safe alert %s)", condition, c.evaluate(t, "document.querySelector('[role=alert]')?.textContent ?? 'none'"))
}
func js(v any) string { raw, _ := json.Marshal(v); return string(raw) }
func (c *chrome) waitText(t *testing.T, value string) {
	t.Helper()
	c.wait(t, "document.body.innerText.includes("+js(value)+")")
}
func (c *chrome) button(t *testing.T, label string) {
	t.Helper()
	c.wait(t, "Array.from(document.querySelectorAll('button')).some(b=>b.textContent==="+js(label)+"&&!b.disabled&&!b.closest('fieldset:disabled'))")
	c.evaluate(t, "Array.from(document.querySelectorAll('button')).find(b=>b.textContent==="+js(label)+").click();true")
}
func (c *chrome) input(t *testing.T, selector, value string) {
	t.Helper()
	c.wait(t, "document.querySelector("+js(selector)+")!==null")
	c.evaluate(t, "(()=>{const e=document.querySelector("+js(selector)+"); const proto=e.tagName==='SELECT'?HTMLSelectElement.prototype:e.tagName==='TEXTAREA'?HTMLTextAreaElement.prototype:HTMLInputElement.prototype;Object.getOwnPropertyDescriptor(proto,'value').set.call(e,"+js(value)+");e.dispatchEvent(new Event('input',{bubbles:true}));e.dispatchEvent(new Event('change',{bubbles:true}));return true})()")
}
func (c *chrome) check(t *testing.T, name string) {
	t.Helper()
	c.evaluate(t, "(()=>{const e=document.querySelector('input[name="+name+"]');if(!e.checked)e.click();return true})()")
}
func (c *chrome) login(t *testing.T, name string) {
	t.Helper()
	c.input(t, "input[name=login]", name)
	c.input(t, "input[name=password]", "synthetic-claim-password")
	c.button(t, "登录账户")
	c.waitText(t, "退出登录")
}
func (c *chrome) open(t *testing.T, w, r string) {
	t.Helper()
	c.evaluate(t, "history.replaceState(null,'','#room/"+w+"/"+r+"');location.reload();true")
	c.waitText(t, "打开房间")
	c.button(t, "打开房间")
	c.waitText(t, "刷新房间与准备")
}
func (c *chrome) consent(t *testing.T) {
	t.Helper()
	c.waitText(t, "保存我的同意与准备")
	for _, name := range []string{"consent", "safety", "ready"} {
		c.check(t, name)
	}
	c.button(t, "保存我的同意与准备")
	c.wait(t, "!document.querySelector('fieldset:disabled')")
}
func (c *chrome) counter(t *testing.T, want string) {
	t.Helper()
	c.wait(t, "(()=>{const root=document.querySelector('[data-testid=game-view]');if(!root)return false;const dt=Array.from(root.querySelectorAll('dt')).find(e=>e.textContent==='counter');return dt?.nextElementSibling?.textContent==="+js(want)+"})()")
}
func (c *chrome) assertNoPrivate(t *testing.T) {
	t.Helper()
	if c.truth(t, "document.body.innerText.includes("+js(PrivateValue)+")") {
		t.Fatal("private GM state reached ordinary participant DOM")
	}
	if !c.truth(t, "localStorage.length===0&&sessionStorage.length===0&&!document.cookie.includes('__Host-trpg_session')") {
		t.Fatal("browser exposed or persisted protected state")
	}
}
func (c *chrome) action(t *testing.T) {
	t.Helper()
	c.input(t, "input[name=action]", "increment")
	c.input(t, "input[aria-label='选项名称 1']", "delta")
	c.input(t, "select[aria-label='选项类型 1']", "number")
	c.input(t, "input[aria-label='选项内容 1']", "1")
	c.button(t, "提交行动")
}
func (c *chrome) screen(t *testing.T, name string) {
	t.Helper()
	root := os.Getenv("M2B010_EVIDENCE_DIR")
	if root == "" {
		return
	}
	var reply struct {
		Data string `json:"data"`
	}
	c.call(t, "Page.captureScreenshot", map[string]any{"format": "png"}, &reply)
	data, e := base64.StdEncoding.DecodeString(reply.Data)
	if e != nil {
		t.Fatal("screenshot decoding failed")
	}
	if e = os.WriteFile(filepath.Join(root, name+".png"), data, 0600); e != nil {
		t.Fatal("screenshot storage failed")
	}
}

func (c *chrome) download(t *testing.T) {
	t.Helper()
	directory := t.TempDir()
	c.call(t, "Browser.setDownloadBehavior", map[string]any{"behavior": "allow", "downloadPath": directory}, nil)
	c.button(t, "下载当前导出页")
	deadline := time.Now().Add(14 * time.Second)
	for time.Now().Before(deadline) {
		files, e := filepath.Glob(filepath.Join(directory, "game-personal-*.json"))
		if e != nil {
			t.Fatal("owned download inspection failed")
		}
		if len(files) == 1 {
			raw, e := os.ReadFile(files[0])
			if e != nil || len(raw) > playerapi.MaxResponseBytes {
				t.Fatal("bounded filtered download absent")
			}
			var exported playerapi.Export
			if json.Unmarshal(raw, &exported) != nil || exported.FormatVersion != 1 || exported.Kind != "personal" {
				t.Fatal("downloaded player export invalid")
			}
			private(t, exported, false)
			clear(raw)
			return
		}
		time.Sleep(60 * time.Millisecond)
	}
	t.Fatal("real player export download did not finish")
}

var _ = fmt.Sprint
