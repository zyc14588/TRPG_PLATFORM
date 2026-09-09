// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//go:build linux && amd64

package ipc

import (
	"syscall"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"golang.org/x/sys/unix"
)

func processAttributes() *syscall.SysProcAttr {
	return &syscall.SysProcAttr{Setpgid: true, Pdeathsig: syscall.SIGKILL}
}

// RLIMIT_DATA is a kernel-enforced ceiling for the data segment and anonymous
// writable mappings, including Go heap allocations. It is not GOMEMLIMIT (soft).
// The pure Go runtime needs large address reservations, so RLIMIT_AS is unsuitable.
func applyProcessLimits(l profile.Limits) error {
	if err := l.Validate(); err != nil {
		return err
	}
	if err := unix.Setrlimit(unix.RLIMIT_CORE, &unix.Rlimit{Cur: 0, Max: 0}); err != nil {
		return err
	}
	if err := unix.Setrlimit(unix.RLIMIT_DATA, &unix.Rlimit{Cur: l.MemoryBytes, Max: l.MemoryBytes}); err != nil {
		return err
	}
	// prctl is per-thread. Apply to every Go runtime thread and inherit on new
	// threads, rather than accidentally protecting only the current goroutine.
	_, _, errno := syscall.AllThreadsSyscall6(unix.SYS_PRCTL, unix.PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0, 0)
	if errno != 0 {
		return errno
	}
	return nil
}

func armCPULimit(seconds uint64) error {
	var usage unix.Rusage
	if err := unix.Getrusage(unix.RUSAGE_SELF, &usage); err != nil {
		return err
	}
	used := uint64(usage.Utime.Sec+usage.Stime.Sec) + (uint64(usage.Utime.Usec+usage.Stime.Usec)+999999)/1000000
	var current unix.Rlimit
	if err := unix.Getrlimit(unix.RLIMIT_CPU, &current); err != nil {
		return err
	}
	// SIGXCPU has its default terminating disposition in the runner and cannot
	// be intercepted by a Lua package. Preserve any stricter inherited hard limit.
	limit := used + seconds
	if current.Max < limit {
		limit = current.Max
	}
	return unix.Setrlimit(unix.RLIMIT_CPU, &unix.Rlimit{Cur: limit, Max: current.Max})
}
