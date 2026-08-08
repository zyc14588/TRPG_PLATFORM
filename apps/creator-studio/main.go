// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"fmt"
	"os"

	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"
)

func main() {
	assets, err := frontendAssets()
	if err != nil {
		fmt.Fprintf(os.Stderr, "creator-studio: prepare baseline assets: %v\n", err)
		os.Exit(1)
	}

	err = wails.Run(&options.App{
		Title:            "Creator Studio",
		Width:            1200,
		Height:           760,
		MinWidth:         640,
		MinHeight:        480,
		BackgroundColour: &options.RGBA{R: 242, G: 245, B: 249, A: 1},
		AssetServer:      &assetserver.Options{Assets: assets},
	})
	if err != nil {
		fmt.Fprintf(os.Stderr, "creator-studio: start Wails shell: %v\n", err)
		os.Exit(1)
	}
}
