//go:build integration && linux

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package package_install_test

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
)

func TestCreatorBinaryEditedArtifactInstallsAndReloads(t *testing.T) {
	e := setup(t)
	repo, err := filepath.Abs("../../..")
	if err != nil {
		t.Fatal(err)
	}
	git := func(args ...string) string {
		cmd := exec.Command("git", args...)
		cmd.Dir = repo
		out, err := cmd.Output()
		if err != nil {
			t.Fatal(err)
		}
		return strings.TrimSpace(string(out))
	}
	commit, tree := git("rev-parse", "HEAD"), git("rev-parse", "HEAD^{tree}")
	if status := git("status", "--porcelain"); status != "" {
		t.Fatal("Creator acceptance requires a fixed clean candidate", status)
	}
	frontend := exec.Command("pnpm", "--dir", "apps/creator-studio/frontend", "build")
	frontend.Dir = repo
	if out, err := frontend.CombinedOutput(); err != nil {
		t.Fatalf("frontend build: %v %s", err, out)
	}
	binary := filepath.Join(t.TempDir(), "creator-studio")
	recipe := "b003-native-creator-production-webkit2_41-trimpath"
	args := []string{"build", "-trimpath", "-buildvcs=false", "-tags", "production,webkit2_41", "-ldflags", fmt.Sprintf("-X main.platformCommit=%s -X main.platformTree=%s -X main.binaryVersion=0.1.0-m1 -X main.buildCommand=%s", commit, tree, recipe), "-o", binary, "./apps/creator-studio"}
	build := exec.Command("go", args...)
	build.Dir = repo
	if out, err := build.CombinedOutput(); err != nil {
		t.Fatalf("native Creator build: %v %s", err, out)
	}
	binaryBytes, err := os.ReadFile(binary)
	if err != nil {
		t.Fatal(err)
	}
	binaryHash := object.Hash(binaryBytes)
	type identity struct {
		Commit  string `json:"platform_commit"`
		Tree    string `json:"platform_tree"`
		Hash    string `json:"binary_sha256"`
		Version string `json:"binary_version"`
		Command string `json:"build_command"`
	}
	run := func(args ...string) []byte {
		cmd := exec.Command(binary, args...)
		out, err := cmd.Output()
		if err != nil {
			var stderr string
			if failure, ok := err.(*exec.ExitError); ok {
				stderr = string(failure.Stderr)
			}
			t.Fatalf("Creator %q: %v %s", args, err, stderr)
		}
		return out
	}
	var actual identity
	if err = json.Unmarshal(run("identity"), &actual); err != nil {
		t.Fatal(err)
	}
	if actual.Commit != commit || actual.Tree != tree || "sha256:"+actual.Hash != binaryHash || actual.Version != "0.1.0-m1" || actual.Command != recipe {
		t.Fatal("Creator identity did not match tested binary", actual)
	}
	t.Logf("Creator source=%s tree=%s binary=%s argv=%q", commit, tree, binaryHash, append([]string{"go"}, args...))
	files := fixtures.Files("third.publisher/editable", "assets", "")
	schema := []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","required":["count"],"properties":{"count":{"type":"integer"}},"additionalProperties":false}`)
	files[archive.ManifestPath] = []byte(strings.Replace(string(files[archive.ManifestPath]), "schema_version = 1", "schema_version = 2", 1) + fmt.Sprintf("\n[[extensions]]\nnamespace = \"third.party.probe\"\nrequired = true\ncontract_version = 1\nschema_path = \"extensions/third.party.probe/value.schema.json\"\nschema_sha256 = %q\npayload_path = \"extensions/third.party.probe/value.json\"\nhost_api_major = 1\nhost_api_min_minor = 0\nhost_api_max_minor = 0\n", object.Hash(schema)))
	files["extensions/third.party.probe/value.schema.json"] = schema
	files["extensions/third.party.probe/value.json"] = []byte(`{"count":1}`)
	pkg, err := fixtures.Build(files)
	if err != nil {
		t.Fatal(err)
	}
	sourceBytes, err := fixtures.Archive(pkg)
	if err != nil {
		t.Fatal(err)
	}
	dir := t.TempDir()
	source := filepath.Join(dir, "source.trpgpkg")
	jsonFile := filepath.Join(dir, "edit.json")
	if err = os.WriteFile(source, sourceBytes, 0600); err != nil {
		t.Fatal(err)
	}
	if err = os.WriteFile(jsonFile, []byte(`{"count":7}`), 0600); err != nil {
		t.Fatal(err)
	}
	var outputs [][]byte
	for index := range 2 {
		output := filepath.Join(dir, fmt.Sprintf("edited-%d.trpgpkg", index))
		result := run("extension", "edit", "--archive", source, "--namespace", "third.party.probe", "--json-file", jsonFile, "--output", output, "--conflict-token", object.Hash(sourceBytes))
		var reply struct {
			Identity identity                        `json:"identity"`
			Error    any                             `json:"error"`
			Phases   []struct{ Name, Status string } `json:"phases"`
		}
		if err = json.Unmarshal(result, &reply); err != nil {
			t.Fatal(err)
		}
		if reply.Error != nil || reply.Identity != actual {
			t.Fatal("Creator edit/identity failure", string(result))
		}
		for _, phase := range reply.Phases {
			if phase.Status != "ok" {
				t.Fatal("Creator phase not successful", phase)
			}
		}
		edited, err := os.ReadFile(output)
		if err != nil {
			t.Fatal(err)
		}
		outputs = append(outputs, edited)
		_ = run("extension", "inspect", "--archive", output)
	}
	if !bytes.Equal(outputs[0], outputs[1]) {
		t.Fatal("Creator second output changed")
	}
	edited, err := archive.ImportBytes(outputs[0], extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	entry, ok := edited.Entry("extensions/third.party.probe/value.json")
	if !ok || string(entry.Bytes()) != `{"count":7}` {
		t.Fatal("real JSON edit did not happen")
	}
	i := e.installer(t, edited, "creator", nil, install.RuntimeConfig{}, nil, nil)
	r := request(t, edited, "creator", "a", alice)
	r.Root.Archive = bytes.NewReader(outputs[0])
	installed, err := i.Install(context.Background(), r)
	if err != nil {
		t.Fatal(err)
	}
	loaded, err := e.reader(t).Load(context.Background(), alice, "a", installed.Root)
	if err != nil {
		t.Fatal(err)
	}
	roundtrip, err := fixtures.Archive(loaded)
	if err != nil || !bytes.Equal(roundtrip, outputs[0]) {
		t.Fatal("Creator -> installer -> Host roundtrip lost bytes", err)
	}
	t.Log("Creator repeated output and installed archive", object.Hash(outputs[0]))
	e.assertCounts(t, 1, 1, 1)
}
