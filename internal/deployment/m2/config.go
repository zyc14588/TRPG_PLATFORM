// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package m2 provides finite, private Linux single-host deployment boundaries.
// It never receives account authority, task tokens or an executable from a user.
package m2

import (
	"context"
	"crypto/sha256"
	"crypto/tls"
	"crypto/x509"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/url"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/budget"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/gateway"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/command"
)

var ErrConfiguration = errors.New("M2 deployment configuration rejected")
var ErrPrivate = errors.New("M2 private service unavailable")

type TLSFiles struct{ CA, Certificate, Key string }
type ProviderConfig struct {
	Endpoint                                model.EndpointData
	TimeoutMillis, ResponseBytes, MaxActive int
	MicrosPerToken                          uint64
}

func (p ProviderConfig) Adapter() gateway.AdapterOptions {
	return gateway.AdapterOptions{Endpoint: model.NewEndpoint(p.Endpoint), Timeout: time.Duration(p.TimeoutMillis) * time.Millisecond, ResponseBytes: p.ResponseBytes, MaxActive: p.MaxActive, MicrosPerToken: p.MicrosPerToken}
}

type Config struct {
	SPDXIdentifier                                                                               string `json:"spdx_license_identifier"`
	Version                                                                                      int
	DeploymentID, Source, Origin                                                                 string
	TLS                                                                                          TLSFiles
	DaemonAddress, Upstream, ExternalAddress, StaticRoot                                         string
	SupervisorSocket, WorkerSocket, ObjectSocket                                                 string
	PeerUID                                                                                      uint32
	Runner, RunnerHash                                                                           string
	Limits                                                                                       profile.Limits
	ObjectRoot, StagingRoot                                                                      string
	DSNFile, CookieKeyFile, ReplayKeyFile, InvitationKeyFile, VaultKeyFile, SeedFile, GrantsFile string
	PackagesFile, PoliciesFile                                                                   string
	Provider                                                                                     ProviderConfig
}

func (Config) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private M2 operator configuration>")
}
func (Config) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }

