// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"hash"
	"io"
	"os"
	"path/filepath"

	"github.com/zyc14588/TRPG_PLATFORM/internal/deployment/m2"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

type platformPackageComponents struct {
	repository   *postgres.Repository
	reader       *store.Reader
	configs      []launch.Configuration
	descriptions []player.Description
}

func openM2Packages(ctx context.Context, c m2.Config, core *platformPlayerCoreData, objects *m2.ObjectClient, launcher *m2.SupervisorLauncher, plan m2.OperatorPlan) (*platformPackageComponents, error) {
	if len(plan.Games) < 1 || len(plan.Games) > 32 {
		return nil, m2.ErrConfiguration
	}
	pc := plan.InstallPolicy
	if pc.Context != "production" && plan.Classification != "TEST_ONLY_STANDARD_CARRIER" {
		return nil, m2.ErrConfiguration
	}
	policy, e := install.NewPolicy(pc)
	if e != nil {
		return nil, e
	}
	grants := map[store.Credential][]store.Membership{}
	for _, g := range plan.Games {
		raw, e := m2.ReadSecret(g.InstallCredentialFile, 256)
		if e != nil {
			return nil, e
		}
		credential := store.Credential(string(raw))
		clear(raw)
		grants[credential] = append(grants[credential], store.Membership{Workspace: g.Workspace, Principal: g.Operator, Install: true, Read: true})
	}
	access, e := store.NewAccess(grants)
	if e != nil {
		return nil, e
	}
	dsn, e := m2.ReadSecret(c.DSNFile, 16384)
	if e != nil {
		return nil, e
	}
	defer clear(dsn)
	repo, e := postgres.OpenInstallationRepository(ctx, string(dsn), objects, extension.DefaultSupport, nil)
	if e != nil {
		return nil, e
	}
	fail := func(e error) (*platformPackageComponents, error) { _ = repo.Close(); return nil, e }
	if e = repo.Bootstrap(ctx); e != nil {
		return fail(e)
	}
	runtime := install.RuntimeConfig{Runner: c.Runner, SHA256: c.RunnerHash, Limits: c.Limits, Launcher: launcher}
	observe := func(e install.Execution) error { return nil }
	installer, e := install.New(install.Options{StagingRoot: c.StagingRoot, Policy: policy, Access: access, Objects: objects, Repository: repo, Support: extension.DefaultSupport, Runtime: runtime, Observe: func(string) error { return nil }, Execution: observe})
	if e != nil {
		return fail(e)
	}
	reader, e := store.NewReader(repo, objects, access, extension.DefaultSupport)
	if e != nil {
		return fail(e)
	}
	factory, e := install.NewSessionFactory(install.SessionOptions{Reader: reader, Policy: policy, Repository: core.guarded.SessionRepository(), Runtime: runtime, Execution: observe, Audit: func(a data.Audit) error {
		if a.Outcome != "PASS" && a.Outcome != "READ_VALIDATED" && a.Outcome != "VALIDATED_PENDING_COMMIT" && a.Outcome != "" {
			fmt.Fprintln(os.Stderr, "M2_INSTALLED_HOST_EXECUTION_GUARD_DENIED")
		}
		return nil
	}, Validate: func(context.Context, data.Commit) error { return nil }})
	if e != nil {
		return fail(e)
	}
	p := &platformPackageComponents{repository: repo, reader: reader}
	for _, g := range plan.Games {
		if e = repo.ProvisionWorkspace(ctx, g.Workspace); e != nil {
			return fail(e)
		}
		raw, e := m2.ReadSecret(g.InstallCredentialFile, 256)
		if e != nil {
			return fail(e)
		}
		credential := store.Credential(string(raw))
		clear(raw)
		result, e := installM2Game(ctx, installer, g, credential)
		if e != nil {
			return fail(e)
		}
		if result.Root != g.Root {
			return fail(install.ErrPolicy)
		}
		request := install.SessionRequest{Credential: credential, Workspace: g.Workspace, Session: g.Game, Root: g.Root, Dependencies: g.Dependencies, Evidence: g.Evidence}
		graphHash, _, e := factory.Presentation(ctx, request)
		if e != nil || graphHash != g.GraphHash {
			return fail(install.ErrPolicy)
		}
		cfg := auth.RoomSecret(launch.ConfigurationData{ID: g.Configuration, WorkspaceID: g.Workspace, Factory: factory, Request: request, Seats: g.Seats, ContentTags: g.ContentTags, SafetyTags: g.SafetyTags})
		p.configs = append(p.configs, cfg)
		p.descriptions = append(p.descriptions, player.Description{WorkspaceID: g.Workspace, ConfigurationID: g.Configuration, GameID: g.Game, Title: g.Title})
		// Reader and installation repository have both exercised the same object
		// service. No platform directory backend is opened in the M2 branch.
		pkg, e := reader.Load(ctx, credential, g.Workspace, g.Root)
		if e != nil {
			return fail(e)
		}
		a, e := pkg.Export()
		if e != nil {
			return fail(e)
		}
		if len(a.Bytes()) == 0 || bytes.Equal(a.Bytes(), nil) || !checkpoint.IsDigest(g.ConfigurationHash) {
			return fail(install.ErrPolicy)
		}
	}
	return p, nil
}

