// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package store defines internal workspace installation persistence. It does
// not expose a public protocol, grant Host capabilities, or start Sessions.
package store

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/json"
	"errors"
	"fmt"
	"regexp"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
)

var (
	ErrDenied        = errors.New("workspace authorization denied")
	ErrNotFound      = errors.New("committed installation not found")
	ErrConflict      = errors.New("installation request identity conflicts")
	ErrMigration     = errors.New("fresh installation requires unsupported migration")
	ErrUnknownCommit = errors.New("installation commit outcome unknown; resolve exact request before retry")
)

var identifier = regexp.MustCompile(`^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,127}$`)

func ValidID(s string) bool { return identifier.MatchString(s) }

type Credential string

func (Credential) String() string               { return "<credential>" }
func (Credential) GoString() string             { return "<credential>" }
func (Credential) MarshalJSON() ([]byte, error) { return nil, ErrDenied }

type Membership struct {
	Principal, Workspace string
	Install, Read        bool
}

// Access is an immutable operator-configured authentication/authorization seam.
// Requests supply credentials, never a claimed principal or trusted boolean.
type Access struct{ members map[[32]byte][]Membership }

func NewAccess(values map[Credential][]Membership) (*Access, error) {
	a := &Access{members: map[[32]byte][]Membership{}}
	for credential, rows := range values {
		if len(credential) < 16 || len(rows) == 0 {
			return nil, ErrDenied
		}
		for _, r := range rows {
			if !ValidID(r.Principal) || !ValidID(r.Workspace) {
				return nil, ErrDenied
			}
		}
		a.members[sha256.Sum256([]byte(credential))] = append([]Membership(nil), rows...)
	}
	return a, nil
}
func (a *Access) Authorize(c Credential, workspace string, install bool) (Membership, error) {
	if a == nil || !ValidID(workspace) {
		return Membership{}, ErrDenied
	}
	for _, m := range a.members[sha256.Sum256([]byte(c))] {
		if m.Workspace == workspace && ((install && m.Install) || (!install && m.Read)) {
			return m, nil
		}
	}
	return Membership{}, ErrDenied
}

type ObjectRef struct{ Path, Key string }
type Artifact struct {
	Identity, PackageID, Version, ContentHash, LockHash string
	SourceArchiveHash, ArchiveKey                       string
	Manifest, IdentityBytes, LockBytes, Rights          []byte
	PolicyDigest, Retention, ValidationDigest           string
	Objects                                             []ObjectRef
}
type Publication struct {
	Workspace, Principal, RequestID, Fingerprint, Root string
	Artifacts                                          []Artifact
}
type Result struct {
	Workspace, RequestID, Fingerprint, Root string
	Artifacts                               []string
}
type Repository interface {
	Preflight(context.Context, string, []string) error
	Publish(context.Context, Publication) (Result, error)
	Resolve(context.Context, string, string, string, string) (Result, error)
	Lookup(context.Context, string, string, string) (Artifact, error)
}

type Reader struct {
	repository Repository
	objects    *object.Directory
	access     *Access
	support    extension.Support
}

func NewReader(repo Repository, objects *object.Directory, access *Access, support extension.Support) (*Reader, error) {
	if repo == nil || objects == nil || access == nil {
		return nil, ErrDenied
	}
	return &Reader{repo, objects, access, support}, nil
}
func (r *Reader) Load(ctx context.Context, c Credential, workspace, identity string) (*archive.Package, error) {
	if _, err := model.ParseContentHash(identity); err != nil {
		return nil, ErrDenied
	}
	m, err := r.access.Authorize(c, workspace, false)
	if err != nil {
		return nil, err
	}
	a, err := r.repository.Lookup(ctx, workspace, m.Principal, identity)
	if err != nil {
		return nil, err
	}
	if a.Identity != identity {
		return nil, object.ErrIntegrity
	}
	return VerifyArtifact(ctx, r.objects, a, r.support)
}

// VerifyArtifact binds the complete index to canonical immutable content. It is
// used before publication and after an authorized lookup; it grants no access.
func VerifyArtifact(ctx context.Context, objects *object.Directory, a Artifact, support extension.Support) (*archive.Package, error) {
	raw, err := objects.Read(ctx, a.ArchiveKey)
	if err != nil {
		return nil, err
	}
	pkg, err := archive.ImportBytes(raw, support)
	if err != nil {
		return nil, err
	}
	lockHash, err := pkg.ExactLock().Digest()
	if err != nil {
		return nil, err
	}
	d, err := pkg.Manifest()
	if err != nil {
		return nil, err
	}
	rights, err := json.Marshal(d.Package.Rights)
	if err != nil {
		return nil, err
	}
	if string(pkg.ArtifactIdentity().Digest()) != a.Identity || string(pkg.ContentHash()) != a.ContentHash || string(lockHash) != a.LockHash || string(d.Package.PackageID) != a.PackageID || string(d.Package.Version) != a.Version ||
		!bytes.Equal(pkg.ManifestBytes(), a.Manifest) || !bytes.Equal(pkg.ArtifactBytes(), a.IdentityBytes) || !bytes.Equal(pkg.LockBytes(), a.LockBytes) || !bytes.Equal(rights, a.Rights) {
		return nil, fmt.Errorf("stored package: %w", object.ErrIntegrity)
	}
	for _, digest := range []string{a.SourceArchiveHash, a.PolicyDigest, a.ValidationDigest} {
		if _, err := model.ParseContentHash(digest); err != nil {
			return nil, object.ErrIntegrity
		}
	}
	if a.Retention == "" || len(a.Retention) > 128 {
		return nil, object.ErrIntegrity
	}
	expected := map[string]string{"@archive": a.ArchiveKey}
	for _, entry := range pkg.Entries() {
		expected[entry.Path()] = object.Hash(entry.Bytes())
	}
	if len(expected) != len(a.Objects) {
		return nil, object.ErrIntegrity
	}
	for _, ref := range a.Objects {
		if expected[ref.Path] != ref.Key {
			return nil, object.ErrIntegrity
		}
		delete(expected, ref.Path)
		if err := objects.Verify(ctx, ref.Key); err != nil {
			return nil, err
		}
	}
	return pkg, nil
}
