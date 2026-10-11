// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package m2

import (
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/json"
	"encoding/pem"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"math/big"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func privateDirectory(t *testing.T) string {
	t.Helper()
	p, e := os.MkdirTemp("", "m2-")
	if e != nil {
		t.Fatal(e)
	}
	t.Cleanup(func() {
		if e := os.RemoveAll(p); e != nil {
			t.Error(e)
		}
	})
	return p
}

func TestPrivateConfigurationLoadsAndRejectsWritableOrUnknownFields(t *testing.T) {
	dir := privateDirectory(t)
	c := Config{Version: 1, DeploymentID: "m2-test-deployment", Source: "sha256:" + strings.Repeat("0", 64), Origin: "https://localhost:8443", SupervisorSocket: "/run/supervisor/service.sock", WorkerSocket: "/run/worker/service.sock", ObjectSocket: "/run/object/service.sock", Runner: "/app/lua-runner", RunnerHash: "sha256:" + strings.Repeat("1", 64), Limits: profile.DefaultLimits(), TLS: TLSFiles{"/run/ca", "/run/cert", "/run/key"}}
	type plain Config
	b, e := json.Marshal(plain(c))
	if e != nil {
		t.Fatal(e)
	}
	p := filepath.Join(dir, "config.json")
	if e = os.WriteFile(p, b, 0400); e != nil {
		t.Fatal(e)
	}
	if got, e := LoadConfig(p); e != nil || got.DeploymentID != c.DeploymentID {
		t.Fatal("valid readonly config rejected", e)
	}
	if e = os.Chmod(p, 0600); e != nil {
		t.Fatal(e)
	}
	if _, e = LoadConfig(p); e == nil {
		t.Fatal("writable config accepted")
	}
	if e = os.Remove(p); e != nil {
		t.Fatal(e)
	}
	if e = os.WriteFile(p, append(b[:len(b)-1], []byte(`,"arbitrary_endpoint":"https://other.invalid"}`)...), 0400); e != nil {
		t.Fatal(e)
	}
	if _, e = LoadConfig(p); e == nil {
		t.Fatal("unknown operator field accepted")
	}
}
func pkiFixture(t *testing.T, dir string, roles ...string) map[string]TLSFiles {
	t.Helper()
	key, e := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if e != nil {
		t.Fatal(e)
	}
	now := time.Now()
	ca := &x509.Certificate{SerialNumber: big.NewInt(1), Subject: pkix.Name{CommonName: "M2 synthetic CA"}, NotBefore: now.Add(-time.Minute), NotAfter: now.Add(time.Hour), IsCA: true, BasicConstraintsValid: true, KeyUsage: x509.KeyUsageCertSign | x509.KeyUsageDigitalSignature}
	der, e := x509.CreateCertificate(rand.Reader, ca, ca, &key.PublicKey, key)
	if e != nil {
		t.Fatal(e)
	}
	caFile := filepath.Join(dir, "ca.pem")
	if e = os.WriteFile(caFile, pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: der}), 0400); e != nil {
		t.Fatal(e)
	}
	files := map[string]TLSFiles{}
	for i, role := range roles {
		k, e := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
		if e != nil {
			t.Fatal(e)
		}
		c := &x509.Certificate{SerialNumber: big.NewInt(int64(i + 2)), DNSNames: []string{role}, NotBefore: now.Add(-time.Minute), NotAfter: now.Add(time.Hour), KeyUsage: x509.KeyUsageDigitalSignature, ExtKeyUsage: []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth, x509.ExtKeyUsageClientAuth}}
		b, e := x509.CreateCertificate(rand.Reader, c, ca, &k.PublicKey, key)
		if e != nil {
			t.Fatal(e)
		}
		kb, e := x509.MarshalPKCS8PrivateKey(k)
		if e != nil {
			t.Fatal(e)
		}
		f := TLSFiles{CA: caFile, Certificate: filepath.Join(dir, role+".pem"), Key: filepath.Join(dir, role+".key")}
		for path, data := range map[string][]byte{f.Certificate: pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: b}), f.Key: pem.EncodeToMemory(&pem.Block{Type: "PRIVATE KEY", Bytes: kb})} {
			if e = os.WriteFile(path, data, 0400); e != nil {
				t.Fatal(e)
			}
		}
		files[role] = f
	}
	return files
}
func TestSecretsAndPeerRolesRejectUnsafeInputs(t *testing.T) {
	dir := privateDirectory(t)
	p := filepath.Join(dir, "secret")
	for _, mode := range []os.FileMode{0400, 0440, 0444, 0600, 0644, 0200} {
		_ = os.Remove(p)
		if e := os.WriteFile(p, []byte("synthetic-test-only"), 0600); e != nil {
			t.Fatal(e)
		}
		if e := os.Chmod(p, mode); e != nil {
			t.Fatal(e)
		}
		b, e := ReadSecret(p, 256)
		clear(b)
		if (mode == 0400 || mode == 0440) != (e == nil) {
			t.Fatalf("unexpected secret mode decision: %o", mode)
		}
	}
	os.Chmod(p, 0400)
	link := filepath.Join(dir, "link")
	if e := os.Symlink(p, link); e != nil {
		t.Fatal(e)
	}
	if _, e := ReadSecret(link, 256); e == nil {
		t.Fatal("symlink accepted")
	}
	if _, e := ReadSecret(filepath.Join(dir, "missing"), 256); e == nil {
		t.Fatal("missing secret accepted")
	}
	files := pkiFixture(t, dir, "platformd", "workerd", "untrusted")
	if _, e := TLSConfig(files["platformd"], "workerd", true); e != nil {
		t.Fatal(e)
	}
	if _, e := json.Marshal(Config{}); e == nil {
		t.Fatal("private configuration exported")
	}
	if e := os.Remove(files["platformd"].Key); e != nil {
		t.Fatal(e)
	}
	if e := os.WriteFile(files["platformd"].Key, []byte("malformed"), 0400); e != nil {
		t.Fatal(e)
	}
	if _, e := TLSConfig(files["platformd"], "workerd", true); e == nil {
		t.Fatal("malformed TLS material accepted")
	}
}

