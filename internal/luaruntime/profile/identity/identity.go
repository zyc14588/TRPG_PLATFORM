// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package identity is the dependency-free source of supported Lua profile and
// runtime identities. It lets checkpoint validation share the same allowlist
// without importing the runtime construction package.
package identity

const (
	ProductionID    = "platform-lua-5.5-p1"
	LanguageVersion = "5.5"
	RuntimeModule   = "github.com/arnodel/golua"
	RuntimeVersion  = "v0.2.0"
	RuntimeIdentity = RuntimeModule + "@" + RuntimeVersion
)

func Supported(profileID, runtimeVersion string) bool {
	return profileID == ProductionID && runtimeVersion == RuntimeIdentity
}
