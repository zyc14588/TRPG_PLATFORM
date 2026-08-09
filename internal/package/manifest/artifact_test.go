// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package manifest_test

import (
	"bytes"
	"encoding/json"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/manifest"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

func artifactInputs(t *testing.T) (manifest.Package, model.ContentHash, dependency.ExactLock) {
	t.Helper()
	document, err := manifest.Parse(fixture(t, "package.toml"))
	if err != nil {
		t.Fatal(err)
	}
	lock, err := dependency.ParseExactLock(fixture(t, "package.lock.json"))
	if err != nil {
		t.Fatal(err)
	}
	hash, err := model.ParseContentHash("sha256:1111111111111111111111111111111111111111111111111111111111111111")
	if err != nil {
		t.Fatal(err)
	}
	return *document.Package, hash, lock
}

func TestArtifactIdentityIsDeterministicAndComplete(t *testing.T) {
	t.Parallel()
	pkg, hash, lock := artifactInputs(t)
	first, err := manifest.BuildArtifactIdentity(pkg, hash, lock)
	if err != nil {
		t.Fatal(err)
	}
	second, err := manifest.BuildArtifactIdentity(pkg, hash, lock)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(first.CanonicalJSON(), second.CanonicalJSON()) || first.Digest() != second.Digest() {
		t.Fatal("identical inputs produced different artifact identities")
	}
	var document map[string]any
	if err := json.Unmarshal(first.CanonicalJSON(), &document); err != nil {
		t.Fatal(err)
	}
	for _, field := range []string{"package_id", "version", "content_hash", "build_provenance", "rights", "dependency_lock"} {
		if _, exists := document[field]; !exists {
			t.Errorf("artifact identity omits %q", field)
		}
	}
}

func TestArtifactIdentityChangesWithProvenanceRightsAndLock(t *testing.T) {
	t.Parallel()
	pkg, hash, lock := artifactInputs(t)
	base, err := manifest.BuildArtifactIdentity(pkg, hash, lock)
	if err != nil {
		t.Fatal(err)
	}
	changedBuild := pkg
	changedBuild.Build.Builder = "creator-cli/0.2"
	buildIdentity, err := manifest.BuildArtifactIdentity(changedBuild, hash, lock)
	if err != nil {
		t.Fatal(err)
	}
	if base.Digest() == buildIdentity.Digest() {
		t.Fatal("build provenance did not affect artifact identity")
	}
	changedRights := pkg
	changedRights.Rights.Statement = "additional rights declaration"
	rightsIdentity, err := manifest.BuildArtifactIdentity(changedRights, hash, lock)
	if err != nil {
		t.Fatal(err)
	}
	if base.Digest() == rightsIdentity.Digest() {
		t.Fatal("rights metadata did not affect artifact identity")
	}
	root, _ := lock.Package(lock.Root())
	libraryID, err := model.ParsePackageID("example.shared/card-library")
	if err != nil {
		t.Fatal(err)
	}
	library, _ := lock.Package(libraryID)
	library.Features = []string{"extended-deck", "standard-deck"}
	pkg.Dependencies[0].Features = []string{"extended-deck", "standard-deck"}
	changedLock, err := dependency.BuildExactLock(lock.Root().String(), []dependency.LockedPackage{root, library})
	if err != nil {
		t.Fatal(err)
	}
	lockIdentity, err := manifest.BuildArtifactIdentity(pkg, hash, changedLock)
	if err != nil {
		t.Fatal(err)
	}
	if base.Digest() == lockIdentity.Digest() {
		t.Fatal("exact dependency lock did not affect artifact identity")
	}
}

func TestArtifactIdentityRejectsLockMismatch(t *testing.T) {
	t.Parallel()
	pkg, _, lock := artifactInputs(t)
	wrongHash, err := model.ParseContentHash("sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := manifest.BuildArtifactIdentity(pkg, wrongHash, lock); err == nil {
		t.Fatal("artifact identity accepted content hash that differs from exact lock")
	}
}