func stringCredential(v []byte) store.Credential { return store.Credential(string(v)) }

// Each explicit archive is inspected through one owned read-only descriptor.
// Its original bytes, inode and metadata remain bound while the unchanged
// installer reads them; the map itself proves no package identity.
type m2ArchiveReader struct {
	file   *os.File
	path   string
	info   os.FileInfo
	hash   hash.Hash
	source string
	read   int64
}

func (r *m2ArchiveReader) unchanged() bool {
	if !m2ArchiveDirectories(r.path) {
		return false
	}
	a, e := r.file.Stat()
	if e != nil {
		return false
	}
	b, e := os.Lstat(r.path)
	return e == nil && a.Mode().IsRegular() && b.Mode().IsRegular() && os.SameFile(r.info, a) && os.SameFile(r.info, b) && a.Mode() == r.info.Mode() && b.Mode() == r.info.Mode() && a.Size() == r.info.Size() && b.Size() == r.info.Size() && a.ModTime().Equal(r.info.ModTime()) && b.ModTime().Equal(r.info.ModTime())
}
func m2ArchiveDirectories(path string) bool {
	for parent := filepath.Dir(path); ; parent = filepath.Dir(parent) {
		info, e := os.Lstat(parent)
		if e != nil || !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
			return false
		}
		if parent == string(filepath.Separator) {
			return true
		}
	}
}
func (r *m2ArchiveReader) Read(p []byte) (int, error) {
	if !r.unchanged() {
		return 0, m2.ErrConfiguration
	}
	n, e := r.file.Read(p)
	r.read += int64(n)
	_, _ = r.hash.Write(p[:n])
	if !r.unchanged() || r.read > r.info.Size() || r.read == r.info.Size() && "sha256:"+hex.EncodeToString(r.hash.Sum(nil)) != r.source {
		return n, m2.ErrConfiguration
	}
	return n, e
}
func openM2Archive(ctx context.Context, path, id string) (*m2ArchiveReader, error) {
	if ctx == nil || ctx.Err() != nil || !filepath.IsAbs(path) || !checkpoint.IsDigest(id) {
		return nil, m2.ErrConfiguration
	}
	if !m2ArchiveDirectories(path) {
		return nil, m2.ErrConfiguration
	}
	before, e := os.Lstat(path)
	if e != nil || !before.Mode().IsRegular() || before.Mode().Perm()&0222 != 0 || before.Size() <= 0 || before.Size() > archive.MaxSnapshotBytes {
		return nil, m2.ErrConfiguration
	}
	f, e := os.Open(path)
	if e != nil {
		return nil, m2.ErrConfiguration
	}
	r := &m2ArchiveReader{file: f, path: path, info: before, hash: sha256.New()}
	fail := func() (*m2ArchiveReader, error) { _ = f.Close(); return nil, m2.ErrConfiguration }
	if !r.unchanged() {
		return fail()
	}
	raw, e := io.ReadAll(io.LimitReader(f, archive.MaxSnapshotBytes+1))
	defer clear(raw)
	if e != nil || int64(len(raw)) != before.Size() || !r.unchanged() || ctx.Err() != nil {
		return fail()
	}
	snapshot, e := archive.NewSnapshot(raw)
	if e != nil {
		return fail()
	}
	pkg, e := archive.Import(snapshot, extension.DefaultSupport)
	if e != nil || string(pkg.ArtifactIdentity().Digest()) != id {
		return fail()
	}
	r.source = checkpoint.Hash(raw)
	if _, e = f.Seek(0, io.SeekStart); e != nil {
		return fail()
	}
	return r, nil
}
func installM2Game(ctx context.Context, installer *install.Installer, g m2.GamePlan, credential store.Credential) (result store.Result, failure error) {
	if g.ValidateDependencyArchives() != nil {
		return result, m2.ErrConfiguration
	}
	opened := []*m2ArchiveReader{}
	defer func() {
		for _, r := range opened {
			failure = errors.Join(failure, r.file.Close())
		}
	}()
	root, e := openM2Archive(ctx, g.ArchiveFile, g.Root)
	if e != nil {
		return result, e
	}
	opened = append(opened, root)
	inputs := make([]install.Input, 0, len(g.Dependencies))
	total := root.info.Size()
	for _, id := range g.Dependencies {
		reader, e := openM2Archive(ctx, g.DependencyArchiveFiles[id], id)
		if e != nil {
			return result, e
		}
		opened = append(opened, reader)
		total += reader.info.Size()
		if total > install.MaxGraphBytes {
			return result, m2.ErrConfiguration
		}
		inputs = append(inputs, install.Input{Archive: reader, Evidence: g.Evidence[id]})
	}
	return installer.Install(ctx, install.Request{Workspace: g.Workspace, ID: g.InstallID, Credential: credential, Root: install.Input{Archive: root, Evidence: g.Evidence[g.Root]}, Dependencies: inputs})
}