func LoadConfig(path string) (Config, error) {
	var c Config
	b, e := readPrivateDocument(path, 65536)
	defer clear(b)
	if e != nil || len(b) > 65536 || checkpoint.StrictDecode(b, &c, 65536) != nil {
		return Config{}, ErrConfiguration
	}
	if c.Version != 1 || len(c.DeploymentID) < 8 || len(c.DeploymentID) > 128 || !checkpoint.IsDigest(c.Source) || c.Limits.Validate() != nil || !checkpoint.IsDigest(c.RunnerHash) {
		return Config{}, ErrConfiguration
	}
	u, e := url.Parse(c.Origin)
	if e != nil || u.Scheme != "https" || u.Host == "" || u.Path != "" || u.User != nil || u.RawQuery != "" || u.Fragment != "" {
		return Config{}, ErrConfiguration
	}
	for _, p := range []string{c.SupervisorSocket, c.WorkerSocket, c.ObjectSocket, c.Runner, c.TLS.CA, c.TLS.Certificate, c.TLS.Key} {
		if !filepath.IsAbs(p) || strings.ContainsRune(p, 0) {
			return Config{}, ErrConfiguration
		}
	}
	return c, nil
}
func readPrivateDocument(path string, maximum int64) ([]byte, error) {
	if maximum <= 0 || maximum > 4<<20 {
		return nil, ErrConfiguration
	}
	before, e := os.Lstat(path)
	if e != nil || !before.Mode().IsRegular() || (before.Mode().Perm() != 0400 && before.Mode().Perm() != 0440) {
		return nil, ErrConfiguration
	}
	f, e := os.Open(path)
	if e != nil {
		return nil, ErrConfiguration
	}
	defer f.Close()
	opened, e := f.Stat()
	if e != nil || !os.SameFile(before, opened) {
		return nil, ErrConfiguration
	}
	b, e := io.ReadAll(io.LimitReader(f, maximum+1))
	after, ae := os.Lstat(path)
	if e != nil || ae != nil || int64(len(b)) > maximum || !os.SameFile(before, after) || !after.Mode().IsRegular() || (after.Mode().Perm() != 0400 && after.Mode().Perm() != 0440) {
		clear(b)
		return nil, ErrConfiguration
	}
	return b, nil
}
func RequireLinux() error {
	if runtime.GOOS != "linux" || runtime.GOARCH != "amd64" {
		return ErrConfiguration
	}
	// A user namespace cannot prove the supported rootful container identity.
	b, e := os.ReadFile("/proc/self/uid_map")
	if e != nil {
		return ErrConfiguration
	}
	f := strings.Fields(string(b))
	if len(f) != 3 || f[0] != "0" || f[1] != "0" || f[2] != "4294967295" {
		return ErrConfiguration
	}
	return nil
}
func ReadSecret(path string, limit int64) ([]byte, error) {
	before, e := os.Lstat(path)
	if e != nil || !before.Mode().IsRegular() || (before.Mode().Perm() != 0400 && before.Mode().Perm() != 0440) {
		return nil, ErrConfiguration
	}
	b, e := auth.ReadSecretFile(path, limit)
	if e != nil {
		return nil, ErrConfiguration
	}
	v := b.StorageValue()
	info, e := os.Lstat(path)
	if e != nil || !os.SameFile(before, info) || !info.Mode().IsRegular() || (info.Mode().Perm() != 0400 && info.Mode().Perm() != 0440) || len(v) == 0 {
		clear(v)
		return nil, ErrConfiguration
	}
	return v, nil
}
func TLSConfig(files TLSFiles, peer string, server bool) (*tls.Config, error) {
	ca, e := ReadSecret(files.CA, 16384)
	if e != nil {
		return nil, e
	}
	defer clear(ca)
	cert, e := ReadSecret(files.Certificate, 16384)
	if e != nil {
		return nil, e
	}
	defer clear(cert)
	key, e := ReadSecret(files.Key, 16384)
	if e != nil {
		return nil, e
	}
	defer clear(key)
	pool := x509.NewCertPool()
	if !pool.AppendCertsFromPEM(ca) {
		return nil, ErrConfiguration
	}
	pair, e := tls.X509KeyPair(cert, key)
	if e != nil {
		return nil, ErrConfiguration
	}
	c := &tls.Config{MinVersion: tls.VersionTLS13, RootCAs: pool, Certificates: []tls.Certificate{pair}, ServerName: peer}
	if server {
		c.ClientCAs = pool
		c.ClientAuth = tls.RequireAndVerifyClientCert
	}
	c.VerifyConnection = func(s tls.ConnectionState) error {
		if len(s.VerifiedChains) == 0 || len(s.PeerCertificates) == 0 || !peerMatches(s.PeerCertificates[0], peer) {
			return ErrPrivate
		}
		return nil
	}
	return c, nil
}
func VerifyRunner(path, digest string) error {
	f, e := os.Open(path)
	if e != nil {
		return ErrConfiguration
	}
	defer f.Close()
	i, e := f.Stat()
	before, be := os.Lstat(path)
	if e != nil || be != nil || !i.Mode().IsRegular() || !os.SameFile(before, i) || i.Size() > 128<<20 || i.Mode().Perm()&0022 != 0 {
		return ErrConfiguration
	}
	h := sha256.New()
	if _, e = io.Copy(h, f); e != nil || "sha256:"+hex.EncodeToString(h.Sum(nil)) != digest {
		return ErrConfiguration
	}
	return nil
}
func decodeRequest(ctx context.Context, r io.Reader, v any, limit int) error {
	if ctx == nil || ctx.Err() != nil {
		return ErrPrivate
	}
	b, e := io.ReadAll(io.LimitReader(r, int64(limit)+1))
	defer clear(b)
	if e != nil || len(b) > limit || checkpoint.StrictDecode(b, v, limit) != nil {
		return ErrPrivate
	}
	return nil
}
func encode(v any) ([]byte, error) { return json.Marshal(v) }

