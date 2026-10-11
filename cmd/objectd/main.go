// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"context"
	"flag"
	"fmt"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/deployment/m2"
)

func main() {
	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()
	os.Exit(run(ctx, os.Args[1:]))
}
func run(ctx context.Context, args []string) int {
	f := flag.NewFlagSet("objectd", flag.ContinueOnError)
	mode := f.String("mode", "serve", "serve or health")
	path := f.String("config", "", "operator configuration reference")
	if f.Parse(args) != nil || f.NArg() != 0 {
		return 2
	}
	c, e := m2.LoadConfig(*path)
	if e != nil || m2.RequireLinux() != nil {
		return 1
	}
	if *mode == "health" {
		client, e := m2.NewObjectClient(c)
		if e != nil {
			return 1
		}
		defer client.Close()
		if client.Ready(ctx) != nil {
			return 1
		}
		return 0
	}
	if *mode != "serve" {
		return 2
	}
	objects, e := m2.NewObjects(c.ObjectRoot)
	if e != nil {
		return 1
	}
	defer objects.Close()
	tls, e := m2.TLSConfig(c.TLS, "platformd|object-storage", true)
	if e != nil {
		return 1
	}
	server, e := m2.ServeUnix(ctx, c.ObjectSocket, tls, c.PeerUID, objects.Handler())
	if e != nil {
		return 1
	}
	fmt.Fprintln(os.Stdout, "M2 private immutable object service started")
	<-ctx.Done()
	closeCtx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()
	if server.Close(closeCtx) != nil {
		return 1
	}
	return 0
}
