// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package capability_test

import (
	"errors"
	"reflect"
	"slices"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/capability"
)

func testPolicy(t *testing.T, official []string) capability.TrustPolicy {
	t.Helper()
	policy, err := capability.NewTrustPolicy(map[capability.TrustLevel][]string{
		capability.TrustOfficial:          official,
		capability.TrustSigned:            {},
		capability.TrustPrivateUnverified: {},
		capability.TrustDevelopment:       {},
	})
	if err != nil {
		t.Fatal(err)
	}
	return policy
}

func TestCapabilityResolutionMatrix(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name          string
		required      []string
		optional      []capability.OptionalSpec
		trust         []string
		execution     []string
		wantEffective []capability.Name
		wantFallbacks []capability.Name
		wantMissing   []capability.Name
	}{
		{
			name: "full intersection", required: []string{"host.state"},
			optional: []capability.OptionalSpec{{Name: "host.log", Fallback: "continue without logs"}},
			trust:    []string{"host.event", "host.log", "host.state"}, execution: []string{"host.log", "host.state"},
			wantEffective: []capability.Name{capability.HostLog, capability.HostState},
		},
		{
			name: "partial intersection", required: []string{"host.state"},
			optional: []capability.OptionalSpec{
				{Name: "host.log", Fallback: "continue without logs"},
				{Name: "host.event", Fallback: "continue without events"},
			},
			trust: []string{"host.log", "host.state"}, execution: []string{"host.event", "host.state"},
			wantEffective: []capability.Name{capability.HostState},
			wantFallbacks: []capability.Name{capability.HostEvent, capability.HostLog},
		},
		{
			name: "empty declared set", trust: []string{"host.event", "host.log", "host.state"},
			execution: []string{"host.event", "host.log", "host.state"}, wantEffective: []capability.Name{},
		},
		{
			name:      "empty trust set",
			optional:  []capability.OptionalSpec{{Name: "host.log", Fallback: "continue without logs"}},
			execution: []string{"host.log"}, wantEffective: []capability.Name{}, wantFallbacks: []capability.Name{capability.HostLog},
		},
		{
			name:     "empty execution context",
			optional: []capability.OptionalSpec{{Name: "host.log", Fallback: "continue without logs"}},
			trust:    []string{"host.log"}, wantEffective: []capability.Name{}, wantFallbacks: []capability.Name{capability.HostLog},
		},
		{
			name: "missing required", required: []string{"host.event", "host.state"},
			trust: []string{"host.state"}, execution: []string{"host.event", "host.state"},
			wantMissing: []capability.Name{capability.HostEvent},
		},
		{
			name: "optional fallback", optional: []capability.OptionalSpec{{Name: "host.log", Fallback: "continue without logs"}},
			trust: []string{"host.log"}, wantEffective: []capability.Name{}, wantFallbacks: []capability.Name{capability.HostLog},
		},
		{
			name: "no escalation", required: []string{"host.state"},
			trust: []string{"host.event", "host.log", "host.state"}, execution: []string{"host.event", "host.log", "host.state"},
			wantEffective: []capability.Name{capability.HostState},
		},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			declaration, err := capability.NewDeclaration(test.required, test.optional)
			if err != nil {
				t.Fatal(err)
			}
			execution, err := capability.NewGrantSet(test.execution)
			if err != nil {
				t.Fatal(err)
			}
			resolution, err := capability.Resolve(declaration, capability.TrustOfficial, testPolicy(t, test.trust), execution)
			if len(test.wantMissing) > 0 {
				var missing *capability.MissingRequiredError
				if !errors.As(err, &missing) || !reflect.DeepEqual(missing.Names, test.wantMissing) {
					t.Fatalf("Resolve error = %#v, want missing %#v", err, test.wantMissing)
				}
				return
			}
			if err != nil {
				t.Fatal(err)
			}
			if got := resolution.Effective.Names(); !reflect.DeepEqual(got, test.wantEffective) {
				t.Fatalf("effective = %#v, want %#v", got, test.wantEffective)
			}
			fallbacks := make([]capability.Name, len(resolution.Fallbacks))
			for index, fallback := range resolution.Fallbacks {
				fallbacks[index] = fallback.Name
				if fallback.Behavior == "" {
					t.Fatalf("fallback for %q has no behavior", fallback.Name)
				}
			}
			if !slices.Equal(fallbacks, test.wantFallbacks) {
				t.Fatalf("fallbacks = %#v, want %#v", fallbacks, test.wantFallbacks)
			}
		})
	}
}

