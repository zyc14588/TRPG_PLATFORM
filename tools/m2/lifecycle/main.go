// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// This Linux evidence runner owns a disposable deployment, not authority in
// an existing installation. Default mode requires an exact clean signed head.
package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"os/signal"
	"syscall"
	"time"

	smoke "github.com/zyc14588/TRPG_PLATFORM/tests/smoke/m2"
)

func main() {
	f := flag.NewFlagSet("m2-lifecycle", flag.ContinueOnError)
	source := f.String("source", "", "exact clean signed candidate commit")
	project := f.String("project", "", "unique owned trpg-m2 project")
	output := f.String("output", "", "private report directory")
	development := f.Bool("development", false, "record an internal development run; cannot establish acceptance")
	stopWorkerd := f.Bool("stop-workerd", false, "private TEST_ONLY current workerd signal probe")
	probe := f.String("object-probe", "", "private TEST_ONLY consumer probe: baseline, unavailable or corrupt")
	config := f.String("config", "", "private test deployment configuration")
	if f.Parse(os.Args[1:]) != nil || f.NArg() != 0 || *source == "" || *project == "" {
		os.Exit(2)
	}
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	if *stopWorkerd {
		if *probe != "" || *config != "/run/operator/config.json" || *output != "" || *development {
			os.Exit(2)
		}
		ctx, cancel := context.WithTimeout(ctx, 2*time.Second)
		defer cancel()
		facts, e := smoke.RunTestOnlyWorkerdStop(ctx, *config, *source, *project, os.Stdin)
		_ = json.NewEncoder(os.Stdout).Encode(facts)
		if e != nil {
			os.Exit(1)
		}
		return
	}
	if *probe != "" {
		if (*probe != "baseline" && *probe != "unavailable" && *probe != "corrupt") || *config != "/run/operator/config.json" || *output != "" || *development {
			os.Exit(2)
		}
		ctx, cancel := context.WithTimeout(ctx, 20*time.Second)
		defer cancel()
		facts, e := smoke.RunObjectConsumerProbe(ctx, *config, *source, *project, *probe)
		_ = json.NewEncoder(os.Stdout).Encode(facts)
		if e != nil {
			os.Exit(1)
		}
		return
	}
	if *config != "" || *output == "" {
		os.Exit(2)
	}
	ctx, cancel := context.WithTimeout(ctx, 12*time.Minute)
	defer cancel()
	if e := runLifecycle(ctx, *source, *project, *output, *development); e != nil {
		fmt.Fprintln(os.Stderr, "M2_LIFECYCLE_FAILED; inspect private machine receipts")
		os.Exit(1)
	}
}
