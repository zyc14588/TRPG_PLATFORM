// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package model

import (
	"encoding/json"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"slices"
	"strings"
	"testing"
	"time"
)

func sampleEndpoint() EndpointData {
	return EndpointData{ID: "local", URL: "http://127.0.0.1:8081/v1", Adapter: "openai-compatible", Models: []string{"fixture:small", "fixture:other"}, AllowLANHTTP: true}
}
func sampleCertificate() CertificationData {
	h := strings.Repeat("a", 64)
	return CertificationData{ID: "platform-default", WorkspaceID: "w", Tuple: Tuple{Model: "fixture:small", Endpoint: sampleEndpoint().URL, Adapter: "openai-compatible", PromptTemplate: "safe-v1", ToolMode: "structured", TestVersion: "tests-v1"}, Level: 3, Capabilities: []string{"structured-actions", "ai-player"}, Games: []GameEvidence{{GraphHash: "sha256:" + h, TestVersion: "tests-v1", EvidenceHash: h, Capabilities: []string{"structured-actions", "ai-player"}}}, EvidenceHash: h, ExpiresAt: time.Now().Add(time.Hour)}
}
func TestApprovedEndpointDenyMatrix(t *testing.T) {
	base := sampleEndpoint()
	if !canonicalEndpoint(base) {
		t.Fatal("approved local compatible endpoint denied")
	}
	for _, url := range []string{"http://public.example/v1", "https://key@provider.example/v1", "https://provider.example/v1?key=x", "https://provider.example/v1#token", "ftp://provider.example/v1", "https://UPPER.example/v1", "https://provider.example/v1?"} {
		t.Run(fmt.Sprintf("url-%d", len(url)), func(t *testing.T) {
			e := base
			e.URL = url
			if canonicalEndpoint(e) {
				t.Fatal("unapproved credential-bearing or ambiguous endpoint accepted")
			}
		})
	}
	base.AllowLANHTTP = false
	if canonicalEndpoint(base) {
		t.Fatal("LAN HTTP enabled implicitly")
	}
}
func TestCertificationBindsAllSixTupleFields(t *testing.T) {
	e := sampleEndpoint()
	c := sampleCertificate()
	registry := map[string]EndpointData{e.ID: e}
	if !validCertificate(c, registry) {
		t.Fatal("reviewed compatible tuple denied")
	}
	original := digest(c)
	changes := []func(*Tuple){func(x *Tuple) { x.Model = "fixture:other" }, func(x *Tuple) { x.Endpoint = "https://other.example/v1" }, func(x *Tuple) { x.Adapter = "unsupported-adapter" }, func(x *Tuple) { x.PromptTemplate = "changed-v2" }, func(x *Tuple) { x.ToolMode = "none" }, func(x *Tuple) { x.TestVersion = "changed-v2" }}
	for i, change := range changes {
		t.Run(fmt.Sprintf("field-%d", i), func(t *testing.T) {
			altered := c
			change(&altered.Tuple)
			if digest(altered) == original {
				t.Fatal("tuple change preserved certification identity")
			}
		})
	}
}
func TestCapabilityLevelAndGameQualificationFailClosed(t *testing.T) {
	c := sampleCertificate()
	e := sampleEndpoint()
	endpoints := map[string]EndpointData{e.ID: e}
	for _, level := range []int{0, 5} {
		x := c
		x.Level = level
		if validCertificate(x, endpoints) {
			t.Fatal("invalid MC level accepted")
		}
	}
	x := c
	x.Level = 4
	if validCertificate(x, endpoints) {
		t.Fatal("MC4 without host qualification accepted")
	}
	x = NewCertification(c).StorageValue()
	x.Games[0].TestVersion = "stale"
	if validCertificate(x, endpoints) {
		t.Fatal("mismatched qualification test version accepted")
	}
	if certified(c, "sha256:"+strings.Repeat("b", 64), nil, time.Now()) || certified(c, c.Games[0].GraphHash, []string{"ai-host"}, time.Now()) || certified(c, c.Games[0].GraphHash, nil, c.ExpiresAt) {
		t.Fatal("missing game capability or expired evidence accepted")
	}
}
func TestBudgetHardBoundsAndWorkspaceIntersection(t *testing.T) {
	b := Limits{Calls: 4, Tokens: 4096, CostMicros: 1000, LatencyMillis: 1000, Tools: 4, ContextBytes: 4096}
	if !validBudget(b) || !fits(b, b) {
		t.Fatal("finite budget denied")
	}
	x := b
	x.Calls = 0
	if validBudget(x) {
		t.Fatal("zero/default budget became authorization")
	}
	x = b
	x.Tokens = 1_000_001
	if validBudget(x) {
		t.Fatal("unbounded tokens accepted")
	}
	x = b
	x.CostMicros++
	if fits(x, b) {
		t.Fatal("seat budget exceeded workspace cap")
	}
	x = b
	x.Subagents = 9
	if validBudget(x) {
		t.Fatal("unbounded agents accepted")
	}
}
func TestQualificationAndRequestCopiesDoNotShareCallerLists(t *testing.T) {
	c := sampleCertificate()
	v := NewCertification(c)
	c.Capabilities[0] = "wrong"
	c.Games[0].Capabilities[0] = "wrong"
	if !slices.Contains(v.StorageValue().Capabilities, "structured-actions") || !slices.Contains(v.StorageValue().Games[0].Capabilities, "structured-actions") {
		t.Fatal("qualification aliases caller")
	}
	r := ConfigureRequestData{FallbackIDs: []string{"approved"}}
	q := NewConfigureRequest(r)
	r.FallbackIDs[0] = "changed"
	if q.StorageValue().FallbackIDs[0] != "approved" {
		t.Fatal("fallback request aliases caller")
	}
}
func TestModelPrivateHandlesNeverBecomeOrdinaryJSON(t *testing.T) {
	v := NewCertification(sampleCertificate())
	q := NewConfigureRequest(ConfigureRequestData{SeatID: "private-fixture-seat"})
	for _, x := range []any{v, &v, q, &q} {
		if _, e := json.Marshal(x); e == nil {
			t.Fatal("private model handle serialized")
		}
		for _, f := range []string{"%#v", "%+v", "%s", "%d", "%*s"} {
			if strings.Contains(fmt.Sprintf(f, x), "private-fixture-seat") {
				t.Fatal("private seat reached diagnostics")
			}
		}
	}
	var service *Service
	if _, e := service.Read(nil, Caller{}, Target{}); e != auth.ErrUnavailable {
		t.Fatal("nil service did not fail closed")
	}
}
