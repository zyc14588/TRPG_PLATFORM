// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package evidence is confined to M1 test artifacts. It defines no public
// platform contract and never substitutes a prior receipt for a fresh run.
package evidence

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
)

func Candidate(root, requested string) (string, error) {
	cmd := exec.Command("git", "--no-optional-locks", "-C", root, "rev-parse", "HEAD")
	raw, err := cmd.Output()
	if err != nil {
		return "", err
	}
	sha := strings.TrimSpace(string(raw))
	if !regexp.MustCompile(`^[0-9a-f]{40}$`).MatchString(sha) || requested != "HEAD" && requested != sha {
		return "", fmt.Errorf("candidate must be the exact current HEAD")
	}
	cmd = exec.Command("git", "--no-optional-locks", "-C", root, "status", "--porcelain=v1")
	raw, err = cmd.Output()
	if err != nil || len(raw) != 0 {
		return "", fmt.Errorf("certification requires a clean source tree")
	}
	return sha, nil
}

// Write atomically replaces a complete machine artifact, including failures.
// A failed fsync/rename/directory sync is itself an acceptance failure.
func Write(path string, value any) error {
	raw, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return err
	}
	dir := filepath.Dir(path)
	if err = os.MkdirAll(dir, 0700); err != nil {
		return err
	}
	f, err := os.CreateTemp(dir, ".m1-evidence-")
	if err != nil {
		return err
	}
	name := f.Name()
	defer os.Remove(name)
	if err = f.Chmod(0600); err == nil {
		_, err = f.Write(append(raw, '\n'))
	}
	if err == nil {
		err = f.Sync()
	}
	closeErr := f.Close()
	if err == nil {
		err = closeErr
	}
	if err == nil {
		err = os.Rename(name, path)
	}
	if err != nil {
		return err
	}
	d, err := os.Open(dir)
	if err != nil {
		return err
	}
	err = d.Sync()
	closeErr = d.Close()
	if err != nil {
		return err
	}
	return closeErr
}
