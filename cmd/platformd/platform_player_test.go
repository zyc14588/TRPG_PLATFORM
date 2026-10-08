// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package main

import (
	"context"
	"testing"
)

func TestPlayerPrivateCompositionRequiresActualOwners(t *testing.T) {
	if _, e := platformPlayers(platformPlayerOptions{ctx: context.Background()}); e == nil {
		t.Fatal("incomplete composition started")
	}
	if _, _, _, e := platformPlayerTasks(context.Background(), nil, nil, nil, nil); e == nil {
		t.Fatal("unowned task composition admitted")
	}
}
