// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package evidence

import (
	"context"
	"fmt"
	"os/exec"
)

// BuildCheckout creates an owned local source copy of the exact candidate.
// This toolchain detects a .git directory for VCS stamping, but does not
// detect the .git file in a linked worktree. No network, branch, source-repo
// config or trust-store change is involved. The caller removes its temp root.
func BuildCheckout(ctx context.Context, root, sha, destination string) error {
	if _, err := Candidate(root, sha); err != nil {
		return err
	}
	cmd := exec.CommandContext(ctx, "git", "clone", "--quiet", "--no-hardlinks", "--no-checkout", root, destination)
	if raw, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("local exact source copy failed: %w (%d output bytes)", err, len(raw))
	}
	cmd = exec.CommandContext(ctx, "git", "-C", destination, "checkout", "--quiet", "--detach", sha)
	if raw, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("local exact source checkout failed: %w (%d output bytes)", err, len(raw))
	}
	if _, err := Candidate(destination, sha); err != nil {
		return err
	}
	return nil
}
