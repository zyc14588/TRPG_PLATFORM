// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//go:build darwin

package creator

import "golang.org/x/sys/unix"

func atomicPublish(temporary, target string) error {
	return unix.RenameatxNp(unix.AT_FDCWD, temporary, unix.AT_FDCWD, target, unix.RENAME_EXCL)
}
