// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package capability_test

import (
	"errors"
	"reflect"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
)

func testPolicy(t *testing.T) capability.TrustPolicy {
	t.Helper()
	policy, err := capability.NewTrustPolicy(map[capability.TrustLevel][]string{
		capability.TrustOfficial:          {"host.ai", "host.log", "host.state"},
		capability.TrustSigned:            {"host.log", "host.state"},
		capability.TrustPrivateUnverified: {"host.state"},
		capability.TrustDevelopment:       {"host.log", "host.state"},
	})
	if err != nil {
		t.Fatal(err)
	}
	return policy
}

func TestEffectiveCapabilitiesAreThreeWayIntersection(t *testing.T) {
	t.Parallel()
	declaration, err := capability.NewDeclaration(
		[]string{"host.state"},
		[]capability.OptionalSpec{{Name: "host.log", Fallback: "continue without logs"}},
	)
	if err != nil {
		t.Fatal(err)
	}
	execution, err := capability.NewGrantSet([]string{"host.ai", "host.state"})
	if err != nil {
		t.Fatal(err)
	}
	resolution, err := capability.Resolve(declaration, capability.TrustOfficial, testPolicy(t), execution)
	if err != nil {
		t.Fatal(err)
	}
	if got, want := resolution.Effective.Names(), []capability.Name{"host.state"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("effective = %#v, want %#v", got, want)
	}
	if len(resolution.Fallbacks) != 1 || resolution.Fallbacks[0].Name != "host.log" || resolution.Fallbacks[0].Behavior != "continue without logs" {
		t.Fatalf("fallbacks = %#v", resolution.Fallbacks)
	}
}

func TestMissingRequiredCapabilityRejectsResolution(t *testing.T) {
	t.Parallel()
	declaration, err := capability.NewDeclaration([]string{"host.state"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	execution, err := capability.NewGrantSet(nil)
	if err != nil {
		t.Fatal(err)
	}
	_, err = capability.Resolve(declaration, capability.TrustOfficial, testPolicy(t), execution)
	var missing *capability.MissingRequiredError
	if !errors.As(err, &missing) || !reflect.DeepEqual(missing.Names, []capability.Name{"host.state"}) {
		t.Fatalf("Resolve error = %#v", err)
	}
}

func TestOptionalCapabilityRequiresFallback(t *testing.T) {
	t.Parallel()
	if _, err := capability.NewDeclaration(nil, []capability.OptionalSpec{{Name: "host.log"}}); err == nil {
		t.Fatal("optional capability without fallback succeeded")
	}
}

func TestTrustPolicyFailsClosedWhenLevelIsMissing(t *testing.T) {
	t.Parallel()
	_, err := capability.NewTrustPolicy(map[capability.TrustLevel][]string{
		capability.TrustOfficial: {"host.state"},
	})
	if err == nil {
		t.Fatal("partial trust policy succeeded")
	}
}
