// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"context"
	"io"
	"os"

	"github.com/zyc14588/TRPG_PLATFORM/internal/baselinecli"
)

var version = "0.1.0-m1"

func main() {
	os.Exit(run(context.Background(), os.Args[1:], os.Stdout, os.Stderr))
}

func run(ctx context.Context, args []string, stdout, stderr io.Writer) int {
	if len(args) > 0 && args[0] == "package" {
		return runPackage(ctx, args[1:], stdout, stderr)
	}
	return baselinecli.Run(ctx, baselinecli.Config{Name: "creator-cli", Version: version}, args, stdout, stderr)
}
