// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package m2smoke

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strconv"
	"sync/atomic"
	"testing"
	"time"
)

func snapshotTestLobby(t *testing.T, handler http.HandlerFunc) *Lobby {
	t.Helper()
	server := httptest.NewTLSServer(handler)
	t.Cleanup(server.Close)
	b := &Browser{client: server.Client(), origin: server.URL, pace: &browserPace{snapshots: map[*Browser]time.Time{}}}
	return &Lobby{Workspace: "w", Room: "r", PlayerConnection: "original-connection", Participant: b}
}

func TestWaitSnapshotRetriesOnlyCurrentReadConflict(t *testing.T) {
	var calls atomic.Int32
	l := snapshotTestLobby(t, func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		var request map[string]any
		if r.Method != "POST" || r.URL.Path != "/api/v1/workspaces/w/rooms/r/session/snapshot" || json.NewDecoder(r.Body).Decode(&request) != nil || request["connection_id"] != "original-connection" || request["after_cursor"] != "0" || request["limit"] != float64(8) {
			t.Error("snapshot changed its original request or connection")
		}
		if calls.Load() == 1 {
			w.WriteHeader(http.StatusConflict)
			_, _ = w.Write([]byte(`{"error":{"code":"CONFLICT"}}`))
			return
		}
		_, _ = w.Write([]byte(`{"data":{"state_version":"4"}}`))
	})
	started := time.Now()
	d, e := waitSnapshot(context.Background(), l, "4")
	if e != nil || d["state_version"] != "4" || calls.Load() != 2 || time.Since(started) >= 8*time.Second {
		t.Fatalf("bounded original read did not recover: calls=%d error=%v", calls.Load(), e)
	}
}

func TestWaitSnapshotDoesNotRetryOtherErrorsOrStaleConflict(t *testing.T) {
	for _, tc := range []struct {
		name   string
		status int
		body   string
	}{
		{"denied", 403, `{"error":{"code":"CONFLICT"}}`},
		{"other-conflict-code", 409, `{"error":{"code":"DENIED"}}`},
		{"unavailable", 503, `{"error":{"code":"CONFLICT"}}`},
		{"malformed-conflict", 409, `{`},
		{"malformed-success", 200, `{}`},
	} {
		t.Run(tc.name, func(t *testing.T) {
			var calls atomic.Int32
			l := snapshotTestLobby(t, func(w http.ResponseWriter, _ *http.Request) {
				calls.Add(1)
				w.WriteHeader(tc.status)
				_, _ = w.Write([]byte(tc.body))
			})
			l.Participant.lastStatus, l.Participant.lastCode = 409, "CONFLICT"
			if _, e := waitSnapshot(context.Background(), l, "4"); e == nil || calls.Load() != 1 {
				t.Fatalf("nonmatching response retried: calls=%d error=%v", calls.Load(), e)
			}
		})
	}
	t.Run("cancelled-admission-with-stale-conflict", func(t *testing.T) {
		var calls atomic.Int32
		l := snapshotTestLobby(t, func(http.ResponseWriter, *http.Request) { calls.Add(1) })
		l.Participant.lastStatus, l.Participant.lastCode = 409, "CONFLICT"
		l.Participant.pace.last = time.Now()
		ctx, cancel := context.WithCancel(context.Background())
		cancel()
		if _, e := waitSnapshot(ctx, l, "4"); e == nil || calls.Load() != 0 {
			t.Fatalf("stale conflict swallowed cancellation: calls=%d error=%v", calls.Load(), e)
		}
	})
}

func TestWaitSnapshotReadConflictPreservesTotalDeadline(t *testing.T) {
	var calls atomic.Int32
	l := snapshotTestLobby(t, func(w http.ResponseWriter, _ *http.Request) {
		calls.Add(1)
		w.WriteHeader(http.StatusConflict)
		_, _ = w.Write([]byte(`{"error":{"code":"CONFLICT"}}`))
	})
	started := time.Now()
	if _, e := waitSnapshot(context.Background(), l, "4"); e == nil || calls.Load() != 2 || time.Since(started) >= 9*time.Second {
		t.Fatalf("read conflicts escaped total deadline: calls=%d error=%v elapsed=%s", calls.Load(), e, time.Since(started))
	}
}

// This models both observed Compose JSON representations. Runtime inspection
// remains a separate check of actual Docker and kernel state in lifecycle.
func boundedTopology() map[string]any {
	services := map[string]any{}
	for _, name := range []string{"reverse-proxy", "platformd", "workerd", "lua-runner", "postgres", "object-storage"} {
		memory := int64(512 << 20)
		if name == "lua-runner" {
			memory = 768 << 20
		}
		if name == "postgres" {
			memory = 1 << 30
		}
		s := map[string]any{"image": "test-image", "user": "1000:1000", "init": true, "read_only": name != "postgres", "cpus": 1, "mem_limit": strconv.FormatInt(memory, 10), "pids_limit": 128, "cap_drop": []string{"ALL"}, "security_opt": []string{"no-new-privileges:true"}, "labels": map[string]string{"trpg.m2.source": "test-source"}, "networks": map[string]any{"private": nil}, "volumes": []any{map[string]any{"target": "/run/operator", "type": "bind", "read_only": true}}}
		if name == "postgres" {
			s["image"] = "sha256:3a82e1f56c8f0f5616a11103ac3d47e632c3938698946a7ad26da0df1334744a"
		} else {
			entry := map[string]string{"reverse-proxy": "reverse-proxyd", "platformd": "platformd", "workerd": "workerd", "lua-runner": "lua-supervisord", "object-storage": "objectd"}[name]
			s["entrypoint"] = []string{"/app/" + entry}
		}
		if name == "reverse-proxy" {
			s["networks"].(map[string]any)["edge"] = nil
			s["ports"] = []any{map[string]any{"host_ip": "127.0.0.1", "published": "33443", "target": 8443, "protocol": "tcp"}}
		}
		if name == "workerd" {
			s["networks"].(map[string]any)["provider"] = nil
		}
		services[name] = s
	}
	return map[string]any{"services": services, "networks": map[string]any{"private": map[string]any{"external": true, "name": "test-private"}, "edge": map[string]any{"external": true, "name": "test-edge"}, "provider": map[string]any{"external": true, "name": "explicit-provider"}}}
}

