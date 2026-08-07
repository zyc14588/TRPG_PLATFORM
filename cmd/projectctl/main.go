// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"context"
	"fmt"
	"os"

	"github.com/zyc14588/TRPG_PLATFORM/internal/projectctl"
)

func main() {
	app, err := projectctl.New(os.Stdout, os.Stderr)
	if err != nil {
		fmt.Fprintln(os.Stderr, "projectctl:", err)
		os.Exit(1)
	}
	if err := app.Run(context.Background(), os.Args[1:]); err != nil {
		fmt.Fprintln(os.Stderr, "projectctl:", err)
		os.Exit(1)
	}
}
