// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//go:build !production

package main

import (
	"embed"
	"io/fs"
)

//go:embed all:frontend/fallback
var developmentAssets embed.FS

func frontendAssets() (fs.FS, error) {
	return fs.Sub(developmentAssets, "frontend/fallback")
}
