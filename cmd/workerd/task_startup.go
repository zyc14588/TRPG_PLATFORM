// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"context"
	"flag"
	"fmt"
	"io"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/gateway"
	"github.com/zyc14588/TRPG_PLATFORM/internal/deployment/m2"
)

func runM2Worker(ctx context.Context, args []string, out, stderr io.Writer, health bool) int {
	f := flag.NewFlagSet("m2-serve", flag.ContinueOnError)
	f.SetOutput(stderr)
	path := f.String("config", "", "operator configuration reference")
	if f.Parse(args) != nil || f.NArg() != 0 {
		return 2
	}
	c, e := m2.LoadConfig(*path)
	if e != nil || m2.RequireLinux() != nil {
		fmt.Fprintln(stderr, "M2 worker configuration rejected")
		return 1
	}
	client, e := m2.NewWorkerClient(c)
	if e != nil {
		return 1
	}
	defer client.Close()
	if health {
		if client.Probe(ctx) != nil {
			return 1
		}
		return 0
	}
	egress, e := gateway.NewEgress(c.Provider.Adapter())
	if e != nil {
		fmt.Fprintln(stderr, "M2 egress configuration rejected")
		return 1
	}
	defer egress.Close()
	_, _ = fmt.Fprintln(out, "M2 bounded provider worker started")
	if e = client.Run(ctx, egress); e != nil && ctx.Err() == nil {
		fmt.Fprintln(stderr, "M2 worker unavailable")
		return 1
	}
	return 0
}
