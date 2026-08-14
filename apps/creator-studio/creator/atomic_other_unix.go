// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//go:build !windows && !linux && !darwin

package creator

import "errors"

func atomicPublish(string, string) error {
	return errors.New("atomic no-clobber publish is unavailable on this platform")
}
