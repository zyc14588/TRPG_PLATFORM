// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package model

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	store "github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"net"
	"net/url"
	"regexp"
	"slices"
	"strings"
	"time"
)

func digest(v any) string {
	raw, _ := json.Marshal(v)
	h := sha256.Sum256(raw)
	return hex.EncodeToString(h[:])
}
func hash(v string) bool { return len(v) == 64 && strings.Trim(v, "0123456789abcdef") == "" }
func graphHash(v string) bool {
	return strings.HasPrefix(v, "sha256:") && hash(strings.TrimPrefix(v, "sha256:"))
}
func scope(s core.Scope) bool {
	return store.ValidID(s.WorkspaceID) && store.ValidID(s.RoomID) && store.ValidID(s.GameID)
}
func labels(xs []string, max int) bool {
	seen := map[string]bool{}
	if len(xs) > max {
		return false
	}
	for _, x := range xs {
		if !store.ValidID(x) || seen[x] {
			return false
		}
		seen[x] = true
	}
	return true
}

var modelName = regexp.MustCompile(`^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,127}$`)

func models(xs []string) bool {
	if len(xs) < 1 || len(xs) > 128 {
		return false
	}
	seen := map[string]bool{}
	for _, x := range xs {
		if !modelName.MatchString(x) || seen[x] {
			return false
		}
		seen[x] = true
	}
	return true
}
func canonicalEndpoint(e EndpointData) bool {
	if !store.ValidID(e.ID) || !models(e.Models) || len(e.URL) > 512 {
		return false
	}
	if e.Adapter != "openai-compatible" {
		return false
	}
	u, err := url.Parse(e.URL)
	if err != nil || u.User != nil || u.Host == "" || u.RawQuery != "" || u.Fragment != "" || u.Opaque != "" || u.ForceQuery || u.RawPath != "" || u.String() != e.URL || u.Hostname() != strings.ToLower(u.Hostname()) {
		return false
	}
	if u.Scheme == "https" {
		return true
	}
	if u.Scheme != "http" || !e.AllowLANHTTP {
		return false
	}
	ip := net.ParseIP(u.Hostname())
	return ip != nil && (ip.IsLoopback() || ip.IsPrivate())
}
func validCertificate(c CertificationData, endpoints map[string]EndpointData) bool {
	if !store.ValidID(c.ID) || !store.ValidID(c.WorkspaceID) || c.Level < 1 || c.Level > 4 || !hash(c.EvidenceHash) || c.ExpiresAt.IsZero() || !labels(c.Capabilities, 32) || len(c.Games) < 1 || len(c.Games) > 128 {
		return false
	}
	x := c.Tuple
	if !modelName.MatchString(x.Model) || !store.ValidID(x.PromptTemplate) || !store.ValidID(x.TestVersion) || (x.ToolMode != "none" && x.ToolMode != "structured") {
		return false
	}
	var approved bool
	for _, e := range endpoints {
		if e.URL == x.Endpoint && e.Adapter == x.Adapter && slices.Contains(e.Models, x.Model) {
			approved = true
		}
	}
	if !approved {
		return false
	}
	if c.Level >= 2 && (x.ToolMode != "structured" || !slices.Contains(c.Capabilities, "structured-actions")) {
		return false
	}
	if c.Level >= 3 && !slices.Contains(c.Capabilities, "ai-player") {
		return false
	}
	if c.Level >= 4 && !slices.Contains(c.Capabilities, "ai-host") {
		return false
	}
	seen := map[string]bool{}
	for _, g := range c.Games {
		if !graphHash(g.GraphHash) || !hash(g.EvidenceHash) || g.TestVersion != x.TestVersion || !labels(g.Capabilities, 32) || seen[g.GraphHash] {
			return false
		}
		seen[g.GraphHash] = true
		for _, cap := range g.Capabilities {
			if !slices.Contains(c.Capabilities, cap) {
				return false
			}
		}
	}
	return true
}
func validBudget(b Limits) bool {
	return b.Calls > 0 && b.Calls <= 64 && b.Tokens > 0 && b.Tokens <= 1_000_000 && b.CostMicros > 0 && b.CostMicros <= 1_000_000_000 && b.LatencyMillis > 0 && b.LatencyMillis <= 120_000 && b.Tools <= 128 && b.Subagents <= 8 && b.ContextBytes > 0 && b.ContextBytes <= 1_048_576 && b.LocalComputeMillis <= 120_000
}
func fits(a, b Limits) bool {
	return a.Calls <= b.Calls && a.Tokens <= b.Tokens && a.CostMicros <= b.CostMicros && a.LatencyMillis <= b.LatencyMillis && a.Tools <= b.Tools && a.Subagents <= b.Subagents && a.ContextBytes <= b.ContextBytes && a.LocalComputeMillis <= b.LocalComputeMillis
}
func certified(c CertificationData, graph string, caps []string, now time.Time) bool {
	if !now.Before(c.ExpiresAt) {
		return false
	}
	for _, g := range c.Games {
		if g.GraphHash == graph {
			for _, cap := range caps {
				if !slices.Contains(c.Capabilities, cap) || !slices.Contains(g.Capabilities, cap) {
					return false
				}
			}
			return true
		}
	}
	return false
}
func CopyConfiguration(c Configuration) Configuration {
	d := c.StorageValue()
	d.Fallbacks = slices.Clone(d.Fallbacks)
	return auth.RoomSecret(d)
}
func NewEndpoint(d EndpointData) Endpoint {
	d.Models = slices.Clone(d.Models)
	return auth.RoomSecret(d)
}
func NewCertification(d CertificationData) Certification {
	d.Capabilities = slices.Clone(d.Capabilities)
	d.Games = slices.Clone(d.Games)
	for i := range d.Games {
		d.Games[i].Capabilities = slices.Clone(d.Games[i].Capabilities)
	}
	return auth.RoomSecret(d)
}
func NewConfigureRequest(d ConfigureRequestData) ConfigureRequest {
	d.FallbackIDs = slices.Clone(d.FallbackIDs)
	return auth.RoomSecret(d)
}
