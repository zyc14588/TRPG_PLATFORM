// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package certification

import (
	"context"
	"errors"
	"strings"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
)

func requestFixture() Request {
	return Request{ID: "owned-model", WorkspaceID: "owned-workspace", GraphHash: checkpoint.Hash([]byte("graph")), Tuple: model.Tuple{Model: "fixture:small", Endpoint: "http://127.0.0.1:8081/v1", Adapter: "openai-compatible", PromptTemplate: "safe-v1", ToolMode: "structured", TestVersion: "test-v1"}, Level: 3, ExpiresAt: time.Now().Add(time.Hour)}
}
func TestCertificationRequiresEveryActualBoundCaseAndRegistryTuple(t *testing.T) {
	r := requestFixture()
	var cases []string
	c, e := Certify(context.Background(), r, func(_ context.Context, tuple model.Tuple, name string) ([]byte, error) {
		if tuple != r.Tuple {
			t.Fatal("probe tuple replaced")
		}
		cases = append(cases, name)
		return []byte("synthetic actual observed fixture " + name), nil
	})
	if e != nil || len(cases) != 6 {
		t.Fatal("certification did not execute all required cases")
	}
	v := c.StorageValue()
	registry, e := New([]model.Certification{c})
	if e != nil {
		t.Fatal("reviewed registry rejected")
	}
	bound := model.CertificateBinding{ID: v.ID, Hash: Hash(v)}
	if _, e := registry.Bound(r.WorkspaceID, bound, r.GraphHash, []string{"ai-player", "structured-actions"}, 3, time.Now()); e != nil {
		t.Fatal("exact actual qualification rejected")
	}
	for _, name := range []string{"workspace", "hash", "graph", "capability", "level", "expired"} {
		t.Run(name, func(t *testing.T) {
			w, b, g, caps, level, now := r.WorkspaceID, bound, r.GraphHash, []string{"ai-player"}, 3, time.Now()
			switch name {
			case "workspace":
				w = "foreign"
			case "hash":
				b.Hash = strings.Repeat("0", 64)
			case "graph":
				g = checkpoint.Hash([]byte("foreign"))
			case "capability":
				caps = []string{"ai-host"}
			case "level":
				level = 4
			case "expired":
				now = r.ExpiresAt
			}
			if _, e := registry.Bound(w, b, g, caps, level, now); e == nil {
				t.Fatal("foreign or invalid certificate binding accepted")
			}
		})
	}
	v.Capabilities[0] = "forged"
	fresh, e := registry.Bound(r.WorkspaceID, bound, r.GraphHash, []string{"structured-actions"}, 3, time.Now())
	if e != nil || fresh.StorageValue().Capabilities[0] != "structured-actions" {
		t.Fatal("registry shared caller slice")
	}
}
func TestCertificationFailurePanicAndDeadlineMintNoRecord(t *testing.T) {
	for _, name := range []string{"failure", "panic", "empty", "timeout"} {
		t.Run(name, func(t *testing.T) {
			ctx, cancel := context.WithTimeout(context.Background(), 20*time.Millisecond)
			defer cancel()
			c, e := Certify(ctx, requestFixture(), func(ctx context.Context, _ model.Tuple, _ string) ([]byte, error) {
				switch name {
				case "panic":
					panic("private diagnostic")
				case "empty":
					return nil, nil
				case "timeout":
					<-ctx.Done()
					return []byte("late"), nil
				}
				return nil, errors.New("private diagnostic")
			})
			if e == nil || c.StorageValue().ID != "" {
				t.Fatal("failed probe minted qualification")
			}
		})
	}
}
func TestQualificationDeadlineDoesNotWaitForIgnoredCancellation(t *testing.T) {
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Millisecond)
	defer cancel()
	release := make(chan struct{})
	done := make(chan struct{})
	start := time.Now()
	_, e := Certify(ctx, requestFixture(), func(context.Context, model.Tuple, string) ([]byte, error) {
		<-release
		close(done)
		return []byte("late"), nil
	})
	close(release)
	<-done
	if e == nil || time.Since(start) > 500*time.Millisecond {
		t.Fatal("ignored cancellation exceeded bounded certification deadline")
	}
}
