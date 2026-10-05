// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package install

import (
	"archive/zip"
	"bytes"
	"context"
	"crypto/ed25519"
	"crypto/rand"
	"errors"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/dependency"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func fixture(t *testing.T, extra map[string][]byte) *archive.Package {
	t.Helper()
	files := fixtures.Files("test.publisher/fixture", "assets", "")
	for k, v := range extra {
		files[k] = v
	}
	p, err := fixtures.Build(files)
	if err != nil {
		t.Fatal(err)
	}
	return p
}

func approval(t *testing.T, p *archive.Package) Approval {
	t.Helper()
	d, err := p.Manifest()
	if err != nil {
		t.Fatal(err)
	}
	return Approval{RightsDigest: RightsDigest(*d.Package), Retention: "retain-fixture", Safety: "ACTIVE"}
}

func TestContentRejectsExecutablesAndUndeclaredBinary(t *testing.T) {
	for name, data := range map[string][]byte{
		"content/bytecode.txt": []byte("\x1bLua"), "content/luajit.txt": []byte("\x1bLJ"), "content/native.txt": []byte("\x7fELF"), "content/windows.txt": []byte("MZpayload"),
		"content/wasm.txt": {0, 97, 115, 109}, "content/mach.txt": {0xfe, 0xed, 0xfa, 0xcf}, "content/raw.bin": {0, 1, 2, 3}, "content/native.so": []byte("plain"), "lua/invalid.lua": []byte("\xff"),
	} {
		t.Run(name, func(t *testing.T) {
			if err := validateContent(fixture(t, map[string][]byte{name: data})); err == nil {
				t.Fatal("unsafe content accepted")
			}
		})
	}
	if err := validateContent(fixture(t, map[string][]byte{"content/image.png": []byte("\x89PNG\r\n\x1a\nimage")})); err != nil {
		t.Fatal(err)
	}
}

func TestStagingPortableArchiveRejectionsAndCleanup(t *testing.T) {
	for _, name := range []string{"../escape", "/absolute", "C:/drive", "a\\b", "a/../b", "a//b", "a/CON", "a\x00b"} {
		t.Run(strings.ReplaceAll(name, "/", "_"), func(t *testing.T) {
			var buf bytes.Buffer
			w := zip.NewWriter(&buf)
			f, err := w.Create(name)
			if err != nil {
				t.Fatal(err)
			}
			_, _ = f.Write([]byte("unsafe"))
			if err = w.Close(); err != nil {
				t.Fatal(err)
			}
			root := t.TempDir()
			_, cleanup, err := stage(context.Background(), root, []Input{{Archive: bytes.NewReader(buf.Bytes())}}, extension.DefaultSupport)
			cleanup()
			if err == nil {
				t.Fatal("unsafe archive accepted")
			}
			rows, _ := os.ReadDir(root)
			if len(rows) != 0 {
				t.Fatal("staging residue")
			}
		})
	}
	for _, mode := range []os.FileMode{os.ModeSymlink | 0777, os.ModeDevice | 0600, os.ModeNamedPipe | 0600} {
		var buf bytes.Buffer
		w := zip.NewWriter(&buf)
		h := &zip.FileHeader{Name: "escape", Method: zip.Store}
		h.SetMode(mode)
		f, _ := w.CreateHeader(h)
		_, _ = f.Write([]byte("target"))
		_ = w.Close()
		_, cleanup, err := stage(context.Background(), t.TempDir(), []Input{{Archive: bytes.NewReader(buf.Bytes())}}, extension.DefaultSupport)
		cleanup()
		if err == nil {
			t.Fatal("special file accepted", mode)
		}
	}
	for _, names := range [][]string{{"A.txt", "a.txt"}, {"é.txt", "e\u0301.txt"}, {"same.txt", "same.txt"}} {
		var buf bytes.Buffer
		w := zip.NewWriter(&buf)
		for _, n := range names {
			f, _ := w.Create(n)
			_, _ = f.Write([]byte("x"))
		}
		_ = w.Close()
		_, cleanup, err := stage(context.Background(), t.TempDir(), []Input{{Archive: bytes.NewReader(buf.Bytes())}}, extension.DefaultSupport)
		cleanup()
		if err == nil {
			t.Fatal("collision accepted", names)
		}
	}
}

