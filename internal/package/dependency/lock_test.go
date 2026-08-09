// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package dependency_test

import (
	"bytes"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
)

const (
	hashA = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	hashB = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
	hashC = "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
)

func mustNode(t *testing.T, id, version, hash string, features, dependencies []string) dependency.LockedPackage {
	t.Helper()
	node, err := dependency.NewLockedPackage(id, version, hash, features, dependencies)
	if err != nil {
		t.Fatal(err)
	}
	return node
}

func TestExactLockIsDeterministicAndTransitive(t *testing.T) {
	t.Parallel()
	root := mustNode(t, "example/root", "1.0.0", hashA, []string{"zeta", "alpha"}, []string{"example/two", "example/one"})
	one := mustNode(t, "example/one", "2.0.0", hashB, nil, []string{"example/two"})
	two := mustNode(t, "example/two", "3.0.0", hashC, []string{"shared"}, nil)
	first, err := dependency.BuildExactLock("example/root", []dependency.LockedPackage{root, one, two})
	if err != nil {
		t.Fatal(err)
	}
	second, err := dependency.BuildExactLock("example/root", []dependency.LockedPackage{two, root, one})
	if err != nil {
		t.Fatal(err)
	}
	firstJSON, err := first.CanonicalJSON()
	if err != nil {
		t.Fatal(err)
	}
	secondJSON, err := second.CanonicalJSON()
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(firstJSON, secondJSON) {
		t.Fatalf("canonical locks differ:\n%s\n%s", firstJSON, secondJSON)
	}
	if bytes.Contains(firstJSON, []byte(":null")) {
		t.Fatalf("canonical lock encodes an empty collection as null: %s", firstJSON)
	}
	parsed, err := dependency.ParseExactLock(firstJSON)
	if err != nil {
		t.Fatal(err)
	}
	if got := len(parsed.Packages()); got != 3 {
		t.Fatalf("transitive package count = %d", got)
	}
}

func TestExactLockRejectsSingleVersionViolation(t *testing.T) {
	t.Parallel()
	first := mustNode(t, "example/root", "1.0.0", hashA, nil, nil)
	second := mustNode(t, "example/root", "2.0.0", hashB, nil, nil)
	_, err := dependency.BuildExactLock("example/root", []dependency.LockedPackage{first, second})
	if err == nil || !strings.Contains(err.Error(), "single-version violation") {
		t.Fatalf("BuildExactLock error = %v", err)
	}
}

func TestExactLockRejectsDirectAndIndirectCycles(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name  string
		nodes []dependency.LockedPackage
	}{
		{
			name: "direct",
			nodes: []dependency.LockedPackage{
				mustNode(t, "example/root", "1.0.0", hashA, nil, []string{"example/root"}),
			},
		},
		{
			name: "indirect",
			nodes: []dependency.LockedPackage{
				mustNode(t, "example/root", "1.0.0", hashA, nil, []string{"example/one"}),
				mustNode(t, "example/one", "1.0.0", hashB, nil, []string{"example/two"}),
				mustNode(t, "example/two", "1.0.0", hashC, nil, []string{"example/root"}),
			},
		},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			_, err := dependency.BuildExactLock("example/root", test.nodes)
			if err == nil || !strings.Contains(err.Error(), "dependency cycle") {
				t.Fatalf("BuildExactLock error = %v", err)
			}
		})
	}
}

func TestExactLockRejectsUnreachableAndUnlockedNodes(t *testing.T) {
	t.Parallel()
	root := mustNode(t, "example/root", "1.0.0", hashA, nil, nil)
	unreachable := mustNode(t, "example/extra", "1.0.0", hashB, nil, nil)
	if _, err := dependency.BuildExactLock("example/root", []dependency.LockedPackage{root, unreachable}); err == nil || !strings.Contains(err.Error(), "unreachable") {
		t.Fatalf("unreachable error = %v", err)
	}
	root = mustNode(t, "example/root", "1.0.0", hashA, nil, []string{"example/missing"})
	if _, err := dependency.BuildExactLock("example/root", []dependency.LockedPackage{root}); err == nil || !strings.Contains(err.Error(), "unlocked") {
		t.Fatalf("unlocked error = %v", err)
	}
}

func TestEnabledFeaturesChangeLockDigest(t *testing.T) {
	t.Parallel()
	build := func(feature string) model.ContentHash {
		root := mustNode(t, "example/root", "1.0.0", hashA, nil, []string{"example/library"})
		library := mustNode(t, "example/library", "2.0.0", hashB, []string{feature}, nil)
		lock, err := dependency.BuildExactLock("example/root", []dependency.LockedPackage{root, library})
		if err != nil {
			t.Fatal(err)
		}
		digest, err := lock.Digest()
		if err != nil {
			t.Fatal(err)
		}
		return digest
	}
	if build("standard") == build("extended") {
		t.Fatal("different enabled Features produced the same lock digest")
	}
}

func TestExactLockJSONRejectsUnknownFields(t *testing.T) {
	t.Parallel()
	data := []byte(`{"schema_version":1,"root":"example/root","packages":[],"fallback":true}`)
	if _, err := dependency.ParseExactLock(data); err == nil {
		t.Fatal("unknown lock field succeeded")
	}
}

func TestExactLockJSONRejectsUnsupportedSchemaVersion(t *testing.T) {
	t.Parallel()
	data := []byte(`{"schema_version":2,"root":"example/root","packages":[]}`)
	if _, err := dependency.ParseExactLock(data); err == nil || !strings.Contains(err.Error(), "unsupported exact lock schema_version") {
		t.Fatalf("unsupported schema error = %v", err)
	}
}
