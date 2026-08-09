// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package dependency_test

import (
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

func testRequirementLock(t *testing.T) dependency.ExactLock {
	t.Helper()
	root := mustNode(t, "example/root", "1.0.0", hashA, nil, []string{"example/library"})
	library := mustNode(t, "example/library", "2.0.0", hashB, []string{"standard"}, nil)
	lock, err := dependency.BuildExactLock("example/root", []dependency.LockedPackage{root, library})
	if err != nil {
		t.Fatal(err)
	}
	return lock
}

func TestValidateRequirementsMatchesExactVersionAndFeatures(t *testing.T) {
	t.Parallel()
	requirement, err := dependency.NewRequirement("example/library", "2.0.0", false, []string{"standard"})
	if err != nil {
		t.Fatal(err)
	}
	root, err := model.ParsePackageID("example/root")
	if err != nil {
		t.Fatal(err)
	}
	if err := dependency.ValidateRequirements(testRequirementLock(t), root, []dependency.Requirement{requirement}); err != nil {
		t.Fatal(err)
	}
}

func TestValidateRequirementsRejectsVersionFeatureAndDeclarationMismatch(t *testing.T) {
	t.Parallel()
	root, err := model.ParsePackageID("example/root")
	if err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name        string
		requirement dependency.Requirement
		contains    string
	}{
		{name: "version", requirement: mustRequirement(t, "example/library", "2.1.0", false, nil), contains: "locks version"},
		{name: "feature", requirement: mustRequirement(t, "example/library", "2.0.0", false, []string{"extended"}), contains: "does not enable"},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			err := dependency.ValidateRequirements(testRequirementLock(t), root, []dependency.Requirement{test.requirement})
			if err == nil || !strings.Contains(err.Error(), test.contains) {
				t.Fatalf("ValidateRequirements error = %v", err)
			}
		})
	}
	if err := dependency.ValidateRequirements(testRequirementLock(t), root, nil); err == nil || !strings.Contains(err.Error(), "undeclared") {
		t.Fatalf("undeclared edge error = %v", err)
	}
}

func TestOptionalRequirementMayBeAbsentFromExactGraph(t *testing.T) {
	t.Parallel()
	rootNode := mustNode(t, "example/root", "1.0.0", hashA, nil, nil)
	lock, err := dependency.BuildExactLock("example/root", []dependency.LockedPackage{rootNode})
	if err != nil {
		t.Fatal(err)
	}
	root, err := model.ParsePackageID("example/root")
	if err != nil {
		t.Fatal(err)
	}
	optional := mustRequirement(t, "example/optional", "1.0.0", true, nil)
	if err := dependency.ValidateRequirements(lock, root, []dependency.Requirement{optional}); err != nil {
		t.Fatal(err)
	}
}

func mustRequirement(t *testing.T, id, version string, optional bool, features []string) dependency.Requirement {
	t.Helper()
	requirement, err := dependency.NewRequirement(id, version, optional, features)
	if err != nil {
		t.Fatal(err)
	}
	return requirement
}