func TestStagingBindsImmutableSourceAndHonorsCancellation(t *testing.T) {
	p := fixture(t, nil)
	raw, err := fixtures.Archive(p)
	if err != nil {
		t.Fatal(err)
	}
	root := t.TempDir()
	items, cleanup, err := stage(context.Background(), root, []Input{{Archive: bytes.NewReader(raw)}}, extension.DefaultSupport)
	if err != nil {
		t.Fatal(err)
	}
	if len(items) != 1 || items[0].pkg.ArtifactIdentity().Digest() != p.ArtifactIdentity().Digest() {
		t.Fatal("identity lost")
	}
	rows, _ := os.ReadDir(root)
	if len(rows) != 1 {
		t.Fatal("missing private staging")
	}
	files, _ := os.ReadDir(filepath.Join(root, rows[0].Name()))
	if len(files) != 1 {
		t.Fatal("package paths unpacked")
	}
	cleanup()
	rows, _ = os.ReadDir(root)
	if len(rows) != 0 {
		t.Fatal("cleanup failed")
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	_, cleanup, err = stage(ctx, root, []Input{{Archive: bytes.NewReader(raw)}}, extension.DefaultSupport)
	cleanup()
	if !errors.Is(err, context.Canceled) {
		t.Fatal(err)
	}
	_, cleanup, err = stage(context.Background(), root, make([]Input, MaxGraphPackages+1), extension.DefaultSupport)
	cleanup()
	if err == nil {
		t.Fatal("unbounded graph")
	}
}

func TestPolicyRequiresActualBoundSignaturesAndIndependentCertification(t *testing.T) {
	p := fixture(t, nil)
	a := approval(t, p)
	pub, private, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	cp, ck, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	now := time.Now().Unix()
	c := PolicyConfig{Context: "production", HostMajor: 1, Keys: map[string]Key{"publisher": {Public: pub, Publisher: "test.publisher", State: "ACTIVE", NotBefore: now - 100, NotAfter: now + 100}, "certifier": {Public: cp, Certification: true, State: "ACTIVE", NotBefore: now - 100, NotAfter: now + 100}}, Artifacts: map[string]Approval{string(p.ArtifactIdentity().Digest()): a}}
	sign := func(kind, key string, k ed25519.PrivateKey) *Attestation {
		b, e := SigningBytes(kind, p, a.Tests, now)
		if e != nil {
			t.Fatal(e)
		}
		return &Attestation{KeyID: key, SignedAt: now, Signature: ed25519.Sign(k, b)}
	}
	e := Evidence{Publisher: sign("publisher", "publisher", private), Certification: sign("certification", "certifier", ck)}
	policy, err := NewPolicy(c)
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err = policy.validate(p, e); err != nil {
		t.Fatal(err)
	}
	for name, mutate := range map[string]func(*PolicyConfig, *Evidence){
		"unsigned": func(c *PolicyConfig, e *Evidence) { e.Publisher = nil; e.Certification = nil }, "publisher-only": func(c *PolicyConfig, e *Evidence) { e.Certification = nil },
		"bad-signature": func(c *PolicyConfig, e *Evidence) { e.Publisher.Signature = []byte("claimed-pass") }, "swapped-roles": func(c *PolicyConfig, e *Evidence) { e.Certification = e.Publisher },
		"revoked": func(c *PolicyConfig, e *Evidence) {
			k := c.Keys["publisher"]
			k.State = "REVOKED"
			c.Keys["publisher"] = k
		},
		"compromised": func(c *PolicyConfig, e *Evidence) {
			k := c.Keys["publisher"]
			k.State = "COMPROMISED"
			c.Keys["publisher"] = k
		},
		"expired": func(c *PolicyConfig, e *Evidence) {
			k := c.Keys["publisher"]
			k.State = "EXPIRED"
			c.Keys["publisher"] = k
		},
		"publisher-mismatch": func(c *PolicyConfig, e *Evidence) {
			k := c.Keys["publisher"]
			k.Publisher = "other.publisher"
			c.Keys["publisher"] = k
		},
		"rights-mismatch": func(c *PolicyConfig, e *Evidence) {
			v := c.Artifacts[string(p.ArtifactIdentity().Digest())]
			v.RightsDigest = "sha256:" + strings.Repeat("f", 64)
			c.Artifacts[string(p.ArtifactIdentity().Digest())] = v
		},
		"suite-changed": func(c *PolicyConfig, e *Evidence) {
			v := c.Artifacts[string(p.ArtifactIdentity().Digest())]
			v.Tests = []Test{{Name: "changed", Source: []byte("return true")}}
			c.Artifacts[string(p.ArtifactIdentity().Digest())] = v
		},
	} {
		t.Run(name, func(t *testing.T) {
			owned, _ := NewPolicy(c)
			copyConfig := owned.config
			e2 := Evidence{Publisher: &Attestation{KeyID: e.Publisher.KeyID, SignedAt: e.Publisher.SignedAt, Signature: append([]byte(nil), e.Publisher.Signature...)}, Certification: &Attestation{KeyID: e.Certification.KeyID, SignedAt: e.Certification.SignedAt, Signature: append([]byte(nil), e.Certification.Signature...)}}
			mutate(&copyConfig, &e2)
			policy, err := NewPolicy(copyConfig)
			if err == nil {
				_, _, err = policy.validate(p, e2)
			}
			if err == nil {
				t.Fatal("untrusted policy accepted")
			}
		})
	}
	// Retired keys remain valid only for signatures made before retirement.
	k := c.Keys["publisher"]
	k.State = "RETIRED"
	k.RetiredAt = now + 1
	c.Keys["publisher"] = k
	policy, err = NewPolicy(c)
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err = policy.validate(p, e); err != nil {
		t.Fatal(err)
	}
}

func TestExactGraphRejectsMissingExtraChangedAndFeatureMismatchedArtifacts(t *testing.T) {
	depFiles := fixtures.Files("test.publisher/library", "library", "")
	dep, err := fixtures.Build(depFiles)
	if err != nil {
		t.Fatal(err)
	}
	rootFiles := fixtures.Files("test.publisher/root", "assets", "")
	rootFiles[archive.ManifestPath] = append(rootFiles[archive.ManifestPath], []byte("\n[[dependencies]]\npackage_id = \"test.publisher/library\"\nversion = \"1.0.0\"\noptional = false\nfeatures = []\n")...)
	root, err := fixtures.Build(rootFiles, dep.ExactLock().Packages()...)
	if err != nil {
		t.Fatal(err)
	}
	if err = validateGraph([]staged{{pkg: root}, {pkg: dep}}); err != nil {
		t.Fatal(err)
	}
	depFiles["content/readme.txt"] = []byte("changed dependency bytes")
	changed, err := fixtures.Build(depFiles)
	if err != nil {
		t.Fatal(err)
	}
	for name, items := range map[string][]staged{"missing": {{pkg: root}}, "extra": {{pkg: root}, {pkg: dep}, {pkg: fixture(t, nil)}}, "duplicate": {{pkg: root}, {pkg: root}}, "changed-bytes": {{pkg: root}, {pkg: changed}}} {
		t.Run(name, func(t *testing.T) {
			if err := validateGraph(items); err == nil {
				t.Fatal("non-exact dependency graph accepted")
			}
		})
	}
	node := dep.ExactLock().Packages()[0]
	node.Features = []string{"feature"}
	rootFiles[archive.ManifestPath] = []byte(strings.Replace(string(rootFiles[archive.ManifestPath]), "features = []", "features = [\"feature\"]", 1))
	featureRoot, err := fixtures.Build(rootFiles, node)
	if err != nil {
		t.Fatal(err)
	}
	if err = validateGraph([]staged{{pkg: featureRoot}, {pkg: dep}}); err == nil {
		t.Fatal("different dependency feature lock accepted")
	}
	// A supplied artifact's own lock is evidence, not just its content/version.
	if _, err = dependency.BuildExactLock("test.publisher/library", []dependency.LockedPackage{node, node}); err == nil {
		t.Fatal("duplicate exact graph accepted")
	}
}

func TestCapabilitiesAndFallbackEvidenceCannotEscalate(t *testing.T) {
	for _, declaration := range []string{"\nrequired = [\"host.event\"]\n", "\nrequired = []\n[[capabilities.optional]]\nname = \"host.log\"\nfallback = \"continue without log\"\n"} {
		files := fixtures.Files("test.publisher/capability", "assets", "")
		files[archive.ManifestPath] = []byte(strings.Replace(string(files[archive.ManifestPath]), "\nrequired = []\n", declaration, 1))
		p, err := fixtures.Build(files)
		if err != nil {
			t.Fatal(err)
		}
		a := approval(t, p)
		policy, err := NewPolicy(PolicyConfig{Context: "ci", HostMajor: 1, Artifacts: map[string]Approval{string(p.ArtifactIdentity().Digest()): a}})
		if err != nil {
			t.Fatal(err)
		}
		if _, _, err = policy.validate(p, Evidence{}); err == nil {
			t.Fatal("missing capability/fallback test admitted")
		}
	}
}
