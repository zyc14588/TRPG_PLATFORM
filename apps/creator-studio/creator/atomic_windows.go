// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//go:build windows

package creator

import "golang.org/x/sys/windows"

func atomicPublish(temporary, target string) error {
	return moveFile(temporary, target, windows.MOVEFILE_WRITE_THROUGH)
}

func atomicReplace(temporary, target string) error {
	return moveFile(temporary, target, windows.MOVEFILE_REPLACE_EXISTING|windows.MOVEFILE_WRITE_THROUGH)
}

func moveFile(temporary, target string, flags uint32) error {
	from, err := windows.UTF16PtrFromString(temporary)
	if err != nil {
		return err
	}
	to, err := windows.UTF16PtrFromString(target)
	if err != nil {
		return err
	}
	return windows.MoveFileEx(from, to, flags)
}

// MOVEFILE_WRITE_THROUGH makes the replacement durable before it returns.
func syncParentDirectory(string) error { return nil }
