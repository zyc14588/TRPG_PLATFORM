// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package budget

import (
	"sync"
	"testing"
)

func TestAllEightDimensionsEnforceHeldAndSpentHardLimits(t *testing.T) {
	names := []string{"calls", "tokens", "cost", "latency", "tools", "subagents", "context", "local-compute"}
	for i, name := range names {
		t.Run(name, func(t *testing.T) {
			a := [8]uint64{}
			a[i] = 1
			one := units(a)
			a[i] = 3
			cap := units(a)
			if !CanReserve(one, one, one, cap) {
				t.Fatal("exact finite cap rejected")
			}
			a[i] = 2
			if CanReserve(one, one, units(a), cap) {
				t.Fatal("reservation silently exceeded hard cap")
			}
			if CanReserve(Units{}, Units{}, one, Units{}) {
				t.Fatal("zero grant authorized consumption")
			}
		})
	}
}
func TestOverflowAndUnderflowCannotCreateBudgetCredit(t *testing.T) {
	if _, e := Add(Units{Tokens: MaxMetric}, Units{Tokens: 1}); e == nil {
		t.Fatal("overflow created credit")
	}
	if _, e := Sub(Units{}, Units{Tokens: 1}); e == nil {
		t.Fatal("underflow created credit")
	}
	if Valid(Units{CostMicros: MaxMetric + 1}) {
		t.Fatal("unbounded amount accepted")
	}
}
func TestConcurrentUpperBoundReservationNeverSpendsUncertainHold(t *testing.T) {
	var mu sync.Mutex
	used, held := Units{}, Units{Calls: 1}
	cap := Units{Calls: 4}
	success := 0
	var wg sync.WaitGroup
	for range 32 {
		wg.Add(1)
		go func() {
			defer wg.Done()
			mu.Lock()
			defer mu.Unlock()
			if CanReserve(used, held, Units{Calls: 1}, cap) {
				held, _ = Add(held, Units{Calls: 1})
				success++
			}
		}()
	}
	wg.Wait()
	if success != 3 || held.Calls != 4 {
		t.Fatal("uncertain hold was freed or concurrent cap exceeded")
	}
}
