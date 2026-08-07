// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//go:build production

package main

import (
	"embed"
	"io/fs"
)

//go:embed all:frontend/dist
var productionAssets embed.FS

func frontendAssets() (fs.FS, error) {
	return fs.Sub(productionAssets, "frontend/dist")
}
