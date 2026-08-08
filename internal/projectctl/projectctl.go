// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package projectctl owns the cross-platform M0 engineering gates. Just and
// CI are deliberately thin callers of this package.
package projectctl

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

type App struct {
	root   string
	stdout io.Writer
	stderr io.Writer
}

func New(stdout, stderr io.Writer) (*App, error) {
	root, err := findRoot()
	if err != nil {
		return nil, err
	}
	return &App{root: root, stdout: stdout, stderr: stderr}, nil
}

func findRoot() (string, error) {
	dir, err := os.Getwd()
	if err != nil {
		return "", fmt.Errorf("get working directory: %w", err)
	}
	for {
		if info, statErr := os.Stat(filepath.Join(dir, "go.mod")); statErr == nil && !info.IsDir() {
			return dir, nil
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			return "", errors.New("could not find repository go.mod")
		}
		dir = parent
	}
}

func (a *App) Run(ctx context.Context, args []string) error {
	if len(args) == 0 || args[0] == "help" || args[0] == "--help" || args[0] == "-h" {
		a.usage()
		return nil
	}

	switch args[0] {
	case "bootstrap":
		return a.bootstrap(ctx, args[1:])
	case "env":
		if !exactArgs(args[1:], "check") {
			return usageError("env check")
		}
		return a.checkEnvironment(ctx)
	case "docs":
		return a.docsCommand(args[1:])
	case "decisions":
		if !exactArgs(args[1:], "check") {
			return usageError("decisions check")
		}
		return a.checkDecisions()
	case "traceability":
		if !exactArgs(args[1:], "check") {
			return usageError("traceability check")
		}
		return a.checkTraceability()
	case "license":
		if !exactArgs(args[1:], "check") {
			return usageError("license check")
		}
		return a.checkLicense(ctx)
	case "scope":
		if err := requireM0(args[1:]); err != nil {
			return fmt.Errorf("scope check: %w", err)
		}
		return a.checkScope(ctx)
	case "check":
		if len(args) != 1 {
			return usageError("check")
		}
		return a.checkAll(ctx)
	case "build":
		if len(args) != 1 {
			return usageError("build")
		}
		return a.build(ctx)
	case "test":
		if len(args) != 1 {
			return usageError("test")
		}
		return a.test(ctx)
	case "ci":
		if len(args) != 1 {
			return usageError("ci")
		}
		return a.ci(ctx)
	case "codex":
		return a.codexCommand(ctx, args[1:])
	case "accept":
		if err := requireM0(args[1:]); err != nil {
			return fmt.Errorf("accept: %w", err)
		}
		return a.acceptM0(ctx)
	default:
		return fmt.Errorf("unknown command %q; run projectctl --help", args[0])
	}
}

func (a *App) usage() {
	fmt.Fprintln(a.stdout, `projectctl - TRPG Platform engineering control plane

Usage:
  projectctl bootstrap
  projectctl env check
  projectctl docs generate|check
  projectctl decisions check
  projectctl traceability check
  projectctl license check
  projectctl scope check --milestone M0
  projectctl check|build|test|ci
  projectctl codex plan --milestone M?
  projectctl codex route --mode PLAN --milestone M?
  projectctl codex route --mode IMPLEMENT|ACCEPT|REPAIR --milestone M? --batch M?-B???
  projectctl codex check
  projectctl accept --milestone M0`)
}

func exactArgs(args []string, want ...string) bool {
	if len(args) != len(want) {
		return false
	}
	for i := range want {
		if args[i] != want[i] {
			return false
		}
	}
	return true
}

func usageError(usage string) error {
	return fmt.Errorf("usage: projectctl %s", usage)
}

func requireM0(args []string) error {
	if !exactArgs(args, "check", "--milestone", "M0") && !exactArgs(args, "--milestone", "M0") {
		return errors.New("exactly --milestone M0 is required")
	}
	return nil
}

func cleanInline(value string) string {
	value = strings.ReplaceAll(value, "|", `\|`)
	value = strings.ReplaceAll(value, "\r", " ")
	value = strings.ReplaceAll(value, "\n", " ")
	return strings.TrimSpace(value)
}
