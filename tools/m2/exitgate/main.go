// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package main

import (
	"context"
	"flag"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/tools/m2/exitproof"
	"os"
	"os/signal"
	"syscall"
	"time"
)

func main() {
	f := flag.NewFlagSet("m2-exitgate", flag.ContinueOnError)
	source := f.String("source", "", "exact clean signed candidate SHA")
	output := f.String("output", "", "new absolute private evidence directory")
	project := f.String("project", "", "unique owned trpg-m2 lifecycle project")
	if f.Parse(os.Args[1:]) != nil || f.NArg() != 0 {
		os.Exit(2)
	}
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	ctx, cancel := context.WithTimeout(ctx, 70*time.Minute)
	defer cancel()
	if e := exitproof.Run(ctx, exitproof.Options{Source: *source, Output: *output, Project: *project}); e != nil {
		fmt.Fprintln(os.Stderr, "M2_EXIT_FAILED: inspect private source-bound command report")
		os.Exit(1)
	}
	fmt.Println("M2_EXIT_PRODUCER_PASS: 13 original TEST IDs; independent ACCEPT and native state reception still required")
}
