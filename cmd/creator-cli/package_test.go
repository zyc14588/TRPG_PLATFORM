// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"bytes"
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
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

func writeCLIInput(t *testing.T, name, contents string) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), name)
	if err := os.WriteFile(path, []byte(contents), 0o600); err != nil {
		t.Fatal(err)
	}
	return path
}

func mutateCLIInput(t *testing.T, fixtureName string, replacements ...string) string {
	t.Helper()
	if len(replacements)%2 != 0 {
		t.Fatal("replacements must be old/new pairs")
	}
	data, err := os.ReadFile(packageFixture(fixtureName))
	if err != nil {
		t.Fatal(err)
	}
	contents := string(data)
	for index := 0; index < len(replacements); index += 2 {
		if !strings.Contains(contents, replacements[index]) {
			t.Fatalf("fixture %s does not contain %q", fixtureName, replacements[index])
		}
		contents = strings.Replace(contents, replacements[index], replacements[index+1], 1)
	}
	return writeCLIInput(t, fixtureName, contents)
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

func TestPackageValidateCanExplicitlyResolveCapabilities(t *testing.T) {
	t.Parallel()
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	exitCode := run(context.Background(), []string{
		"package", "validate",
		"--manifest", packageFixture("package.toml"),
		"--lock", packageFixture("package.lock.json"),
		"--resolve-capabilities",
		"--trust-grant", "host.event",
		"--trust-grant", "host.state",
		"--context-grant", "host.state",
		"--context-grant", "host.event",
	}, &stdout, &stderr)
	if exitCode != 0 {
		t.Fatalf("exit = %d, stderr = %s", exitCode, stderr.String())
	}
	if !strings.Contains(stdout.String(), `"valid":true`) {
		t.Fatalf("stdout = %s", stdout.String())
	}
}

func TestPackageCommandFailsClosed(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name string
		args func(*testing.T) []string
	}{
		{
			name: "missing lock",
			args: func(*testing.T) []string {
				return []string{"package", "validate", "--manifest", packageFixture("package.toml")}
			},
		},
		{
			name: "malformed manifest",
			args: func(t *testing.T) []string {
				return []string{"package", "validate", "--manifest", writeCLIInput(t, "malformed.toml", "schema_version = ["), "--lock", packageFixture("package.lock.json")}
			},
		},
		{
			name: "unsupported package kind",
			args: func(t *testing.T) []string {
				manifest := mutateCLIInput(t, "package.toml", `package_kind = "game-system"`, `package_kind = "bundle"`)
				return []string{"package", "validate", "--manifest", manifest, "--lock", packageFixture("package.lock.json")}
			},
		},
		{
			name: "unknown capability",
			args: func(t *testing.T) []string {
				manifest := mutateCLIInput(t, "package.toml", `"host.event"`, `"host.unknown"`)
				return []string{"package", "validate", "--manifest", manifest, "--lock", packageFixture("package.lock.json")}
			},
		},
		{
			name: "missing required capability",
			args: func(*testing.T) []string {
				return []string{"package", "validate", "--manifest", packageFixture("package.toml"), "--lock", packageFixture("package.lock.json"), "--resolve-capabilities"}
			},
		},
		{
			name: "unknown trust grant",
			args: func(*testing.T) []string {
				return []string{"package", "validate", "--manifest", packageFixture("package.toml"), "--lock", packageFixture("package.lock.json"), "--resolve-capabilities", "--trust-grant", "host.unknown"}
			},
		},
		{
			name: "unknown execution-context grant",
			args: func(*testing.T) []string {
				return []string{"package", "validate", "--manifest", packageFixture("package.toml"), "--lock", packageFixture("package.lock.json"), "--resolve-capabilities", "--context-grant", "host.unknown"}
			},
		},
		{
			name: "extra undeclared lock Feature",
			args: func(t *testing.T) []string {
				lock := mutateCLIInput(t, "package.lock.json", `"features": ["standard-deck"]`, `"features": ["extra-feature", "standard-deck"]`)
				return []string{"package", "validate", "--manifest", packageFixture("package.toml"), "--lock", lock}
			},
		},
		{
			name: "missing lock Feature",
			args: func(t *testing.T) []string {
				lock := mutateCLIInput(t, "package.lock.json", `"features": ["standard-deck"]`, `"features": []`)
				return []string{"package", "validate", "--manifest", packageFixture("package.toml"), "--lock", lock}
			},
		},
		{
			name: "invalid dependency graph",
			args: func(t *testing.T) []string {
				lock := mutateCLIInput(t, "package.lock.json", `"dependencies": ["example.shared/card-library"]`, `"dependencies": []`)
				return []string{"package", "validate", "--manifest", packageFixture("package.toml"), "--lock", lock}
			},
		},
		{
			name: "direct cycle",
			args: func(t *testing.T) []string {
				lock := mutateCLIInput(t, "package.lock.json", `"dependencies": ["example.shared/card-library"]`, `"dependencies": ["example.rules/hidden-cards"]`)
				return []string{"package", "validate", "--manifest", packageFixture("package.toml"), "--lock", lock}
			},
		},
		{
			name: "indirect cycle",
			args: func(t *testing.T) []string {
				lock := mutateCLIInput(t, "package.lock.json", `"dependencies": []`, `"dependencies": ["example.rules/hidden-cards"]`)
				return []string{"package", "validate", "--manifest", packageFixture("package.toml"), "--lock", lock}
			},
		},
		{
			name: "content hash mismatch",
			args: func(*testing.T) []string {
				return []string{"package", "build", "--manifest", packageFixture("package.toml"), "--lock", packageFixture("package.lock.json"), "--content-hash", "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"}
			},
		},
		{
			name: "invalid package identity",
			args: func(t *testing.T) []string {
				manifest := mutateCLIInput(t, "package.toml", `package_id = "example.rules/hidden-cards"`, `package_id = "Example Rules/hidden-cards"`)
				return []string{"package", "validate", "--manifest", manifest, "--lock", packageFixture("package.lock.json")}
			},
		},
		{
			name: "invalid semantic version",
			args: func(t *testing.T) []string {
				manifest := mutateCLIInput(t, "package.toml", `version = "1.2.3"`, `version = "1.2"`)
				return []string{"package", "validate", "--manifest", manifest, "--lock", packageFixture("package.lock.json")}
			},
		},
		{
			name: "unknown subcommand",
			args: func(*testing.T) []string {
				return []string{"package", "install"}
			},
		},
	}
	for _, test := range tests {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			var stdout bytes.Buffer
			var stderr bytes.Buffer
			if got := run(context.Background(), test.args(t), &stdout, &stderr); got == 0 {
				t.Fatalf("illegal input exited zero; stdout = %s", stdout.String())
			}
			if strings.Contains(stdout.String(), `"valid":true`) {
				t.Fatalf("illegal input reported valid:true: %s", stdout.String())
			}
			if stderr.Len() == 0 {
				t.Fatal("illegal input failed without a diagnostic")
			}
		})
	}
}
