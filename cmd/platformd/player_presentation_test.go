// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package main

import (
	"context"
	"testing"
)

func TestPresentationCompositionRequiresActualModelService(t *testing.T) {
	if _, e := platformPlayersWithPresentation(platformPlayerOptions{ctx: context.Background()}, nil, nil, nil); e == nil {
		t.Fatal("unbound model composition admitted")
	}
}
