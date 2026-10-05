// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package install performs isolated, fail-closed initial package installation.
package install

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
)

type Options struct {
	StagingRoot string
	Policy      *Policy
	Access      *store.Access
	Objects     *object.Directory
	Repository  store.Repository
	Support     extension.Support
	Runtime     RuntimeConfig
	Observe     func(string) error
	Execution   func(Execution) error
}
type Installer struct{ options Options }
type Request struct {
	Credential    store.Credential
	Workspace, ID string
	Root          Input
	Dependencies  []Input
}

func New(o Options) (*Installer, error) {
	if !filepath.IsAbs(o.StagingRoot) || o.Policy == nil || o.Access == nil || o.Objects == nil || o.Repository == nil || o.Observe == nil || o.Execution == nil {
		return nil, ErrPolicy
	}
	info, err := os.Lstat(o.StagingRoot)
	if err != nil {
		return nil, err
	}
	if !info.IsDir() || info.Mode()&os.ModeSymlink != 0 || info.Mode().Perm()&0077 != 0 {
		return nil, ErrPolicy
	}
	return &Installer{options: o}, nil
}

func (i *Installer) Install(ctx context.Context, r Request) (store.Result, error) {
	var zero store.Result
	o := i.options
	membership, err := o.Access.Authorize(r.Credential, r.Workspace, true)
	if err != nil {
		return zero, err
	}
	if !store.ValidID(r.ID) {
		return zero, store.ErrConflict
	}
	step := func(name string) error {
		if err := ctx.Err(); err != nil {
			return err
		}
		return o.Observe(name)
	}
	if err = step("authorized"); err != nil {
		return zero, err
	}
	inputs := append([]Input{r.Root}, r.Dependencies...)
	items, cleanup, err := stage(ctx, o.StagingRoot, inputs, o.Support)
	defer cleanup()
	if err != nil {
		return zero, fmt.Errorf("staging: %w", err)
	}
	if err = step("staged"); err != nil {
		return zero, err
	}
	if err = validateGraph(items); err != nil {
		return zero, err
	}
	dependencies := items[1:]
	sort.Slice(dependencies, func(a, b int) bool {
		return dependencies[a].pkg.ExactLock().Root() < dependencies[b].pkg.ExactLock().Root()
	})
	for index := range items {
		if err = validateContent(items[index].pkg); err != nil {
			return zero, err
		}
		a, _, e := o.Policy.validate(items[index].pkg, items[index].evidence)
		if e != nil {
			return zero, e
		}
		items[index].approval = a
	}
	if err = step("policy-validated"); err != nil {
		return zero, err
	}
	// Bind idempotency to the exact source archives, graph and trusted policy.
	fingerprints := []string{o.Policy.Digest(), r.Workspace, membership.Principal}
	for _, item := range items {
		source, _ := item.pkg.SourceArchiveHash()
		fingerprints = append(fingerprints, string(source), string(item.pkg.ArtifactIdentity().Digest()))
	}
	encoded, _ := json.Marshal(fingerprints)
	fingerprint := object.Hash(encoded)
	// Recovery must return the exact graph requested, even if every artifact in
	// an inconsistent stored result is independently valid and readable.
	checkResult := func(result store.Result) (store.Result, error) {
		if result.Workspace != r.Workspace || result.RequestID != r.ID || result.Fingerprint != fingerprint || result.Root != string(items[0].pkg.ArtifactIdentity().Digest()) || len(result.Artifacts) != len(items) {
			return zero, store.ErrConflict
		}
		expected := map[string]bool{}
		for _, item := range items {
			expected[string(item.pkg.ArtifactIdentity().Digest())] = true
		}
		for _, id := range result.Artifacts {
			if !expected[id] {
				return zero, store.ErrConflict
			}
			delete(expected, id)
		}
		return result, nil
	}
	if result, e := o.Repository.Resolve(ctx, r.Workspace, membership.Principal, r.ID, fingerprint); e == nil {
		return checkResult(result)
	} else if !errors.Is(e, store.ErrNotFound) {
		return zero, e
	}
	validation, err := validateRuntime(ctx, o.Runtime, items, o.Execution)
	if err != nil {
		return zero, fmt.Errorf("production-profile: %w", err)
	}
	if err = step("runtime-validated"); err != nil {
		return zero, err
	}
	ids := make([]string, len(items))
	for index, item := range items {
		doc, _ := item.pkg.Manifest()
		ids[index] = string(doc.Package.PackageID)
	}
	if err = o.Repository.Preflight(ctx, r.Workspace, ids); err != nil {
		return zero, err
	}
	if err = step("fresh-install-preflight-no-affected-state"); err != nil {
		return zero, err
	}
	p := store.Publication{Workspace: r.Workspace, Principal: membership.Principal, RequestID: r.ID, Fingerprint: fingerprint, Root: string(items[0].pkg.ArtifactIdentity().Digest())}
	for _, item := range items {
		doc, _ := item.pkg.Manifest()
		pkg := item.pkg
		lockHash, _ := pkg.ExactLock().Digest()
		sourceHash, _ := pkg.SourceArchiveHash()
		a := store.Artifact{Identity: string(pkg.ArtifactIdentity().Digest()), PackageID: string(doc.Package.PackageID), Version: string(doc.Package.Version), ContentHash: string(pkg.ContentHash()), LockHash: string(lockHash), SourceArchiveHash: string(sourceHash), Manifest: pkg.ManifestBytes(), IdentityBytes: pkg.ArtifactBytes(), LockBytes: pkg.LockBytes(), PolicyDigest: o.Policy.Digest(), Retention: item.approval.Retention, ValidationDigest: validation}
		a.Rights, _ = json.Marshal(doc.Package.Rights)
		if err = step("before-object-persist"); err != nil {
			return zero, err
		}
		for _, entry := range pkg.Entries() {
			key, e := o.Objects.Put(ctx, entry.Bytes())
			if e != nil {
				return zero, e
			}
			a.Objects = append(a.Objects, store.ObjectRef{Path: entry.Path(), Key: key})
		}
		snapshot, e := pkg.Export()
		if e != nil {
			return zero, e
		}
		a.ArchiveKey, e = o.Objects.Put(ctx, snapshot.Bytes())
		if e != nil {
			return zero, e
		}
		a.Objects = append(a.Objects, store.ObjectRef{Path: "@archive", Key: a.ArchiveKey})
		p.Artifacts = append(p.Artifacts, a)
		if err = step("objects-persisted"); err != nil {
			return zero, err
		}
	}
	for _, a := range p.Artifacts {
		for _, ref := range a.Objects {
			if err = o.Objects.Verify(ctx, ref.Key); err != nil {
				return zero, err
			}
		}
	}
	if err = step("before-publish"); err != nil {
		return zero, err
	}
	result, err := o.Repository.Publish(ctx, p)
	if err != nil {
		if !errors.Is(err, store.ErrUnknownCommit) {
			return zero, err
		}
		// Cancellation cannot convert an acknowledged or unknown COMMIT into a
		// rollback claim. Bound recovery time independently, and never delete bytes.
		recovery, cancel := context.WithTimeout(context.WithoutCancel(ctx), 5*time.Second)
		defer cancel()
		resolved, resolveErr := o.Repository.Resolve(recovery, r.Workspace, membership.Principal, r.ID, fingerprint)
		if resolveErr == nil {
			resolved, resolveErr = checkResult(resolved)
			if resolveErr == nil {
				return resolved, nil
			}
		}
		return zero, errors.Join(store.ErrUnknownCommit, resolveErr)
	}
	result, err = checkResult(result)
	if err != nil {
		return zero, errors.Join(store.ErrUnknownCommit, err)
	}
	// Failure of acknowledgement/audit after COMMIT is reported as unknown;
	// exact-request recovery uses the same authoritative database path.
	if err = o.Observe("committed"); err != nil {
		return zero, errors.Join(store.ErrUnknownCommit, err)
	}
	return result, nil
}