// OperatorPlan is private trusted construction data, read from a bounded file.
// Certificates and package policies are explicit; startup/health never qualifies
// a model, consents for a person, casts a ready vote or launches a session.
type GamePlan struct {
	Workspace, Operator, InstallID, InstallCredentialFile, ArchiveFile, Root, PackageID, GraphHash, Configuration, ConfigurationHash, Game, Title string
	DependencyArchiveFiles                                                                                                                        map[string]string
	Dependencies, ContentTags, SafetyTags                                                                                                         []string
	Evidence                                                                                                                                      map[string]install.Evidence
	Seats                                                                                                                                         []launch.SeatRule
	Views                                                                                                                                         map[string]command.ViewPolicy
	Commands                                                                                                                                      map[string]install.SchemaReference
	ContextViews                                                                                                                                  map[string]command.ViewPolicy
}

// ValidateDependencyArchives binds only explicit construction inputs. The
// original importer/installer still proves each archive and exact graph.
func (g GamePlan) ValidateDependencyArchives() error {
	if len(g.Dependencies) >= install.MaxGraphPackages || len(g.DependencyArchiveFiles) != len(g.Dependencies) {
		return ErrConfiguration
	}
	seen := map[string]bool{}
	paths := map[string]bool{g.ArchiveFile: true}
	for _, id := range g.Dependencies {
		path, ok := g.DependencyArchiveFiles[id]
		if !checkpoint.IsDigest(id) || id == g.Root || seen[id] || !ok || !filepath.IsAbs(path) || len(path) > 4096 || paths[path] {
			return ErrConfiguration
		}
		seen[id] = true
		paths[path] = true
	}
	return nil
}

type OperatorPlan struct {
	Classification     string
	InstallPolicy      install.PolicyConfig
	Games              []GamePlan
	Certificates       []model.CertificationData
	Defaults, Labels   map[string]string
	WorkspaceLimits    map[string]model.Limits
	Caps               map[string]budget.Caps
	Amount             budget.Units
	MaxCalls           int
	TaskCredentialFile string
}

func ReadOperatorPlan(path string) (OperatorPlan, error) {
	var p OperatorPlan
	before, e := os.Lstat(path)
	if e != nil || !before.Mode().IsRegular() || (before.Mode().Perm() != 0400 && before.Mode().Perm() != 0440) {
		return p, ErrConfiguration
	}
	f, e := os.Open(path)
	if e != nil {
		return p, ErrConfiguration
	}
	defer f.Close()
	after, e := f.Stat()
	if e != nil || !os.SameFile(before, after) || after.Mode().Perm() != before.Mode().Perm() {
		return p, ErrConfiguration
	}
	b, e := io.ReadAll(io.LimitReader(f, 4<<20))
	defer clear(b)
	if e != nil || len(b) >= 4<<20 || checkpoint.StrictDecode(b, &p, 4<<20) != nil || len(p.Games) < 1 || len(p.Games) > 32 {
		return OperatorPlan{}, ErrConfiguration
	}
	current, e := os.Lstat(path)
	if e != nil || !os.SameFile(before, current) || current.Mode().Perm() != before.Mode().Perm() {
		return OperatorPlan{}, ErrConfiguration
	}
	for _, g := range p.Games {
		if g.ValidateDependencyArchives() != nil {
			return OperatorPlan{}, ErrConfiguration
		}
	}
	return p, nil
}

func peerMatches(c *x509.Certificate, roles string) bool {
	for _, role := range strings.Split(roles, "|") {
		if c.VerifyHostname(role) == nil {
			return true
		}
	}
	return false
}