func TestTopologyAcceptsObservedDecimalMemoryAndRejectsUnboundedExposure(t *testing.T) {
	for _, number := range []bool{false, true} {
		v := boundedTopology()
		if number {
			for _, s := range v["services"].(map[string]any) {
				m := s.(map[string]any)
				n, _ := strconv.ParseInt(m["mem_limit"].(string), 10, 64)
				m["mem_limit"] = n
			}
		}
		raw, _ := json.Marshal(v)
		if _, e := VerifyTopology(raw, "test-source", "test-image"); e != nil {
			t.Fatalf("bounded decimal representation rejected: numeric=%t", number)
		}
	}
	faults := map[string]func(map[string]any){
		"root":             func(s map[string]any) { s["user"] = "0:0" },
		"unbounded-memory": func(s map[string]any) { s["mem_limit"] = "0" },
		"invalid-memory":   func(s map[string]any) { s["mem_limit"] = "512m" },
		"unbounded-pids":   func(s map[string]any) { s["pids_limit"] = -1 },
		"privileged":       func(s map[string]any) { s["privileged"] = true },
		"capability":       func(s map[string]any) { s["cap_add"] = []string{"NET_ADMIN"} },
		"no-cap-drop":      func(s map[string]any) { s["cap_drop"] = []string{} },
		"no-nnp":           func(s map[string]any) { s["security_opt"] = []string{} },
		"writable-root":    func(s map[string]any) { s["read_only"] = false },
		"writable-secret":  func(s map[string]any) { s["volumes"].([]any)[0].(map[string]any)["read_only"] = false },
		"writable-cas": func(s map[string]any) {
			s["volumes"] = append(s["volumes"].([]any), map[string]any{"target": "/var/lib/trpg/objects", "type": "volume"})
		},
		"public-listener":  func(s map[string]any) { s["ports"] = []any{map[string]any{"published": "34443"}} },
		"provider-network": func(s map[string]any) { s["networks"].(map[string]any)["provider"] = nil },
	}
	for name, mutate := range faults {
		t.Run(name, func(t *testing.T) {
			v := boundedTopology()
			mutate(v["services"].(map[string]any)["platformd"].(map[string]any))
			raw, _ := json.Marshal(v)
			if _, e := VerifyTopology(raw, "test-source", "test-image"); e == nil {
				t.Fatal("unsafe deployment configuration accepted")
			}
		})
	}
	for _, field := range []string{"host_ip", "target", "protocol", "published"} {
		t.Run("proxy-"+field, func(t *testing.T) {
			v := boundedTopology()
			port := v["services"].(map[string]any)["reverse-proxy"].(map[string]any)["ports"].([]any)[0].(map[string]any)
			port[field] = map[string]any{"host_ip": "0.0.0.0", "target": 8080, "protocol": "udp", "published": "0"}[field]
			raw, _ := json.Marshal(v)
			if _, e := VerifyTopology(raw, "test-source", "test-image"); e == nil {
				t.Fatal("unsafe proxy exposure accepted")
			}
		})
	}
}

func TestInventoryRequiresSixKnownDistinctServicesAndOnlyProxyTLS(t *testing.T) {
	fixture := func() []map[string]any {
		var v []map[string]any
		for _, name := range []string{"reverse-proxy", "platformd", "workerd", "lua-runner", "postgres", "object-storage"} {
			s := map[string]any{"ID": "owned-" + name, "Name": "owned-" + name, "Service": name, "State": "running", "Health": "healthy"}
			if name == "reverse-proxy" {
				s["Publishers"] = []any{map[string]any{"URL": "127.0.0.1", "PublishedPort": 34443, "TargetPort": 8443, "Protocol": "tcp"}}
			}
			v = append(v, s)
		}
		return v
	}
	valid, _ := json.Marshal(fixture())
	if _, ready, e := ServiceInventory(valid); e != nil || !ready {
		t.Fatal("real six-service shape rejected")
	}
	for _, fault := range []string{"duplicate", "unknown", "same-container", "worker-port", "proxy-plain", "proxy-public", "unhealthy"} {
		t.Run(fault, func(t *testing.T) {
			v := fixture()
			switch fault {
			case "duplicate":
				v[1]["Service"] = "reverse-proxy"
			case "unknown":
				v[1]["Service"] = "placeholder"
			case "same-container":
				v[1]["ID"] = v[0]["ID"]
			case "worker-port":
				v[2]["Publishers"] = []any{map[string]any{"URL": "127.0.0.1", "PublishedPort": 34444, "TargetPort": 8443, "Protocol": "tcp"}}
			case "proxy-plain":
				v[0]["Publishers"].([]any)[0].(map[string]any)["TargetPort"] = 8080
			case "proxy-public":
				v[0]["Publishers"].([]any)[0].(map[string]any)["URL"] = "0.0.0.0"
			case "unhealthy":
				v[1]["Health"] = "unhealthy"
			}
			raw, _ := json.Marshal(v)
			if _, ready, e := ServiceInventory(raw); e == nil && ready {
				t.Fatal("bad inventory counted as healthy deployment")
			}
		})
	}
}
