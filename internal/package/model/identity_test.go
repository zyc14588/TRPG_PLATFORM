// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package model_test

import (
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

func TestPackageIDIsStableIdentity(t *testing.T) {
	t.Parallel()
	id, err := model.ParsePackageID("example.publisher/stable-name")
	if err != nil {
		t.Fatal(err)
	}
	if id.String() != "example.publisher/stable-name" {
		t.Fatalf("PackageID = %q", id)
	}
	for _, invalid := range []string{"", "Example/name", "example", "example/", "example/../name", "example/name/extra"} {
		if _, err := model.ParsePackageID(invalid); err == nil {
			t.Errorf("ParsePackageID(%q) succeeded", invalid)
		}
	}
}

func TestPackageIDDoesNotContainMutableDisplayOrVersionMetadata(t *testing.T) {
	t.Parallel()
	id, err := model.ParsePackageID("example.publisher/stable-name")
	if err != nil {
		t.Fatal(err)
	}
	type releaseMetadata struct {
		PackageID   model.PackageID
		DisplayName string
		Version     string
	}
	first := releaseMetadata{PackageID: id, DisplayName: "First Name", Version: "1.0.0"}
	second := releaseMetadata{PackageID: id, DisplayName: "Renamed", Version: "2.0.0"}
	if first.PackageID != second.PackageID {
		t.Fatal("display/version metadata changed immutable package_id")
	}
}

func TestCanonicalSemanticVersion(t *testing.T) {
	t.Parallel()
	for _, valid := range []string{"0.0.0", "1.2.3", "1.2.3-alpha.1+build.7"} {
		if _, err := model.ParseVersion(valid); err != nil {
			t.Errorf("ParseVersion(%q): %v", valid, err)
		}
	}
	for _, invalid := range []string{"v1.2.3", "1.2", "01.2.3", "1.2.3-01", "1.2.3+"} {
		if _, err := model.ParseVersion(invalid); err == nil {
			t.Errorf("ParseVersion(%q) succeeded", invalid)
		}
	}
}

func TestCanonicalContentHash(t *testing.T) {
	t.Parallel()
	valid := "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
	if _, err := model.ParseContentHash(valid); err != nil {
		t.Fatal(err)
	}
	for _, invalid := range []string{"0123", "sha256:1234", "sha256:ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef0123456789"} {
		if _, err := model.ParseContentHash(invalid); err == nil {
			t.Errorf("ParseContentHash(%q) succeeded", invalid)
		}
	}
}

func TestRightsCanonicalizeAuthorsAndRequireDeclaration(t *testing.T) {
	t.Parallel()
	rights, err := model.NormalizeRights(model.Rights{
		Authors: []string{"Zed", "Ada"}, Source: " original ", Statement: " owned ",
	})
	if err != nil {
		t.Fatal(err)
	}
	if rights.Authors[0] != "Ada" || rights.Authors[1] != "Zed" || rights.Source != "original" {
		t.Fatalf("rights not canonicalized: %#v", rights)
	}
	if _, err := model.NormalizeRights(model.Rights{Authors: []string{"Ada"}, Source: "original"}); err == nil {
		t.Fatal("rights without license expression or statement succeeded")
	}
	if _, err := model.NormalizeRights(model.Rights{
		Authors: []string{"Ada"}, Source: "original", LicenseExpression: "LicenseRef-Test", Statement: " ",
	}); err == nil {
		t.Fatal("explicit whitespace-only rights statement succeeded")
	}
	if _, err := model.NormalizeRights(model.Rights{
		Authors: []string{"Ada"}, Source: "original", LicenseExpression: " ", Statement: "owned",
	}); err == nil {
		t.Fatal("explicit whitespace-only license expression succeeded")
	}
}
