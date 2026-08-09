// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"bytes"
	"context"
	"encoding/json"
	"path/filepath"
	"testing"
)

const (
	fixtureContentHash    = "sha256:1111111111111111111111111111111111111111111111111111111111111111"
	fixtureLockDigest     = "sha256:4f39b7996f57c7a9e86ce1cbb040b121195e1ff9c52e2ef3474e78e790ea6085"
	fixtureArtifactDigest = "sha256:3bb935edf2b0865af5c1a0bf9b58440580fe7353d3075cd0e7ac98c6dcb026be"
)

func packageFixture(name string) string {
	return filepath.Join("..", "..", "internal", "package", "testdata", name)
}

func TestPackageValidateProducesMachineReadableResult(t *testing.T) {
	t.Parallel()
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	exitCode := run(context.Background(), []string{
		"package", "validate",
		"--manifest", packageFixture("package.toml"),
		"--lock", packageFixture("package.lock.json"),
	}, &stdout, &stderr)
	if exitCode != 0 {
		t.Fatalf("exit = %d, stderr = %s", exitCode, stderr.String())
	}
	var result map[string]any
	if err := json.Unmarshal(stdout.Bytes(), &result); err != nil {
		t.Fatalf("result is not JSON: %v\n%s", err, stdout.String())
	}
	if result["valid"] != true || result["artifact_type"] != "package" || result["artifact_digest"] != fixtureArtifactDigest || result["lock_digest"] != fixtureLockDigest {
		t.Fatalf("result = %#v", result)
	}
}

func TestPackageBuildIsRepeatable(t *testing.T) {
	t.Parallel()
	args := []string{
		"package", "build",
		"--manifest", packageFixture("package.toml"),
		"--lock", packageFixture("package.lock.json"),
		"--content-hash", fixtureContentHash,
	}
	var first bytes.Buffer
	var firstErr bytes.Buffer
	if exitCode := run(context.Background(), args, &first, &firstErr); exitCode != 0 {
		t.Fatalf("first exit = %d, stderr = %s", exitCode, firstErr.String())
	}
	var second bytes.Buffer
	var secondErr bytes.Buffer
	if exitCode := run(context.Background(), args, &second, &secondErr); exitCode != 0 {
		t.Fatalf("second exit = %d, stderr = %s", exitCode, secondErr.String())
	}
	if !bytes.Equal(first.Bytes(), second.Bytes()) {
		t.Fatalf("build outputs differ:\n%s\n%s", first.String(), second.String())
	}
	var result struct {
		ArtifactIdentity json.RawMessage `json:"artifact_identity"`
		ArtifactDigest   string          `json:"artifact_digest"`
	}
	if err := json.Unmarshal(first.Bytes(), &result); err != nil {
		t.Fatal(err)
	}
	if len(result.ArtifactIdentity) == 0 || result.ArtifactDigest != fixtureArtifactDigest {
		t.Fatalf("build result = %s", first.String())
	}
}

func TestBundleValidationNeedsNoRuntimeLock(t *testing.T) {
	t.Parallel()
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	exitCode := run(context.Background(), []string{
		"package", "validate", "--manifest", packageFixture("bundle.toml"),
	}, &stdout, &stderr)
	if exitCode != 0 {
		t.Fatalf("exit = %d, stderr = %s", exitCode, stderr.String())
	}
	var result map[string]any
	if err := json.Unmarshal(stdout.Bytes(), &result); err != nil {
		t.Fatal(err)
	}
	if result["artifact_type"] != "bundle" || result["lock_digest"] != nil {
		t.Fatalf("result = %#v", result)
	}
}

func TestPackageCommandFailsClosed(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name string
		args []string
		want int
	}{
		{name: "missing lock", args: []string{"package", "validate", "--manifest", packageFixture("package.toml")}, want: 1},
		{name: "wrong content hash", args: []string{"package", "build", "--manifest", packageFixture("package.toml"), "--lock", packageFixture("package.lock.json"), "--content-hash", "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"}, want: 1},
		{name: "unknown subcommand", args: []string{"package", "install"}, want: 2},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			var stdout bytes.Buffer
			var stderr bytes.Buffer
			if got := run(context.Background(), test.args, &stdout, &stderr); got != test.want {
				t.Fatalf("exit = %d, want %d; stderr = %s", got, test.want, stderr.String())
			}
		})
	}
}