func TestCapabilityInputOrderingDoesNotAffectResolution(t *testing.T) {
	t.Parallel()
	resolve := func(required, trust, execution []string, optional []capability.OptionalSpec) capability.Resolution {
		declaration, err := capability.NewDeclaration(required, optional)
		if err != nil {
			t.Fatal(err)
		}
		context, err := capability.NewGrantSet(execution)
		if err != nil {
			t.Fatal(err)
		}
		resolution, err := capability.Resolve(declaration, capability.TrustOfficial, testPolicy(t, trust), context)
		if err != nil {
			t.Fatal(err)
		}
		return resolution
	}
	first := resolve(
		[]string{"host.state", "host.event"},
		[]string{"host.log", "host.state", "host.event"},
		[]string{"host.event", "host.log", "host.state"},
		[]capability.OptionalSpec{{Name: "host.log", Fallback: "continue without logs"}},
	)
	second := resolve(
		[]string{"host.event", "host.state"},
		[]string{"host.event", "host.state", "host.log"},
		[]string{"host.state", "host.event", "host.log"},
		[]capability.OptionalSpec{{Name: "host.log", Fallback: "continue without logs"}},
	)
	if !reflect.DeepEqual(first.Effective.Names(), second.Effective.Names()) || !reflect.DeepEqual(first.Fallbacks, second.Fallbacks) {
		t.Fatalf("ordering changed resolution: %#v != %#v", first, second)
	}
}

func TestCapabilityRegistryRejectsUnknownNamesAtEveryBoundary(t *testing.T) {
	t.Parallel()
	for _, unknown := range []string{"host.unknown", "host.network"} {
		unknown := unknown
		t.Run(unknown, func(t *testing.T) {
			t.Parallel()
			if _, err := capability.ParseName(unknown); err == nil {
				t.Fatal("syntactically valid unregistered capability succeeded")
			}
			if _, err := capability.NewDeclaration([]string{unknown}, nil); err == nil {
				t.Fatal("unknown required capability succeeded")
			}
			if _, err := capability.NewDeclaration(nil, []capability.OptionalSpec{{Name: unknown, Fallback: "continue"}}); err == nil {
				t.Fatal("unknown optional capability succeeded")
			}
			if _, err := capability.NewGrantSet([]string{unknown}); err == nil {
				t.Fatal("unknown execution-context grant succeeded")
			}
			_, err := capability.NewTrustPolicy(map[capability.TrustLevel][]string{
				capability.TrustOfficial:          {unknown},
				capability.TrustSigned:            {},
				capability.TrustPrivateUnverified: {},
				capability.TrustDevelopment:       {},
			})
			if err == nil {
				t.Fatal("unknown trust grant succeeded")
			}
		})
	}
}

func TestUnknownCapabilityInAllThreeRawSetsStillFailsClosed(t *testing.T) {
	t.Parallel()
	const unknown = "host.unknown"
	_, declarationErr := capability.NewDeclaration([]string{unknown}, nil)
	_, trustErr := capability.NewTrustPolicy(map[capability.TrustLevel][]string{
		capability.TrustOfficial:          {unknown},
		capability.TrustSigned:            {},
		capability.TrustPrivateUnverified: {},
		capability.TrustDevelopment:       {},
	})
	_, executionErr := capability.NewGrantSet([]string{unknown})
	if declarationErr == nil || trustErr == nil || executionErr == nil {
		t.Fatalf("unknown three-layer input did not fail closed: declaration=%v trust=%v execution=%v", declarationErr, trustErr, executionErr)
	}
}

func TestResolutionRevalidatesDirectlyConstructedUnknownCapability(t *testing.T) {
	t.Parallel()
	declaration := capability.Declaration{Required: []capability.Name{"host.unknown"}}
	execution, err := capability.NewGrantSet([]string{"host.event", "host.log", "host.state"})
	if err != nil {
		t.Fatal(err)
	}
	resolution, err := capability.Resolve(
		declaration,
		capability.TrustOfficial,
		testPolicy(t, []string{"host.event", "host.log", "host.state"}),
		execution,
	)
	if err == nil || len(resolution.Effective.Names()) != 0 {
		t.Fatalf("unknown capability reached resolution: resolution=%#v error=%v", resolution, err)
	}
}

func TestCapabilityInputsRejectDuplicates(t *testing.T) {
	t.Parallel()
	if _, err := capability.NewDeclaration([]string{"host.state", "host.state"}, nil); err == nil {
		t.Fatal("duplicate required capability succeeded")
	}
	if _, err := capability.NewDeclaration(
		[]string{"host.state"},
		[]capability.OptionalSpec{{Name: "host.state", Fallback: "continue"}},
	); err == nil {
		t.Fatal("capability repeated across required and optional sets succeeded")
	}
	if _, err := capability.NewGrantSet([]string{"host.log", "host.log"}); err == nil {
		t.Fatal("duplicate capability grant succeeded")
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
