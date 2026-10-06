// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"context"
	"fmt"
	"os"
	"os/signal"
	"syscall"

	"github.com/zyc14588/TRPG_PLATFORM/internal/baselinecli"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/ipc"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
)

var version = "0.1.0-m1-b002"

func main() {
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	if len(os.Args) == 2 && os.Args[1] == "licenses" {
		fmt.Fprint(os.Stdout, profile.ThirdPartyNotices)
		return
	}
	if len(os.Args) > 1 && os.Args[1] == "evidence" {
		os.Exit(evidenceMain(os.Args[2:], os.Stdout, os.Stderr))
	}
	if len(os.Args) == 2 && os.Args[1] == "serve" {
		if err := ipc.Serve(ctx, os.Stdin, os.Stdout); err != nil {
			fmt.Fprintln(os.Stderr, "lua-runner: execution boundary rejected")
			os.Exit(1)
		}
		return
	}
	os.Exit(baselinecli.Run(ctx, baselinecli.Config{Name: "lua-runner", Version: version}, os.Args[1:], os.Stdout, os.Stderr))
}
