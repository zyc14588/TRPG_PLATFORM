// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package baselinecli

import (
	"bytes"
	"context"
	"strings"
	"testing"
)

func TestVersionAndHealth(t *testing.T) {
	t.Parallel()

	cfg := Config{Name: "lua-runner", Version: "0.0.0-m0", AllowServe: true}
	for _, test := range []struct {
		name string
		args []string
		want string
	}{
		{name: "version", args: []string{"version"}, want: "lua-runner 0.0.0-m0\n"},
		{name: "health", args: []string{"health"}, want: `{"component":"lua-runner","status":"ok","milestone":"M0","playable":false}` + "\n"},
	} {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			var stdout bytes.Buffer
			var stderr bytes.Buffer
			if code := Run(context.Background(), cfg, test.args, &stdout, &stderr); code != ExitOK {
				t.Fatalf("Run() exit code = %d, want %d; stderr=%q", code, ExitOK, stderr.String())
			}
			if got := stdout.String(); got != test.want {
				t.Fatalf("stdout = %q, want %q", got, test.want)
			}
		})
	}
}

func TestUnknownCommandHasStableUsageExit(t *testing.T) {
	t.Parallel()

	var stdout bytes.Buffer
	var stderr bytes.Buffer
	code := Run(context.Background(), Config{Name: "lua-runner", Version: "dev"}, []string{"play"}, &stdout, &stderr)
	if code != ExitUsage {
		t.Fatalf("Run() exit code = %d, want %d", code, ExitUsage)
	}
	if !strings.Contains(stderr.String(), "unknown command") {
		t.Fatalf("stderr = %q, want unknown-command diagnostic", stderr.String())
	}
}

func TestServeStopsWhenContextIsCancelled(t *testing.T) {
	t.Parallel()

	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	code := Run(ctx, Config{Name: "workerd", Version: "dev", AllowServe: true}, []string{"serve"}, &stdout, &stderr)
	if code != ExitOK {
		t.Fatalf("Run() exit code = %d, want %d; stderr=%q", code, ExitOK, stderr.String())
	}
	if !strings.Contains(stdout.String(), "no playable functionality") {
		t.Fatalf("stdout = %q, want M0 scope marker", stdout.String())
	}
}
