// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package baselinecli provides the deliberately small command surface shared by
// M0 process shells. It contains no game, account, room, session, storage, or AI
// behavior.
package baselinecli

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
)

const (
	ExitOK    = 0
	ExitUsage = 2
)

type Config struct {
	Name       string
	Version    string
	AllowServe bool
}

type status struct {
	Component string `json:"component"`
	Status    string `json:"status"`
	Milestone string `json:"milestone"`
	Playable  bool   `json:"playable"`
}

func Run(ctx context.Context, cfg Config, args []string, stdout, stderr io.Writer) int {
	if cfg.Name == "" || cfg.Version == "" {
		fmt.Fprintln(stderr, "invalid baseline command configuration")
		return ExitUsage
	}

	if len(args) == 0 {
		writeHelp(stdout, cfg)
		return ExitOK
	}

	switch args[0] {
	case "help", "-h", "--help":
		writeHelp(stdout, cfg)
		return ExitOK
	case "version", "-v", "--version":
		fmt.Fprintf(stdout, "%s %s\n", cfg.Name, cfg.Version)
		return ExitOK
	case "health":
		result := status{
			Component: cfg.Name,
			Status:    "ok",
			Milestone: "M0",
			Playable:  false,
		}
		if err := json.NewEncoder(stdout).Encode(result); err != nil {
			fmt.Fprintf(stderr, "encode health response: %v\n", err)
			return ExitUsage
		}
		return ExitOK
	case "serve":
		if !cfg.AllowServe {
			fmt.Fprintf(stderr, "%s: serve is unavailable in this M0 shell\n", cfg.Name)
			return ExitUsage
		}
		fmt.Fprintf(stdout, "%s M0 shell ready; no playable functionality\n", cfg.Name)
		<-ctx.Done()
		return ExitOK
	default:
		fmt.Fprintf(stderr, "%s: unknown command %q\n", cfg.Name, args[0])
		writeHelp(stderr, cfg)
		return ExitUsage
	}
}

func writeHelp(w io.Writer, cfg Config) {
	fmt.Fprintf(w, "Usage: %s <command>\n\n", cfg.Name)
	fmt.Fprintln(w, "Commands:")
	fmt.Fprintln(w, "  version  Print the M0 shell version")
	fmt.Fprintln(w, "  health   Print deterministic baseline health JSON")
	if cfg.AllowServe {
		fmt.Fprintln(w, "  serve    Keep the empty process shell running")
	}
	fmt.Fprintln(w, "  help     Print this help")
}
