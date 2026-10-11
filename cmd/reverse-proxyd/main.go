// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"context"
	"crypto/tls"
	"flag"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
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
	f := flag.NewFlagSet("reverse-proxyd", flag.ContinueOnError)
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
		if m2.ProbeDaemon(ctx, c, "reverse-proxy") != nil {
			return 1
		}
		return 0
	}
	if *mode != "serve" {
		return 2
	}
	proxy, e := m2.NewProxy(c)
	if e != nil {
		return 1
	}
	defer proxy.Close()
	config, e := m2.ExternalTLS(c.TLS)
	if e != nil {
		return 1
	}
	l, e := net.Listen("tcp", c.ExternalAddress)
	if e != nil {
		return 1
	}
	s := &http.Server{Handler: proxy.Handler(), TLSConfig: config, ReadHeaderTimeout: 2 * time.Second, ReadTimeout: 7 * time.Second, WriteTimeout: 7 * time.Second, IdleTimeout: 10 * time.Second, MaxHeaderBytes: 16384, ErrorLog: log.New(io.Discard, "", 0), BaseContext: func(net.Listener) context.Context { return ctx }}
	done := make(chan error, 1)
	go func() { done <- s.Serve(tls.NewListener(l, config)) }()
	fmt.Fprintln(os.Stdout, "M2 same-origin HTTPS proxy started")
	select {
	case <-ctx.Done():
		stop, cancel := context.WithTimeout(context.Background(), 3*time.Second)
		defer cancel()
		if s.Shutdown(stop) != nil {
			_ = s.Close()
		}
		<-done
		return 0
	case <-done:
		return 1
	}
}
