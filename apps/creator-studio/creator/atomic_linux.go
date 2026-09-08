// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//go:build linux

package creator

import "golang.org/x/sys/unix"

func atomicPublish(temporary, target string) error {
	return unix.Renameat2(unix.AT_FDCWD, temporary, unix.AT_FDCWD, target, unix.RENAME_NOREPLACE)
}
