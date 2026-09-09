// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//go:build !linux || !amd64

package ipc

import (
	"errors"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"syscall"
)

func processAttributes() *syscall.SysProcAttr { return nil }
func applyProcessLimits(profile.Limits) error { return errors.New("UNSUPPORTED_CORE_PLATFORM") }
func armCPULimit(uint64) error                { return errors.New("UNSUPPORTED_CORE_PLATFORM") }
