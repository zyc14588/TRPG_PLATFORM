//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package presentation_test

import (
	"bytes"
	"encoding/json"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
	"github.com/zyc14588/TRPG_PLATFORM/tests/integration/player_presentation/fixture"
	"io"
	"net/http"
	"os"
	"strings"
	"testing"
	"time"
)

func TestMain(m *testing.M) { fixture.Run(m) }
func data(t *testing.T, v map[string]any, status int) map[string]any {
	t.Helper()
	if status != 200 {
		t.Fatalf("safe response status %d", status)
	}
	d, ok := v["data"].(map[string]any)
	if !ok {
		t.Fatal("presentation data missing")
	}
	return d
}
func selections(t *testing.T, v map[string]any, status int) []any {
	t.Helper()
	d := data(t, v, status)
	x, ok := d["model_selections"].([]any)
	if !ok {
		t.Fatal("complete model array absent")
	}
	return x
}

func TestPresentationActualAuthenticatedGraphOverHTTPS(t *testing.T) {
	h := fixture.New(t, false)
	v, status, headers, _ := h.GET(t, "owner", "ai-required", nil)
	d := data(t, v, status)
	if headers.Get("Cache-Control") != "no-store" || headers.Get("Referrer-Policy") != "no-referrer" {
		t.Fatal("privacy headers absent")
	}
	var actualHash string
	var packages []install.PresentationPackage
	for _, c := range h.Configurations() {
		x := c.StorageValue()
		if x.ID == "ai-required" {
			q := x.Request
			q.Session = "game"
			var e error
			actualHash, packages, e = x.Factory.Presentation(h.Context(), q)
			if e != nil {
				t.Fatal("actual graph authentication failed")
			}
		}
	}
	if d["graph_hash"] != actualHash || d["workspace_id"] != h.Workspace() || d["configuration_id"] != "ai-required" || d["game_id"] != "game" {
		t.Fatal("identity/actual graph binding mismatch")
	}
	expected, _ := json.Marshal(packages)
	var expectedValue any
	if json.Unmarshal(expected, &expectedValue) != nil {
		t.Fatal("authenticated projection JSON invalid")
	}
	expected, _ = json.Marshal(expectedValue)
	got, _ := json.Marshal(d["packages"])
	if !bytes.Equal(expected, got) {
		t.Fatal("authenticated full manifest facts differ")
	}
	if len(packages) < 1 || !strings.Contains(packages[0].PackageID, "/") || !strings.HasPrefix(packages[0].ArtifactDigest, "sha256:") {
		t.Fatal("native package identity/digest lost")
	}
}
func TestPresentationReadDoesNotMutateStateOrDispatch(t *testing.T) {
	h := fixture.New(t, false)
	before := h.Baseline(t)
	for i := 0; i < 2; i++ {
		v, s, _, _ := h.GET(t, "owner", "ai-required", nil)
		_ = data(t, v, s)
	}
	if before != h.Baseline(t) || h.ProviderCalls.Load() != 0 {
		t.Fatal("read changed durable business state or dispatched")
	}
	if h.SQL(t, "SELECT count(*) FROM platform_launch.sessions WHERE workspace_id='"+h.Workspace()+"'") != "0" {
		t.Fatal("read provisioned authoritative session")
	}
}
func TestPresentationRevalidatesMembership(t *testing.T) {
	h := fixture.New(t, false)
	h.AddParticipantMembership(t)
	v, s, _, _ := h.GET(t, "participant", "ai-required", nil)
	_ = data(t, v, s)
	h.RemoveParticipantMembership(t)
	v, s, _, _ = h.GET(t, "participant", "ai-required", nil)
	if s == 200 || v["data"] != nil {
		t.Fatal("removed relationship reused")
	}
}
func TestPresentationRevalidatesCookie(t *testing.T) {
	h := fixture.New(t, false)
	h.RevokeCookie(t)
	v, s, _, _ := h.GET(t, "owner", "ai-required", nil)
	if s == 200 || v["data"] != nil {
		t.Fatal("revoked session reused")
	}
}
func TestPresentationGuestReadsOnlyCurrentSelectedConfiguration(t *testing.T) {
	h := fixture.New(t, true)
	v, s, _, _ := h.GET(t, "participant", "ai-required", nil)
	if len(selections(t, v, s)) != 0 {
		t.Fatal("guest acquired private AI inventory")
	}
	v, s, _, _ = h.GET(t, "participant", "minimal", nil)
	if s == 200 || v["data"] != nil {
		t.Fatal("guest read an unselected configuration")
	}
	h.ChangeConfiguration(t, "minimal")
	v, s, _, _ = h.GET(t, "participant", "ai-required", nil)
	if s == 200 || v["data"] != nil {
		t.Fatal("guest reused prior selected configuration")
	}
	v, s, _, _ = h.GET(t, "participant", "minimal", nil)
	_ = data(t, v, s)
	h.DisableGuest(t)
	v, s, _, _ = h.GET(t, "participant", "minimal", nil)
	if s == 200 || v["data"] != nil {
		t.Fatal("disabled guest reused cached facts")
	}
}
func TestPresentationCrossTenantAndUnrelatedAccountDenied(t *testing.T) {
	h := fixture.New(t, false)
	for _, which := range []string{"unrelated", "owner"} {
		v, s, _, _ := h.GET(t, which, "ai-required", func(r *http.Request) {
			if which == "owner" {
				r.URL.Path = strings.Replace(r.URL.Path, h.Workspace(), "different-tenant", 1)
			}
		})
		if s == 200 || v["data"] != nil {
			t.Fatal("unrelated relationship/tenant leaked")
		}
	}
}
func TestPresentationNoConsentIsNeverModelReady(t *testing.T) {
	h := fixture.New(t, false)
	v, s, _, _ := h.GET(t, "owner", "ai-required", nil)
	x := selections(t, v, s)
	if len(x) != 1 || x[0].(map[string]any)["ready"] != false {
		t.Fatal("missing consent became ready")
	}
}
func TestPresentationCurrentModelRevocations(t *testing.T) {
	for name, revoke := range map[string]func(*fixture.Harness, *testing.T){"configuration": func(h *fixture.Harness, t *testing.T) { h.RevokeModel(t) }, "credential": func(h *fixture.Harness, t *testing.T) { h.RevokeCredential(t) }, "certification": func(h *fixture.Harness, t *testing.T) { h.RevokeCertificate(t) }} {
		t.Run(name, func(t *testing.T) {
			h := fixture.New(t, false)
			h.Confirm(t)
			v, s, _, _ := h.GET(t, "owner", "ai-required", nil)
			x := selections(t, v, s)
			if len(x) != 1 || x[0].(map[string]any)["ready"] != true {
				t.Fatal("current qualified selection not ready")
			}
			revoke(h, t)
			v, s, _, _ = h.GET(t, "owner", "ai-required", nil)
			if len(selections(t, v, s)) != 0 {
				t.Fatal("revoked authority remained available")
			}
		})
	}
}
func TestPresentationAggregateBudgetExhaustionCannotDefaultReady(t *testing.T) {
	h := fixture.New(t, false)
	h.Confirm(t)
	v, s, _, _ := h.GET(t, "owner", "ai-required", nil)
	x := selections(t, v, s)
	if len(x) != 1 || x[0].(map[string]any)["ready"] != true {
		t.Fatal("current ready chain missing")
	}
	h.ExhaustBudget(t)
	before := h.Baseline(t)
	v, s, _, _ = h.GET(t, "owner", "ai-required", nil)
	x = selections(t, v, s)
	if len(x) != 1 || x[0].(map[string]any)["ready"] != false || before != h.Baseline(t) {
		t.Fatal("exhausted budget became ready or read wrote counters")
	}
}
func TestPresentationStrictHTTPForms(t *testing.T) {
	h := fixture.New(t, false)
	cases := map[string]func(*http.Request){"query": func(r *http.Request) { r.URL.RawQuery = "configuration=other" }, "post": func(r *http.Request) { r.Method = "POST" }, "authorization": func(r *http.Request) { r.Header.Set("Authorization", "Bearer synthetic") }, "foreign_origin": func(r *http.Request) { r.Header.Set("Origin", "https://foreign.invalid") }, "duplicate_origin": func(r *http.Request) { r.Header.Add("Origin", r.Header.Get("Origin")) }, "body": func(r *http.Request) { r.Body = io.NopCloser(strings.NewReader("{}")); r.ContentLength = 2 }, "extra_path": func(r *http.Request) { r.URL.Path += "/extra" }, "payload_header": func(r *http.Request) { r.Header.Set("Content-Type", "application/json") }}
	before := h.Baseline(t)
	for name, mutate := range cases {
		t.Run(name, func(t *testing.T) {
			v, s, _, b := h.GET(t, "owner", "ai-required", mutate)
			if s == 200 || v["data"] != nil || len(b) > player.MaxResponseBytes {
				t.Fatal("invalid request read accepted")
			}
		})
	}
	if before != h.Baseline(t) {
		t.Fatal("invalid form wrote business state")
	}
}
func TestPresentationPublicPrivacyAndClosedProductSchema(t *testing.T) {
	h := fixture.New(t, false)
	v, s, _, b := h.GET(t, "owner", "ai-required", nil)
	_ = data(t, v, s)
	for _, private := range []string{"owned-B014-synthetic-private-provider-key", "endpoint", "ciphertext", "CredentialID", "ExpiresAt", "fixture-gm-private-value", "certificate", "127.0.0.1:", "Controller", "Boundaries"} {
		if bytes.Contains(b, []byte(private)) {
			t.Fatal("private source leaked; output withheld")
		}
	}
	if len(b) > player.MaxResponseBytes {
		t.Fatal("unbounded response")
	}
}
func TestPresentationFixtureRequiresPhysicalOwnership(t *testing.T) {
	if os.Getenv("M2B014_OWNED_DATABASE") != "1" {
		t.Fatal("physical ownership absent")
	}
}
func TestPresentationActualCompleteDependencyGraphAndMissingGraphFailClosed(t *testing.T) {
	h := fixture.NewWithDependency(t)
	v, s, _, _ := h.GET(t, "owner", "ai-required", nil)
	d := data(t, v, s)
	packages := d["packages"].([]any)
	if len(packages) != 2 {
		t.Fatal("authenticated dependency graph truncated")
	}
	found := false
	for _, p := range packages {
		if p.(map[string]any)["package_id"] == "fixture.example/content" {
			found = true
		}
	}
	if !found {
		t.Fatal("actual installed dependency absent")
	}
	h.BreakGraphMapping(t, true)
	v, s, _, _ = h.GET(t, "owner", "ai-required", nil)
	if s == 200 || v["data"] != nil {
		t.Fatal("incomplete graph treated as presentation")
	}
}
func TestPresentationMismatchedInstallationFailsClosed(t *testing.T) {
	h := fixture.New(t, false)
	h.BreakGraphMapping(t, false)
	v, s, _, _ := h.GET(t, "owner", "ai-required", nil)
	if s == 200 || v["data"] != nil {
		t.Fatal("unknown installation became presentation")
	}
}
func TestPresentationUnseatedAdministratorCannotReadAISeatInventory(t *testing.T) {
	h := fixture.New(t, false)
	h.UnseatHost(t)
	v, s, _, _ := h.GET(t, "owner", "ai-required", nil)
	if len(selections(t, v, s)) != 0 {
		t.Fatal("unseated workspace authority read AI-seat data")
	}
}
func TestPresentationCanceledReadHasNoMutation(t *testing.T) {
	h := fixture.New(t, false)
	before := h.Baseline(t)
	if e := h.DirectCanceled(t); e == nil {
		t.Fatal("canceled read succeeded")
	}
	if before != h.Baseline(t) || h.ProviderCalls.Load() != 0 {
		t.Fatal("canceled read mutated/dispatched")
	}
}
func TestPresentationDeadlineRollsBackAndReleasesPostgreSQL(t *testing.T) {
	h := fixture.New(t, false)
	h.DelayReadUntilDeadline(t)
	before := h.Baseline(t)
	start := time.Now()
	v, s, _, _ := h.GET(t, "owner", "ai-required", nil)
	if s == 200 || v["data"] != nil || time.Since(start) > 6*time.Second {
		t.Fatal("five-second processing deadline failed")
	}
	if before != h.Baseline(t) || h.OpenTransactionCount(t) != "0" || h.ProviderCalls.Load() != 0 {
		t.Fatal("deadline retained transaction or mutated/dispatched")
	}
}
func TestPresentationMalformedBudgetCannotGrantReadiness(t *testing.T) {
	h := fixture.New(t, false)
	h.Confirm(t)
	h.CorruptBudget(t)
	v, s, _, _ := h.GET(t, "owner", "ai-required", nil)
	if s == 200 || v["data"] != nil {
		t.Fatal("malformed authoritative counters accepted")
	}
}
func TestPresentationUnknownBillingCannotBecomeReadyOrAutoResume(t *testing.T) {
	h := fixture.New(t, false)
	h.MarkUnknownBilling(t)
	before := h.Baseline(t)
	v, s, _, _ := h.GET(t, "owner", "ai-required", nil)
	x := selections(t, v, s)
	if len(x) != 1 || x[0].(map[string]any)["ready"] != false || before != h.Baseline(t) || h.ProviderCalls.Load() != 0 {
		t.Fatal("unknown durable usage became ready, resumed, mutated or dispatched")
	}
}
