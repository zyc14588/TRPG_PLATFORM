// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package hostapi

import (
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"time"
)

type Budget struct {
	Callbacks     int
	Patches       int
	Rows          int
	DataBytes     int
	Events        int
	Tasks         int
	Continuations int
	Outbox        int
	OutputBytes   int
}

func DefaultBudget() Budget {
	return Budget{Callbacks: 256, Patches: 128, Rows: 128, DataBytes: 256 << 10, Events: 64, Tasks: 32, Continuations: 32, Outbox: 64, OutputBytes: 64 << 10}
}
func (b Budget) Validate() error {
	d := DefaultBudget()
	x := []int{b.Callbacks, b.Patches, b.Rows, b.DataBytes, b.Events, b.Tasks, b.Continuations, b.Outbox, b.OutputBytes}
	y := []int{d.Callbacks, d.Patches, d.Rows, d.DataBytes, d.Events, d.Tasks, d.Continuations, d.Outbox, d.OutputBytes}
	for i, n := range x {
		if n < 1 || n > y[i] {
			return profile.Fail(profile.ErrConfiguration)
		}
	}
	return nil
}

type AuditPolicy struct {
	Level           string
	Development     bool
	AuthorizedUntil time.Time
}

func (a AuditPolicy) validate(now time.Time) error {
	switch a.Level {
	case "", "AUDIT-0", "AUDIT-1":
		return nil
	case "AUDIT-2":
		if a.AuthorizedUntil.After(now) && a.AuthorizedUntil.Sub(now) <= time.Hour {
			return nil
		}
	case "AUDIT-3":
		if a.Development {
			return nil
		}
	}
	return profile.Fail("AUDIT_POLICY_REJECTED")
}
