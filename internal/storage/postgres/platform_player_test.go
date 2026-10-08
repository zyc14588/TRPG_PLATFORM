// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package postgres

import (
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
)

func TestPlayerStorageConstructorsRejectMissingOrMixedOwnership(t *testing.T) {
	if _, e := NewPlatformPlayerStorage(nil); e != auth.ErrInvalid {
		t.Fatal("missing owner admitted")
	}
	if _, e := NewControlledLaunchStorage(nil, nil); e != auth.ErrInvalid {
		t.Fatal("missing native owner admitted")
	}
	if _, e := NewControlledPlayerTasks(nil, nil); e == nil {
		t.Fatal("missing task owner admitted")
	}
	for _, v := range []string{"connection-", "connection-untrusted", "../connection-id", "connection-0123456789abcdef0123456789abcdef-extra"} {
		if playerConnectionID.MatchString(v) {
			t.Fatal("unbounded connection ID accepted")
		}
	}
}
