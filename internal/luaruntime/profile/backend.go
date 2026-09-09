// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package profile

import (
	_ "embed"
	"runtime/debug"
)

//go:embed THIRD_PARTY_NOTICES.txt
var ThirdPartyNotices string

// CheckBackend verifies the executable's dependency identity, including rejects
// for local replacement modules that could silently change the versioned profile.
func CheckBackend() error {
	info, ok := debug.ReadBuildInfo()
	if !ok || info.GoVersion != "go1.26.5" {
		return Fail(ErrConfiguration)
	}
	for _, dep := range info.Deps {
		if dep.Path == "github.com/iceisfun/golua/v2" {
			if dep.Version == "v2.0.5" && dep.Replace == nil && dep.Sum == "h1:9j1YsuOO114OVDITfsAK5RMpfWzZcVLV+S86Kenn+ts=" {
				return nil
			}
			return Fail(ErrConfiguration)
		}
	}
	return Fail(ErrConfiguration)
}
