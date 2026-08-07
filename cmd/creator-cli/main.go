// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"context"
	"os"

	"github.com/zyc14588/TRPG_PLATFORM/internal/baselinecli"
)

var version = "0.0.0-m0"

func main() {
	os.Exit(baselinecli.Run(context.Background(), baselinecli.Config{Name: "creator-cli", Version: version}, os.Args[1:], os.Stdout, os.Stderr))
}
