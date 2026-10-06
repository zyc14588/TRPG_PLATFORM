// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package capability_test

import (
	"errors"
	"fmt"
	"reflect"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
)

func TestClosedHostRegistryMatchesNormativeCategories(t *testing.T) {
	t.Parallel()
	want := []capability.Name{
		capability.HostAI, capability.HostContent, capability.HostDB,
		capability.HostEvent, capability.HostLog, capability.HostRandom,
		capability.HostRules, capability.HostState, capability.HostTask, capability.HostTime,
	}
	if got := capability.RegisteredNames(); !reflect.DeepEqual(got, want) {
		t.Fatalf("registered Host categories = %v, want %v", got, want)
	}
	for _, name := range want {
		if got, err := capability.ParseName(string(name)); err != nil || got != name {
			t.Fatalf("ParseName(%q) = %q, %v", name, got, err)
		}
	}
}

// Test every category against every combination of the three authorization
// layers, including trust grants present only at a different trust level.
func TestHostRegistryNeverGrantsOutsideThreeLayerIntersection(t *testing.T) {
	t.Parallel()
	levels := []capability.TrustLevel{
		capability.TrustOfficial, capability.TrustSigned,
		capability.TrustPrivateUnverified, capability.TrustDevelopment,
	}
	for _, name := range capability.RegisteredNames() {
		t.Run(string(name), func(t *testing.T) {
			t.Parallel()
			for _, level := range levels {
				t.Run(string(level), func(t *testing.T) {
					t.Parallel()
					for mask := 0; mask < 8; mask++ {
						t.Run(fmt.Sprintf("decl-trust-context-%03b", mask), func(t *testing.T) {
							t.Parallel()
							raw := string(name)
							var declared, contextNames []string
							if mask&1 != 0 {
								declared = []string{raw}
							}
							if mask&4 != 0 {
								contextNames = []string{raw}
							}
							context, err := capability.NewGrantSet(contextNames)
							if err != nil {
								t.Fatal(err)
							}
							grants := make(map[capability.TrustLevel][]string)
							for _, other := range levels {
								grants[other] = []string{raw}
							}
							if mask&2 == 0 {
								grants[level] = nil
							}
							policy, err := capability.NewTrustPolicy(grants)
							if err != nil {
								t.Fatal(err)
							}
							declaration, err := capability.NewDeclaration(declared, nil)
							if err != nil {
								t.Fatal(err)
							}
							resolution, err := capability.Resolve(declaration, level, policy, context)
							if mask&1 != 0 && mask != 7 {
								var missing *capability.MissingRequiredError
								if !errors.As(err, &missing) || !reflect.DeepEqual(missing.Names, []capability.Name{name}) || len(resolution.Effective.Names()) != 0 {
									t.Fatalf("required missing layer granted: resolution=%+v error=%v", resolution, err)
								}
							} else if err != nil || resolution.Effective.Contains(name) != (mask == 7) || len(resolution.Effective.Names()) != boolCount(mask == 7) {
								t.Fatalf("required intersection: resolution=%+v error=%v", resolution, err)
							}

							const fallback = "disable this category deterministically"
							var optional []capability.OptionalSpec
							if mask&1 != 0 {
								optional = []capability.OptionalSpec{{Name: raw, Fallback: fallback}}
							}
							declaration, err = capability.NewDeclaration(nil, optional)
							if err != nil {
								t.Fatal(err)
							}
							resolution, err = capability.Resolve(declaration, level, policy, context)
							if err != nil || resolution.Effective.Contains(name) != (mask == 7) || len(resolution.Effective.Names()) != boolCount(mask == 7) {
								t.Fatalf("optional intersection: resolution=%+v error=%v", resolution, err)
							}
							if mask&1 != 0 && mask != 7 {
								want := []capability.Fallback{{Name: name, Behavior: fallback}}
								if !reflect.DeepEqual(resolution.Fallbacks, want) {
									t.Fatalf("optional denied without exact fallback: %v", resolution.Fallbacks)
								}
							} else if len(resolution.Fallbacks) != 0 {
								t.Fatalf("unexpected optional fallback: %v", resolution.Fallbacks)
							}
						})
					}
				})
			}
		})
	}
}

func boolCount(value bool) int {
	if value {
		return 1
	}
	return 0
}

func TestHostRegistryPreservesZeroGrantDefaults(t *testing.T) {
	t.Parallel()
	context, err := capability.NewGrantSet(nil)
	if err != nil {
		t.Fatal(err)
	}
	policy := testPolicy(t, nil)
	for _, name := range capability.RegisteredNames() {
		t.Run(string(name), func(t *testing.T) {
			t.Parallel()
			declaration, err := capability.NewDeclaration([]string{string(name)}, nil)
			if err != nil {
				t.Fatal(err)
			}
			if resolution, err := capability.Resolve(declaration, capability.TrustOfficial, policy, context); err == nil || len(resolution.Effective.Names()) != 0 {
				t.Fatalf("registry introduced default grants: %+v, %v", resolution, err)
			}
		})
	}
}

func TestHostRegistryKeepsPrivilegedAndUnknownNamesClosed(t *testing.T) {
	t.Parallel()
	for _, raw := range []string{"host.network", "host.filesystem", "host.db.raw-sql", "host.db.ddl", "host.task.execute", "host.unknown"} {
		t.Run(raw, func(t *testing.T) {
			t.Parallel()
			if _, err := capability.NewDeclaration([]string{raw}, nil); err == nil {
				t.Fatal("unregistered required declaration accepted")
			}
			if _, err := capability.NewDeclaration(nil, []capability.OptionalSpec{{Name: raw, Fallback: "disable"}}); err == nil {
				t.Fatal("unregistered optional declaration accepted")
			}
			if _, err := capability.NewGrantSet([]string{raw}); err == nil {
				t.Fatal("unregistered execution grant accepted")
			}
			if _, err := capability.NewTrustPolicy(map[capability.TrustLevel][]string{
				capability.TrustOfficial: {raw}, capability.TrustSigned: {raw},
				capability.TrustPrivateUnverified: {raw}, capability.TrustDevelopment: {raw},
			}); err == nil {
				t.Fatal("unregistered trust grant accepted")
			}
			declaration := capability.Declaration{Required: []capability.Name{capability.Name(raw)}}
			context, err := capability.NewGrantSet(nil)
			if err != nil {
				t.Fatal(err)
			}
			if resolution, err := capability.Resolve(declaration, capability.TrustOfficial, testPolicy(t, nil), context); err == nil || len(resolution.Effective.Names()) != 0 {
				t.Fatalf("directly constructed unknown name reached resolution: %+v, %v", resolution, err)
			}
		})
	}
}
