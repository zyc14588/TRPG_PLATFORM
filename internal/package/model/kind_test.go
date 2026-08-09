// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package model_test

import (
	"reflect"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

func TestPackageKindsDistinguishRuntimePackagesFromBundle(t *testing.T) {
	t.Parallel()
	want := []model.PackageKind{
		model.PackageKindGameSystem,
		model.PackageKindContent,
		model.PackageKindAssets,
		model.PackageKindUIExtension,
		model.PackageKindLibrary,
	}
	if got := model.PackageKinds(); !reflect.DeepEqual(got, want) {
		t.Fatalf("PackageKinds() = %#v, want %#v", got, want)
	}
	for _, invalid := range []string{
		"",
		"bundle",
		"Game-System",
		"CONTENT",
		" content",
		"content ",
		"ui_extension",
		"unknown",
	} {
		if _, err := model.ParsePackageKind(invalid); err == nil {
			t.Errorf("ParsePackageKind(%q) succeeded", invalid)
		}
	}
	if model.PackageKindLibrary.CanStartSession() {
		t.Fatal("library package can start a Session")
	}
	if !model.PackageKindGameSystem.CanStartSession() {
		t.Fatal("game-system package cannot start a Session")
	}
	for _, kind := range []model.PackageKind{model.PackageKindContent, model.PackageKindAssets, model.PackageKindUIExtension} {
		if kind.CanStartSession() {
			t.Fatalf("%s package can start a Session", kind)
		}
	}
}

func TestArtifactTypes(t *testing.T) {
	t.Parallel()
	for _, value := range []string{"package", "bundle"} {
		if _, err := model.ParseArtifactType(value); err != nil {
			t.Fatalf("ParseArtifactType(%q): %v", value, err)
		}
	}
	if _, err := model.ParseArtifactType("library"); err == nil {
		t.Fatal("runtime kind accepted as artifact type")
	}
}
