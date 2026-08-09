// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/signal"
	"syscall"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/ipc"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
)

var version = "0.1.0-m1-b002"

func main() {
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	os.Exit(run(ctx, os.Args[1:], os.Stdin, os.Stdout, os.Stderr))
}

func run(ctx context.Context, args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	if len(args) == 0 {
		writeHelp(stdout)
		return 0
	}
	switch args[0] {
	case "help", "-h", "--help":
		writeHelp(stdout)
		return 0
	case "version", "-v", "--version":
		fmt.Fprintf(stdout, "lua-runner %s\n", version)
		return 0
	case "health":
		result := struct {
			Component string `json:"component"`
			Status    string `json:"status"`
			Profile   string `json:"lua_profile"`
		}{Component: "lua-runner", Status: "ok", Profile: profile.ProductionID}
		if err := json.NewEncoder(stdout).Encode(result); err != nil {
			fmt.Fprintf(stderr, "encode health response: %v\n", err)
			return 2
		}
		return 0
	case "profile":
		return writeProfile(stdout, stderr)
	case "licenses":
		fmt.Fprintf(stdout, "%s@%s\n%s\n", profile.RuntimeModule, profile.RuntimeVersion, profile.RuntimeLicenseText())
		return 0
	case "serve":
		if len(args) != 1 {
			fmt.Fprintln(stderr, "serve accepts no arguments")
			return 2
		}
		if err := ipc.Serve(ctx, stdin, stdout); err != nil && !errors.Is(err, context.Canceled) {
			fmt.Fprintf(stderr, "lua-runner serve: %v\n", err)
			return 1
		}
		return 0
	case "evidence":
		return runEvidence(ctx, args[1:], stdout, stderr)
	default:
		fmt.Fprintf(stderr, "lua-runner: unknown command %q\n", args[0])
		writeHelp(stderr)
		return 2
	}
}

func writeProfile(stdout, stderr io.Writer) int {
	p := profile.Production()
	c := profile.RuntimeCandidate()
	result := struct {
		ProfileID            string   `json:"profile_id"`
		LanguageVersion      string   `json:"language_version"`
		RuntimeVersion       string   `json:"runtime_version"`
		RuntimeCommit        string   `json:"runtime_commit"`
		RuntimeLicense       string   `json:"runtime_license"`
		RuntimeLicenseSHA256 string   `json:"runtime_license_sha256"`
		SourceOnly           bool     `json:"source_only"`
		ProductionDebug      bool     `json:"production_debug"`
		Allowed              []string `json:"allowed_libraries"`
		Denied               []string `json:"denied_surfaces"`
	}{
		ProfileID:            p.ID,
		LanguageVersion:      p.LanguageVersion,
		RuntimeVersion:       p.RuntimeVersion,
		RuntimeCommit:        c.Commit,
		RuntimeLicense:       c.License,
		RuntimeLicenseSHA256: c.LicenseSHA256,
		SourceOnly:           true,
		ProductionDebug:      false,
		Allowed:              []string{"basic-restricted", "coroutine", "math", "string-restricted", "table", "utf8"},
		Denied:               []string{"bytecode", "debug", "dofile", "dynamic-libraries", "environment", "filesystem", "golib", "io", "load", "loadfile", "native-modules", "network", "os", "package", "process", "raw-credentials", "require", "string.dump"},
	}
	encoder := json.NewEncoder(stdout)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(result); err != nil {
		fmt.Fprintf(stderr, "encode profile response: %v\n", err)
		return 2
	}
	return 0
}

func writeHelp(w io.Writer) {
	fmt.Fprintln(w, "Usage: lua-runner <command>")
	fmt.Fprintln(w)
	fmt.Fprintln(w, "Commands:")
	fmt.Fprintln(w, "  version   Print runner version")
	fmt.Fprintln(w, "  health    Print deterministic health JSON")
	fmt.Fprintln(w, "  profile   Print the production Lua profile and runtime identity")
	fmt.Fprintln(w, "  licenses  Print embedded third-party runtime license text")
	fmt.Fprintln(w, "  serve     Serve versioned VM lifecycle IPC over stdin/stdout")
	fmt.Fprintln(w, "  evidence  Run candidate-bound TEST-LUA evidence")
	fmt.Fprintln(w, "  help      Print this help")
}
