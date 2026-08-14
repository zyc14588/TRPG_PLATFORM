// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//go:build !windows

package creator

import "os"

func atomicReplace(temporary, target string) error {
	return os.Rename(temporary, target)
}

func syncParentDirectory(directory string) error {
	parent, err := os.Open(directory)
	if err != nil {
		return err
	}
	if err := parent.Sync(); err != nil {
		_ = parent.Close()
		return err
	}
	return parent.Close()
}