func TestDeclaredDependencyArchiveMapIsExactAndBounded(t *testing.T) {
	root := "sha256:" + strings.Repeat("1", 64)
	dep := "sha256:" + strings.Repeat("2", 64)
	base := GamePlan{Root: root, ArchiveFile: "/run/operator/root.zip", Dependencies: []string{dep}, DependencyArchiveFiles: map[string]string{dep: "/run/operator/dependency.zip"}}
	for _, name := range []string{"valid", "missing", "extra", "duplicate", "root", "bad-identity", "relative", "same-file", "too-many", "nil-compatible"} {
		t.Run(name, func(t *testing.T) {
			g := base
			g.Dependencies = append([]string(nil), base.Dependencies...)
			g.DependencyArchiveFiles = map[string]string{dep: base.DependencyArchiveFiles[dep]}
			switch name {
			case "missing":
				g.DependencyArchiveFiles = nil
			case "extra":
				g.DependencyArchiveFiles[root] = "/run/operator/extra.zip"
			case "duplicate":
				g.Dependencies = append(g.Dependencies, dep)
			case "root":
				g.Dependencies = []string{root}
				g.DependencyArchiveFiles = map[string]string{root: "/run/operator/dependency.zip"}
			case "bad-identity":
				g.Dependencies = []string{"unverified"}
				g.DependencyArchiveFiles = map[string]string{"unverified": "/run/operator/dependency.zip"}
			case "relative":
				g.DependencyArchiveFiles[dep] = "dependency.zip"
			case "same-file":
				g.DependencyArchiveFiles[dep] = g.ArchiveFile
			case "too-many":
				g.Dependencies = make([]string, install.MaxGraphPackages)
			case "nil-compatible":
				g.Dependencies = nil
				g.DependencyArchiveFiles = nil
			}
			err := g.ValidateDependencyArchives()
			if (name == "valid" || name == "nil-compatible") != (err == nil) {
				t.Fatal("archive construction constraint", name, err)
			}
		})
	}
}
