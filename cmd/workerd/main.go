// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"context"
	"os"
	"os/signal"
	"syscall"

	"github.com/zyc14588/TRPG_PLATFORM/internal/baselinecli"
)

var version = "0.0.0-m0"

func main() {
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	if len(os.Args) > 1 && (os.Args[1] == "m2-serve" || os.Args[1] == "m2-health") {
		os.Exit(runM2Worker(ctx, os.Args[2:], os.Stdout, os.Stderr, os.Args[1] == "m2-health"))
	}
	os.Exit(baselinecli.Run(ctx, baselinecli.Config{Name: "workerd", Version: version, AllowServe: true}, os.Args[1:], os.Stdout, os.Stderr))
}
