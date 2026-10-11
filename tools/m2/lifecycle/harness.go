// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"bytes"
	"context"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/json"
	"encoding/pem"
	"errors"
	"fmt"
	"io"
	"math/big"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/deployment/m2"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	archivefixture "github.com/zyc14588/TRPG_PLATFORM/internal/package/testdata/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
	smoke "github.com/zyc14588/TRPG_PLATFORM/tests/smoke/m2"
)

const postgresImage = "sha256:3a82e1f56c8f0f5616a11103ac3d47e632c3938698946a7ad26da0df1334744a"

type receipt struct {
	Name           string
	Argv           []string
	Exit           int
	Bytes          int
	SHA256         string
	DurationMillis int64
}
type phase struct {
	Name         string
	Passed       bool
	Facts        any
	FailureStage string `json:",omitempty"`
}
type report struct {
	Candidate, Tree, Source, Project, Mode, Verdict string
	Phases                                          []phase
	Commands                                        []receipt
	Cleanup                                         bool
	RRQualification                                 string
	Error                                           string `json:",omitempty"`
}
type lifecycle struct {
	registry                                                         map[string]any
	markers                                                          [][]byte
	root, temp, out, project, compose, image, network, source        string
	env                                                              []string
	report                                                           report
	files                                                            map[string]m2.TLSFiles
	config                                                           m2.Config
	plan                                                             m2.OperatorPlan
	provider                                                         *smoke.Provider
	browser                                                          *smoke.Browser
	seq                                                              int
	networkOwned, imageOwned, composeOwned, edgeOwned, providerOwned bool
	sentinel, sentinelSnapshot                                       string
	healthProbe                                                      string
}

func runLifecycle(ctx context.Context, requested, project, output string, development bool) (result error) {
	if m2.RequireLinux() != nil || os.Geteuid() == 0 || !regexp.MustCompile(`^trpg-m2-[a-z0-9-]{8,48}$`).MatchString(project) || !filepath.IsAbs(output) {
		return m2.ErrConfiguration
	}
	root, e := os.Getwd()
	if e != nil {
		return e
	}
	root, e = filepath.Abs(root)
	if e != nil {
		return e
	}
	h := &lifecycle{root: root, project: project, out: output, compose: filepath.Join(root, "deploy/m2/compose.yaml"), network: project + "-private"}
	if e = os.MkdirAll(output, 0700); e != nil {
		return e
	}
	if e = os.Chmod(output, 0700); e != nil {
		return e
	}
	h.report = report{Project: project, Mode: "FORMAL_REQUIRED_VERIFICATION", Verdict: "FAIL", RRQualification: "NOT_RUN; NOT_A_B011_HARD_PREREQUISITE"}
	if development {
		h.report.Mode = "DEVELOPMENT_NOT_FORMAL_ACCEPTANCE"
	}
	source, e := h.command(ctx, "source-commit", nil, "git", "rev-parse", requested+"^{commit}")
	if e != nil {
		return e
	}
	h.report.Candidate = strings.TrimSpace(string(source))
	head, e := h.command(ctx, "source-head", nil, "git", "rev-parse", "HEAD")
	if e != nil || strings.TrimSpace(string(head)) != h.report.Candidate {
		return m2.ErrConfiguration
	}
	tree, e := h.command(ctx, "source-tree", nil, "git", "rev-parse", "HEAD^{tree}")
	if e != nil {
		return e
	}
	h.report.Tree = strings.TrimSpace(string(tree))
	status, e := h.command(ctx, "source-status", nil, "git", "status", "--porcelain")
	if e != nil || (!development && len(status) != 0) {
		return m2.ErrConfiguration
	}
	if !development {
		if _, e = h.command(ctx, "source-signature", nil, "git", "verify-commit", h.report.Candidate); e != nil {
			return m2.ErrConfiguration
		}
	}
	files, e := h.command(ctx, "source-files", nil, "git", "ls-files", "--cached", "--others", "--exclude-standard", "-z")
	if e != nil {
		return e
	}
	manifest := []map[string]string{}
	for _, name := range strings.Split(string(files), "\x00") {
		if name == "" {
			continue
		}
		b, e := os.ReadFile(filepath.Join(root, name))
		if os.IsNotExist(e) {
			manifest = append(manifest, map[string]string{"path": name, "state": "deleted"})
			continue
		}
		if e != nil {
			return e
		}
		manifest = append(manifest, map[string]string{"path": name, "sha256": object.Hash(b)})
	}
	sourceManifest := map[string]any{"commit": h.report.Candidate, "tree": h.report.Tree, "files": manifest}
	manifestBytes, e := json.Marshal(sourceManifest)
	if e != nil {
		return e
	}
	h.source = object.Hash(manifestBytes)
	h.report.Source = h.source
	if e = atomicJSON(filepath.Join(output, "source-manifest.json"), sourceManifest); e != nil {
		return e
	}
	h.temp, e = os.MkdirTemp("", "trpg-m2-owned-")
	if e != nil {
		return e
	}
	registry := map[string]any{"owner": "M2-B011 lifecycle", "project": project, "source": h.source, "local_root": h.temp, "network": h.network, "edge_network": h.project + "-edge", "provider_network": h.project + "-provider", "image_tag": "trpg-platform/m2:" + project, "unrelated_sentinel_name": project + "-sentinel", "compose": h.compose, "volumes": []string{project + "_postgres", project + "_objects", project + "_supervisor-socket", project + "_worker-socket", project + "_object-socket"}}
	h.registry = registry
	h.markers = [][]byte{[]byte(smoke.PrivateValue), []byte(smoke.ProviderKey), []byte(smoke.RegistrationToken), []byte("m2-owned-synthetic-password"), []byte("m2-player-synthetic-password"), []byte("owned-synthetic-sentinel-password")}
	if e = atomicJSON(filepath.Join(output, "resources.json"), registry); e != nil {
		return e
	}
	defer func() {
		cleanup, done := context.WithTimeout(context.Background(), 45*time.Second)
		defer done()
		if result != nil && h.composeOwned {
			_, _ = h.composeCommand(cleanup, "logs", "--no-color", "--tail", "40")
			_, _ = h.composeCommand(cleanup, "ps", "--all", "--format", "json")
		}
		result = errors.Join(result, h.cleanup(cleanup))
		privacy := h.auditOrdinaryOutputs()
		h.report.Phases = append(h.report.Phases, phase{Name: "ordinary-output-privacy", Passed: privacy == nil, Facts: map[string]any{"marker_count": len(h.markers), "prompt_and_key_bytes_saved": false}})
		result = errors.Join(result, privacy)
		if result == nil && h.report.Cleanup {
			h.report.Verdict = "PASS"
			if development {
				h.report.Verdict = "DEVELOPMENT_CHECKS_PASSED_NOT_ACCEPTED"
			}
		} else {
			h.report.Error = "source, runtime, phase, privacy or owned cleanup failed"
		}
		result = errors.Join(result, atomicJSON(filepath.Join(output, "report.json"), h.report))
		for _, marker := range h.markers {
			clear(marker)
		}
		fmt.Printf("M2_LIFECYCLE %s candidate=%s project=%s report=%s\n", h.report.Verdict, h.report.Candidate, project, filepath.Join(output, "report.json"))
	}()
	if e = h.phase("owned-network", func() (any, error) {
		info, e := h.command(ctx, "docker-info", nil, "docker", "info", "--format", "{{json .SecurityOptions}}")
		if e != nil || bytes.Contains(info, []byte("rootless")) {
			return nil, m2.ErrConfiguration
		}
		if _, e = h.command(ctx, "network-preexist", nil, "docker", "network", "inspect", h.network); e == nil {
			return nil, m2.ErrPrivate
		}
		for _, kind := range []string{"container", "network", "volume", "image"} {
			args := []string{"docker", kind, "ls", "-q", "--filter", "label=trpg.m2.project=" + project}
			if kind == "container" || kind == "image" {
				args = append(args, "--all")
			}
			raw, e := h.command(ctx, "project-preexist", nil, args...)
			if e != nil || len(bytes.TrimSpace(raw)) != 0 {
				return nil, m2.ErrPrivate
			}
		}
		existing, e := h.command(ctx, "compose-preexist", nil, "docker", "ps", "--all", "--quiet", "--filter", "label=com.docker.compose.project="+project)
		if e != nil || len(bytes.TrimSpace(existing)) != 0 {
			return nil, m2.ErrPrivate
		}
		_, e = h.command(ctx, "network-create", nil, "docker", "network", "create", "--internal", "--label", "trpg.m2.project="+project, "--label", "trpg.m2.source="+h.source, h.network)
		if e != nil {
			return nil, e
		}
		h.networkOwned = true
		_, e = h.command(ctx, "edge-network-create", nil, "docker", "network", "create", "--label", "trpg.m2.project="+project, "--label", "trpg.m2.source="+h.source, project+"-edge")
		if e != nil {
			return nil, e
		}
		h.edgeOwned = true
		_, e = h.command(ctx, "provider-network-create", nil, "docker", "network", "create", "--internal", "--label", "trpg.m2.project="+project, "--label", "trpg.m2.source="+h.source, project+"-provider")
		if e != nil {
			return nil, e
		}
		h.providerOwned = true
		raw, e := h.command(ctx, "network-gateway", nil, "docker", "network", "inspect", project+"-provider", "--format", "{{(index .IPAM.Config 0).Gateway}}")
		if e != nil {
			return nil, e
		}
		gateway := strings.TrimSpace(string(raw))
		listener, e := net.Listen("tcp", net.JoinHostPort(gateway, "0"))
		if e != nil {
			return nil, e
		}
		if e = h.makePKI(net.ParseIP(gateway)); e != nil {
			listener.Close()
			return nil, e
		}
		providerTLS, e := m2.ExternalTLS(h.files["provider"])
		if e != nil {
			listener.Close()
			return nil, e
		}
		h.provider, e = smoke.NewProvider(listener, providerTLS)
		if e != nil {
			listener.Close()
			return nil, e
		}
		return map[string]any{"network": h.network, "edge_network": h.project + "-edge", "provider_network": project + "-provider", "internal_provider": true, "provider": "TEST_ONLY_BOUND_COMPATIBLE_LOCAL_HTTPS_PROVIDER", "public_provider_calls": "NOT_RUN"}, nil
	}); e != nil {
		return e
	}
	if e = h.phase("unrelated-real-postgres-sentinel", func() (any, error) { return h.startSentinel(ctx) }); e != nil {
		return e
	}
	if e = h.phase("source-build", func() (any, error) { return h.build(ctx) }); e != nil {
		return e
	}
	if e = h.phase("compose-config", func() (any, error) {
		raw, e := h.composeCommand(ctx, "config", "--format", "json")
		if e != nil {
			return nil, e
		}
		return smoke.VerifyTopology(raw, h.source, h.image)
	}); e != nil {
		return e
	}
	if e = h.phase("six-service-start", func() (any, error) {
		h.composeOwned = true
		if _, e := h.composeCommand(ctx, "up", "--detach"); e != nil {
			return nil, e
		}
		inventory, e := h.waitHealthy(ctx)
		if e != nil {
			return nil, e
		}
		ids, e := h.composeCommand(ctx, "ps", "--all", "--quiet")
		if e != nil {
			return nil, e
		}
		args := append([]string{"docker", "inspect"}, strings.Fields(string(ids))...)
		raw, e := h.command(ctx, "actual-container-isolation", nil, args...)
		if e != nil {
			return nil, e
		}
		isolation, e := smoke.VerifyRuntimeInspect(raw, h.source, h.image, h.project, os.Geteuid())
		if e != nil {
			return nil, e
		}
		networks, e := h.actualNetworks(ctx)
		if e != nil {
			return nil, e
		}
		return map[string]any{"services": inventory, "networks": networks, "actual_isolation": isolation}, nil
	}); e != nil {
		return e
	}
	if e = h.phase("actual-daemon-liveness-and-readiness", func() (any, error) { return h.daemonHealthFaults(ctx) }); e != nil {
		return e
	}
	browser, e := smoke.NewBrowser(h.config.Origin, h.files["reverse-proxy"].CA)
	if e != nil {
		return e
	}
	h.browser = browser
	defer browser.Close()
	if e = h.phase("actual-https-owner-and-workspace", func() (any, error) {
		account, e := browser.Login(ctx, "m2_owner", "m2-owned-synthetic-password")
		if e != nil {
			return nil, e
		}
		workspace, e := browser.CreateWorkspace(ctx, "Owned B011 deployment")
		if e != nil {
			return nil, e
		}
		second, e := browser.CreateWorkspace(ctx, "Owned B011 separate tenant")
		if e != nil {
			return nil, e
		}
		h.plan, e = h.planForWorkspace(workspace)
		if e != nil {
			return nil, e
		}

		other, e := h.planForWorkspace(second)
		if e != nil {
			return nil, e
		}
		other.Games[0].Operator = "explicit-test-second-operator"
		other.Games[0].InstallCredentialFile = "/run/operator/install-credential-second"
		if e = writePrivate(filepath.Join(h.temp, "operator/platformd/install-credential-second"), []byte("m2-b011-owned-second-install-credential")); e != nil {
			return nil, e
		}
		h.markers = append(h.markers, []byte("m2-b011-owned-second-install-credential"))
		h.plan.Games = append(h.plan.Games, other.Games...)
		h.plan.Certificates = append(h.plan.Certificates, other.Certificates...)
		for k, v := range other.Defaults {
			h.plan.Defaults[k] = v
		}
		for k, v := range other.Labels {
			h.plan.Labels[k] = v
		}
		for k, v := range other.WorkspaceLimits {
			h.plan.WorkspaceLimits[k] = v
		}
		for k, v := range other.Caps {
			h.plan.Caps[k] = v
		}
		if _, e = h.composeCommand(ctx, "stop", "--timeout", "8", "platformd", "workerd", "reverse-proxy"); e != nil {
			return nil, e
		}
		if h.plan.Classification != "TEST_ONLY_STANDARD_CARRIER" {
			return nil, m2.ErrConfiguration
		}
		if e = writePrivate(filepath.Join(h.temp, "operator/workerd/test-only-classification"), []byte(h.plan.Classification)); e != nil {
			return nil, e
		}
		if e = h.writePlan(); e != nil {
			return nil, e
		}
		if _, e = h.composeCommand(ctx, "up", "--detach", "platformd", "workerd", "reverse-proxy"); e != nil {
			return nil, e
		}
		if _, e = h.waitHealthy(ctx); e != nil {
			return nil, e
		}

		graph, e := smoke.VerifyInstalledTenants(ctx, h.plan, h.composeCommand)
		if e != nil {
			return nil, e
		}
		return map[string]any{"owner_authenticated": account != "", "two_workspaces_created_via_existing_api": true, "installed_root_and_dependency": graph}, nil
	}); e != nil {
		return e
	}
	var lobby smoke.Lobby
	if e = h.phase("mixed-seat-explicit-owner-action", func() (any, error) {
		var e error
		lobby, e = browser.PrepareMixedLobby(ctx, h.plan.Games[0].Workspace, h.plan.Games[0].Configuration, h.plan.Games[0].Game, smoke.RegistrationToken)
		if e != nil {
			return nil, e
		}
		if e = h.ownerAction(ctx, lobby); e != nil {
			return nil, e
		}
		return map[string]any{"real_current_account_session_and_csrf": true, "explicit_one_time_opt_in": true, "normal_restart_no_model_writes": true}, nil
	}); e != nil {
		return e
	}
	if e = h.phase("actual-deployment-smoke", func() (any, error) {
		facts, e := smoke.RunDeployment(ctx, browser, &lobby, h.provider, h.plan)
		if e != nil {
			diagnostic, qe := h.composeCommand(ctx, "exec", "--no-TTY", "postgres", "psql", "--no-psqlrc", "--username=trpg_operator", "--dbname=trpg_m2", "--tuples-only", "--no-align", "--command", `SELECT status,attempt,(convert_from(body,'UTF8')::jsonb->'metadata'->>'origin_principal'='task-system') AS internal_origin FROM platform_task.jobs ORDER BY task_id`)
			guards, ge := smoke.CommandGuardMetadata(ctx, lobby, h.composeCommand)
			_, _ = h.composeCommand(ctx, "logs", "--no-color", "--tail", "15", "platformd")
			facts = map[string]any{"phase_facts": facts, "provider": h.provider.Metadata(), "bounded_task_metadata": strings.TrimSpace(string(diagnostic)), "query_succeeded": qe == nil, "underlying_command_guard_rows": guards, "guard_query_succeeded": ge == nil}
		}
		return facts, e
	}); e != nil {
		return e
	}
	if e = h.phase("actual-object-three-consumers", func() (any, error) {
		return smoke.RunObjectFaults(ctx, h.provider, h.plan, filepath.Join(h.temp, "data/objects"), h.composeCommand, h.waitHealthy, h.objectProbe)
	}); e != nil {
		return e
	}
	if e = h.phase("kernel-lifecycle-faults", func() (any, error) { return h.kernelFaults(ctx) }); e != nil {
		return e
	}
	if e = h.phase("durable-restart-and-process-faults", func() (any, error) { return h.restartAndFaults(ctx, lobby) }); e != nil {
		return e
	}
	return nil
}
func (h *lifecycle) phase(name string, run func() (any, error)) error {
	facts, e := run()
	p := phase{Name: name, Passed: e == nil, Facts: facts}
	var stage interface{ CheckStage() string }
	if errors.As(e, &stage) {
		p.FailureStage = stage.CheckStage()
	}
	h.report.Phases = append(h.report.Phases, p)
	_ = atomicJSON(filepath.Join(h.out, "report.json"), h.report)
	return e
}
func (h *lifecycle) command(ctx context.Context, name string, input []byte, args ...string) ([]byte, error) {
	h.seq++
	cmd := exec.CommandContext(ctx, args[0], args[1:]...)
	cmd.Dir = h.root
	cmd.Env = h.env
	if h.env == nil {
		cmd.Env = os.Environ()
	}
	if input != nil {
		cmd.Stdin = bytes.NewReader(input)
	}
	var out bytes.Buffer
	cmd.Stdout = &out
	cmd.Stderr = &out
	start := time.Now()
	e := cmd.Run()
	b := out.Bytes()
	code := 0
	if e != nil {
		code = -1
		var x *exec.ExitError
		if errors.As(e, &x) {
			code = x.ExitCode()
		}
	}
	h.report.Commands = append(h.report.Commands, receipt{name, args, code, len(b), object.Hash(b), time.Since(start).Milliseconds()})
	if len(b) > 2<<20 {
		return nil, m2.ErrPrivate
	}
	// All subprocess inputs are fixed metadata; secrets/prompt bytes never enter
	// arguments or these ordinary build/runtime logs.
	if e2 := os.WriteFile(filepath.Join(h.out, fmt.Sprintf("command-%03d-%s.log", h.seq, name)), b, 0600); e2 != nil {
		return nil, e2
	}
	return bytes.Clone(b), e
}
func (h *lifecycle) composeCommand(ctx context.Context, args ...string) ([]byte, error) {
	argv := append([]string{"docker", "compose", "--project-name", h.project, "-f", h.compose}, args...)
	return h.command(ctx, "compose", nil, argv...)
}
func atomicJSON(path string, v any) error {
	b, e := json.MarshalIndent(v, "", "  ")
	if e != nil || len(b) > 16<<20 {
		return m2.ErrPrivate
	}
	b = append(b, '\n')
	tmp := path + ".new"
	if e = os.WriteFile(tmp, b, 0600); e != nil {
		return e
	}
	return os.Rename(tmp, path)
}
func writePrivate(path string, b []byte) error {
	if e := os.MkdirAll(filepath.Dir(path), 0700); e != nil {
		return e
	}
	if e := os.Remove(path); e != nil && !errors.Is(e, os.ErrNotExist) {
		return e
	}
	return os.WriteFile(path, b, 0400)
}
func (h *lifecycle) build(ctx context.Context) (any, error) {
	rootfs := filepath.Join(h.temp, "rootfs")
	for _, d := range []string{"app", "etc/ssl/certs"} {
		if e := os.MkdirAll(filepath.Join(rootfs, d), 0755); e != nil {
			return nil, e
		}
	}
	for _, d := range []string{"data/postgres", "data/objects", "run/supervisor", "run/object", "run/worker"} {
		if e := os.MkdirAll(filepath.Join(h.temp, d), 0700); e != nil {
			return nil, e
		}
	}
	hashes := map[string]string{}
	for _, name := range []string{"platformd", "workerd", "lua-supervisord", "lua-runner", "objectd", "reverse-proxyd"} {
		path := filepath.Join(rootfs, "app", name)
		oldEnv := h.env
		h.env = append(os.Environ(), "CGO_ENABLED=0", "GOOS=linux", "GOARCH=amd64")
		_, e := h.command(ctx, "build-"+name, nil, "go", "build", "-trimpath", "-ldflags=-s -w -buildid=", "-o", path, "./cmd/"+name)
		h.env = oldEnv
		if e != nil {
			return nil, e
		}
		if e = os.Chmod(path, 0555); e != nil {
			return nil, e
		}
		b, e := os.ReadFile(path)
		if e != nil {
			return nil, e
		}
		hashes[name] = object.Hash(b)
	}
	// This same-source helper exists only in this owned TEST_ONLY image. It is
	// synchronously exec'd in platformd; it is never a seventh service or a
	// production deployment endpoint.
	probePath := filepath.Join(rootfs, "app/m2-object-probe")
	oldEnv := h.env
	h.env = append(os.Environ(), "CGO_ENABLED=0", "GOOS=linux", "GOARCH=amd64")
	_, probeError := h.command(ctx, "build-private-object-probe", nil, "go", "build", "-trimpath", "-ldflags=-s -w -buildid=", "-o", probePath, "./tools/m2/lifecycle")
	h.env = oldEnv
	if probeError != nil {
		return nil, probeError
	}
	if e := os.Chmod(probePath, 0555); e != nil {
		return nil, e
	}
	probeBytes, e := os.ReadFile(probePath)
	if e != nil {
		return nil, e
	}
	h.registry["private_object_probe_sha256"] = object.Hash(probeBytes)
	if _, e := h.command(ctx, "web-player-build", nil, "pnpm", "--filter", "@trpg-platform/web-player", "build"); e != nil {
		return nil, e
	}
	if e := copyTree(filepath.Join(h.root, "apps/web-player/dist"), filepath.Join(rootfs, "app/web-player")); e != nil {
		return nil, e
	}
	if e := copyTree(filepath.Join(h.root, "schemas"), filepath.Join(rootfs, "app/schemas")); e != nil {
		return nil, e
	}
	if b, e := os.ReadFile("/etc/ssl/certs/ca-certificates.crt"); e == nil {
		fixtureCA, e := os.ReadFile(h.files["provider"].CA)
		if e != nil {
			return nil, e
		}
		b = append(b, fixtureCA...)
		if e = os.WriteFile(filepath.Join(rootfs, "etc/ssl/certs/ca-certificates.crt"), b, 0444); e != nil {
			return nil, e
		}
	} else {
		return nil, e
	}
	listener, e := net.Listen("tcp", "127.0.0.1:0")
	if e != nil {
		return nil, e
	}
	port := listener.Addr().(*net.TCPAddr).Port
	listener.Close()
	h.config = m2.Config{SPDXIdentifier: "PolyForm-Noncommercial-1.0.0", Version: 1, DeploymentID: h.project, Source: h.source, Origin: "https://localhost:" + strconv.Itoa(port), DaemonAddress: "platformd:8443", Upstream: "https://platformd:8443", ExternalAddress: ":8443", StaticRoot: "/app/web-player", SupervisorSocket: "/run/m2/supervisor/service.sock", WorkerSocket: "/run/m2/worker/service.sock", ObjectSocket: "/run/m2/object/service.sock", PeerUID: uint32(os.Geteuid()), Runner: "/app/lua-runner", RunnerHash: hashes["lua-runner"], Limits: profile.DefaultLimits(), ObjectRoot: "/var/lib/trpg/objects", StagingRoot: "/run/staging", DSNFile: "/run/operator/dsn", CookieKeyFile: "/run/operator/cookie-key", ReplayKeyFile: "/run/operator/replay-key", InvitationKeyFile: "/run/operator/invitation-key", VaultKeyFile: "/run/operator/vault-key", SeedFile: "/run/operator/seed.json", GrantsFile: "/run/operator/grants.json", PackagesFile: "/run/operator/packages.json", Provider: m2.ProviderConfig{Endpoint: model.EndpointData{ID: "owned-offline-provider", URL: h.provider.URL() + "/v1", Adapter: "openai-compatible", Models: []string{"fixture:small"}, AllowLANHTTP: true}, TimeoutMillis: 1000, ResponseBytes: 65536, MaxActive: 4, MicrosPerToken: 2}}
	for _, role := range []string{"platformd", "workerd", "lua-runner", "object-storage", "reverse-proxy"} {
		c := h.config
		c.TLS = m2.TLSFiles{CA: "/run/operator/ca.pem", Certificate: "/run/operator/service.pem", Key: "/run/operator/service.key"}
		type plain m2.Config
		b, e := json.Marshal(plain(c))
		if e != nil {
			return nil, e
		}
		if e = writePrivate(filepath.Join(h.temp, "operator", role, "config.json"), b); e != nil {
			return nil, e
		}
	}
	for _, name := range []string{"cookie-key", "replay-key", "invitation-key", "vault-key", "task-credential"} {
		b := make([]byte, 32)
		if _, e = rand.Read(b); e != nil {
			return nil, e
		}
		if e = writePrivate(filepath.Join(h.temp, "operator/platformd", name), b); e != nil {
			return nil, e
		}
		h.markers = append(h.markers, append([]byte(nil), b...))
		clear(b)
	}
	password := "m2-synthetic-" + h.project
	h.markers = append(h.markers, []byte(password))
	if e = writePrivate(filepath.Join(h.temp, "operator/postgres/password"), []byte(password)); e != nil {
		return nil, e
	}
	if e = writePrivate(filepath.Join(h.temp, "operator/platformd/dsn"), []byte("postgres://trpg_operator:"+password+"@postgres:5432/trpg_m2?sslmode=disable")); e != nil {
		return nil, e
	}
	if e = writePrivate(filepath.Join(h.temp, "operator/platformd/install-credential"), []byte("m2-b011-owned-install-credential")); e != nil {
		return nil, e
	}
	seed, _ := json.Marshal(map[string]string{"login_name": "m2_owner", "password": "m2-owned-synthetic-password", "display_name": "Owned test host"})
	if e = writePrivate(filepath.Join(h.temp, "operator/platformd/seed.json"), seed); e != nil {
		return nil, e
	}
	grants, _ := json.Marshal([]map[string]string{{"token": smoke.RegistrationToken, "expires_at": time.Now().Add(time.Hour).UTC().Format(time.RFC3339Nano)}})
	if e = writePrivate(filepath.Join(h.temp, "operator/platformd/grants.json"), grants); e != nil {
		return nil, e
	}
	h.plan, e = h.planForWorkspace("m2-initial-fixture")
	if e != nil {
		return nil, e
	}
	if e = h.writePlan(); e != nil {
		return nil, e
	}
	tag := "trpg-platform/m2:" + h.project
	existing, e := h.command(ctx, "image-preexist", nil, "docker", "image", "ls", "--quiet", "--filter", "reference="+tag)
	if e != nil || len(bytes.TrimSpace(existing)) != 0 {
		return nil, m2.ErrPrivate
	}
	h.imageOwned = true
	if _, e = h.command(ctx, "image-build", nil, "docker", "build", "--network=none", "--target", "m2", "--build-arg", "M2_UID="+strconv.Itoa(os.Geteuid()), "--build-arg", "M2_GID="+strconv.Itoa(os.Getegid()), "--build-arg", "M2_PROJECT="+h.project, "--build-arg", "M2_SOURCE="+h.source, "-f", filepath.Join(h.root, "deploy/docker/Dockerfile"), "-t", tag, h.temp); e != nil {
		return nil, e
	}
	raw, e := h.command(ctx, "image-id", nil, "docker", "image", "inspect", tag, "--format", "{{.Id}}")
	if e != nil {
		return nil, e
	}
	h.image = strings.TrimSpace(string(raw))
	h.registry["image_id"] = h.image
	if e = atomicJSON(filepath.Join(h.out, "resources.json"), h.registry); e != nil {
		return nil, e
	}
	h.env = append(os.Environ(), "TRPG_M2_PROJECT="+h.project, "TRPG_M2_IMAGE="+h.image, "TRPG_M2_SOURCE="+h.source, "TRPG_M2_ROOT="+h.temp, "TRPG_M2_NETWORK="+h.network, "TRPG_M2_EDGE_NETWORK="+h.project+"-edge", "TRPG_M2_PROVIDER_NETWORK="+h.project+"-provider", "TRPG_M2_PORT="+strconv.Itoa(port), "TRPG_M2_UID="+strconv.Itoa(os.Geteuid()), "TRPG_M2_GID="+strconv.Itoa(os.Getegid()))
	return map[string]any{"binaries": hashes, "image": h.image, "source": h.source, "static_build_unchanged": true}, nil
}
func copyTree(source, target string) error {
	return filepath.WalkDir(source, func(path string, d os.DirEntry, e error) error {
		if e != nil {
			return e
		}
		rel, e := filepath.Rel(source, path)
		if e != nil {
			return e
		}
		to := filepath.Join(target, rel)
		if d.IsDir() {
			return os.MkdirAll(to, 0755)
		}
		if !d.Type().IsRegular() {
			return m2.ErrConfiguration
		}
		b, e := os.ReadFile(path)
		if e != nil {
			return e
		}
		return os.WriteFile(to, b, 0444)
	})
}
func (h *lifecycle) planForWorkspace(workspace string) (m2.OperatorPlan, error) {
	pkg, dependency, policy, e := smoke.BuildCarrierWithDependency(install.RuntimeConfig{Runner: h.config.Runner, SHA256: h.config.RunnerHash, Limits: h.config.Limits})
	if e != nil {
		return m2.OperatorPlan{}, e
	}
	b, e := archivefixture.Archive(pkg)
	if e != nil {
		return m2.OperatorPlan{}, e
	}
	if e = writePrivate(filepath.Join(h.temp, "operator/platformd/carrier.zip"), b); e != nil {
		return m2.OperatorPlan{}, e
	}
	depBytes, e := archivefixture.Archive(dependency)
	if e != nil {
		return m2.OperatorPlan{}, e
	}
	if e = writePrivate(filepath.Join(h.temp, "operator/platformd/dependency.zip"), depBytes); e != nil {
		return m2.OperatorPlan{}, e
	}
	plan, e := smoke.CarrierPlan(pkg, policy, workspace, h.config.Provider.Endpoint.URL)
	if e != nil {
		return m2.OperatorPlan{}, e
	}
	e = smoke.AttachCarrierDependency(&plan, dependency)
	return plan, e
}
func (h *lifecycle) writePlan() error {
	b, e := json.Marshal(h.plan)
	if e != nil {
		return e
	}
	return writePrivate(filepath.Join(h.temp, "operator/platformd/packages.json"), b)
}
func (h *lifecycle) makePKI(providerIP net.IP) error {
	key, e := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if e != nil {
		return e
	}
	now := time.Now()
	ca := &x509.Certificate{SerialNumber: big.NewInt(1), Subject: pkix.Name{CommonName: h.project}, NotBefore: now.Add(-time.Minute), NotAfter: now.Add(time.Hour), IsCA: true, BasicConstraintsValid: true, KeyUsage: x509.KeyUsageCertSign | x509.KeyUsageDigitalSignature}
	der, e := x509.CreateCertificate(rand.Reader, ca, ca, &key.PublicKey, key)
	if e != nil {
		return e
	}
	h.files = map[string]m2.TLSFiles{}
	for i, role := range []string{"platformd", "workerd", "lua-runner", "object-storage", "reverse-proxy", "provider"} {
		k, e := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
		if e != nil {
			return e
		}
		names := []string{role}
		if role == "reverse-proxy" {
			names = append(names, "localhost")
		}
		c := &x509.Certificate{SerialNumber: big.NewInt(int64(i + 2)), DNSNames: names, NotBefore: now.Add(-time.Minute), NotAfter: now.Add(time.Hour), KeyUsage: x509.KeyUsageDigitalSignature, ExtKeyUsage: []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth, x509.ExtKeyUsageClientAuth}}
		if role == "provider" {
			c.IPAddresses = []net.IP{providerIP}
		}
		b, e := x509.CreateCertificate(rand.Reader, c, ca, &k.PublicKey, key)
		if e != nil {
			return e
		}
		kb, e := x509.MarshalPKCS8PrivateKey(k)
		if e != nil {
			return e
		}
		h.markers = append(h.markers, pem.EncodeToMemory(&pem.Block{Type: "PRIVATE KEY", Bytes: kb}))
		dir := filepath.Join(h.temp, "operator", role)
		f := m2.TLSFiles{CA: filepath.Join(dir, "ca.pem"), Certificate: filepath.Join(dir, "service.pem"), Key: filepath.Join(dir, "service.key")}
		for path, value := range map[string][]byte{f.CA: pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: der}), f.Certificate: pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: b}), f.Key: pem.EncodeToMemory(&pem.Block{Type: "PRIVATE KEY", Bytes: kb})} {
			if e = writePrivate(path, value); e != nil {
				return e
			}
		}
		h.files[role] = f
	}
	return nil
}
func (h *lifecycle) waitHealthy(ctx context.Context) (any, error) {
	deadline := time.Now().Add(75 * time.Second)
	for time.Now().Before(deadline) {
		raw, e := h.composeCommand(ctx, "ps", "--all", "--format", "json")
		if e != nil {
			return nil, e
		}
		inventory, ready, e := smoke.ServiceInventory(raw)
		if e != nil {
			return nil, e
		}
		if ready {
			h.registry["containers"] = inventory
			if e = atomicJSON(filepath.Join(h.out, "resources.json"), h.registry); e != nil {
				return nil, e
			}
			// Docker health means each process is alive. Dependency/source
			// readiness remains a separate authenticated platform check.
			if status, e := h.daemonReadiness(ctx); e == nil && status == http.StatusOK {
				return inventory, nil
			}
		}
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-time.After(250 * time.Millisecond):
		}
	}
	_, _ = h.composeCommand(ctx, "logs", "--no-color", "--tail", "40")
	return nil, m2.ErrPrivate
}

type daemonBinding struct{ ID, Address string }

func (h *lifecycle) bindDaemon(ctx context.Context, role string) (daemonBinding, error) {
	raw, e := h.composeCommand(ctx, "ps", "--all", "--quiet", role)
	id := strings.TrimSpace(string(raw))
	if e != nil || !regexp.MustCompile(`^[a-f0-9]{64}$`).MatchString(id) {
		return daemonBinding{}, m2.ErrPrivate
	}
	raw, e = h.command(ctx, "health-owned-daemon-binding", nil, "docker", "inspect", id)
	var v []struct {
		ID, Image       string
		Config          struct{ Labels map[string]string }
		State           struct{ Running bool }
		NetworkSettings struct {
			Networks map[string]struct{ IPAddress string }
		}
	}
	if e != nil || json.Unmarshal(raw, &v) != nil || len(v) != 1 || v[0].ID != id || v[0].Image != h.image || !v[0].State.Running || v[0].Config.Labels["trpg.m2.project"] != h.project || v[0].Config.Labels["trpg.m2.source"] != h.source || v[0].Config.Labels["com.docker.compose.service"] != role {
		return daemonBinding{}, m2.ErrPrivate
	}
	ip := v[0].NetworkSettings.Networks[h.network].IPAddress
	if net.ParseIP(ip) == nil {
		return daemonBinding{}, m2.ErrPrivate
	}
	return daemonBinding{id, net.JoinHostPort(ip, "8443")}, nil
}

func (h *lifecycle) daemonReadiness(ctx context.Context) (int, error) {
	bound, e := h.bindDaemon(ctx, "platformd")
	if e != nil {
		return 0, e
	}
	tlsConfig, e := m2.TLSConfig(h.files["platformd"], "platformd", false)
	if e != nil {
		return 0, e
	}
	client := &http.Client{Transport: &http.Transport{Proxy: nil, TLSClientConfig: tlsConfig, MaxResponseHeaderBytes: 16384}, Timeout: 2 * time.Second, CheckRedirect: func(*http.Request, []*http.Request) error { return m2.ErrPrivate }}
	defer client.CloseIdleConnections()
	r, e := http.NewRequestWithContext(ctx, "GET", "https://"+bound.Address+"/health/ready", nil)
	if e != nil {
		return 0, e
	}
	reply, e := client.Do(r)
	if e != nil {
		return 0, e
	}
	defer reply.Body.Close()
	body, e := io.ReadAll(io.LimitReader(reply.Body, 64))
	if e != nil || len(body) >= 64 || (reply.StatusCode != http.StatusOK && reply.StatusCode != http.StatusServiceUnavailable) {
		return 0, m2.ErrPrivate
	}
	h.seq++
	if e = atomicJSON(filepath.Join(h.out, fmt.Sprintf("readiness-%03d-status-%d.json", h.seq, reply.StatusCode)), map[string]any{"container": bound.ID, "status": reply.StatusCode, "authenticated_peer": "platformd", "source": h.source, "body_sha256": object.Hash(body)}); e != nil {
		return 0, e
	}
	return reply.StatusCode, nil
}

func (h *lifecycle) healthDomainDigest(ctx context.Context) (string, error) {
	queries := []string{}
	for _, table := range []string{"host_command.sessions", "host_command.requests", "host_command.events", "host_command.tasks", "host_command.continuations", "platform_task.jobs", "platform_budget.tasks", "platform_budget.reservations", "platform_budget.counters", "platform_budget.pauses", "platform_model.configurations"} {
		queries = append(queries, "SELECT '"+table+"' AS n,md5(coalesce(jsonb_agg(j ORDER BY j::text),'[]'::jsonb)::text) AS d FROM (SELECT to_jsonb(t) AS j FROM "+table+" t) v")
	}
	raw, e := h.composeCommand(ctx, "exec", "--no-TTY", "postgres", "psql", "--no-psqlrc", "--username=trpg_operator", "--dbname=trpg_m2", "--tuples-only", "--no-align", "--command", "SELECT md5(string_agg(d,'' ORDER BY n)) FROM ("+strings.Join(queries, " UNION ALL ")+") proofs")
	digest := strings.TrimSpace(string(raw))
	if e != nil || !regexp.MustCompile(`^[a-f0-9]{32}$`).MatchString(digest) {
		return "", m2.ErrPrivate
	}
	return digest, nil
}

func (h *lifecycle) daemonHealthFaults(ctx context.Context) (any, error) {
	facts := map[string]any{}
	proxy, e := h.bindDaemon(ctx, "reverse-proxy")
	if e != nil {
		return facts, e
	}
	platform, e := h.bindDaemon(ctx, "platformd")
	if e != nil {
		return facts, e
	}
	objects, e := h.bindDaemon(ctx, "object-storage")
	if e != nil {
		return facts, e
	}
	before, e := h.healthDomainDigest(ctx)
	if e != nil {
		return facts, e
	}
	calls := h.provider.Calls()
	probeLive := func() error {
		for _, v := range []struct{ id, binary, mode string }{{proxy.ID, "/app/reverse-proxyd", "--mode=health"}, {platform.ID, "/app/platformd", "m2-health"}} {
			if _, e := h.command(ctx, "actual-daemon-health-command", nil, "docker", "exec", v.id, v.binary, v.mode, "--config=/run/operator/config.json"); e != nil {
				return e
			}
		}
		return nil
	}
	if status, e := h.daemonReadiness(ctx); e != nil || status != http.StatusOK {
		return facts, m2.ErrPrivate
	}
	if e = probeLive(); e != nil {
		return facts, e
	}
	facts["baseline_real_health_commands_and_authenticated_readiness"] = true
	if _, e = h.command(ctx, "health-stop-exact-object-service", nil, "docker", "stop", "--time", "8", objects.ID); e != nil {
		return facts, e
	}
	if status, e := h.daemonReadiness(ctx); e != nil || status != http.StatusServiceUnavailable {
		return facts, m2.ErrPrivate
	}
	if e = probeLive(); e != nil {
		return facts, e
	}
	facts["live_platform_and_proxy_with_objects_unready"] = true
	facts["readiness_rejects_actual_dependency_outage"] = true
	if _, e = h.command(ctx, "health-restore-exact-object-service", nil, "docker", "start", objects.ID); e != nil {
		return facts, e
	}
	if _, e = h.waitHealthy(ctx); e != nil {
		return facts, e
	}
	// Run the actual health binary from a separately owned nonroot read-only
	// container. Its test configuration maps the wildcard listener to the
	// already authenticated/registered service IP; Host and TLS remain exact.
	c := h.config
	c.TLS = m2.TLSFiles{CA: "/run/operator/ca.pem", Certificate: "/run/operator/service.pem", Key: "/run/operator/service.key"}
	c.ExternalAddress = proxy.Address
	type plain m2.Config
	b, e := json.Marshal(plain(c))
	if e != nil {
		return facts, e
	}
	path := filepath.Join(h.temp, "health-probe-config.json")
	if e = writePrivate(path, b); e != nil {
		return facts, e
	}
	h.registry["health_probe_config_sha256"] = object.Hash(b)
	raw, e := h.command(ctx, "create-owned-proxy-health-command", nil, "docker", "create", "--name", h.project+"-health-probe", "--network", h.network, "--user", strconv.Itoa(os.Geteuid())+":"+strconv.Itoa(os.Getegid()), "--read-only", "--cap-drop", "ALL", "--security-opt", "no-new-privileges:true", "--pids-limit", "16", "--memory", "64m", "--label", "trpg.m2.project="+h.project, "--label", "trpg.m2.source="+h.source, "--mount", "type=bind,src="+filepath.Join(h.temp, "operator/reverse-proxy")+",dst=/run/operator,readonly", "--mount", "type=bind,src="+path+",dst=/run/health-config.json,readonly", "--entrypoint", "/app/reverse-proxyd", h.image, "--mode=health", "--config=/run/health-config.json")
	if e != nil {
		return facts, e
	}
	h.healthProbe = strings.TrimSpace(string(raw))
	if !regexp.MustCompile(`^[a-f0-9]{64}$`).MatchString(h.healthProbe) {
		return facts, m2.ErrPrivate
	}
	h.registry["health_probe_container"] = h.healthProbe
	if e = atomicJSON(filepath.Join(h.out, "resources.json"), h.registry); e != nil {
		return facts, e
	}
	raw, e = h.command(ctx, "owned-proxy-health-created-binding", nil, "docker", "inspect", "--format", `{{.Id}} {{.Image}} {{index .Config.Labels "trpg.m2.project"}} {{index .Config.Labels "trpg.m2.source"}} {{.State.Running}}`, h.healthProbe)
	if e != nil || strings.TrimSpace(string(raw)) != h.healthProbe+" "+h.image+" "+h.project+" "+h.source+" false" {
		return facts, m2.ErrPrivate
	}
	// First prove the mapped command succeeds against the real live proxy.
	if _, e = h.command(ctx, "mapped-proxy-health-live", nil, "docker", "start", "--attach", h.healthProbe); e != nil {
		return facts, e
	}
	if _, e = h.command(ctx, "health-stop-exact-proxy", nil, "docker", "stop", "--time", "8", proxy.ID); e != nil {
		return facts, e
	}
	if status, e := h.daemonReadiness(ctx); e != nil || status != http.StatusOK {
		return facts, m2.ErrPrivate
	}
	_, probeError := h.command(ctx, "mapped-proxy-health-unavailable", nil, "docker", "start", "--attach", h.healthProbe)
	raw, e = h.command(ctx, "owned-proxy-health-exit", nil, "docker", "inspect", "--format", `{{.Id}} {{.Image}} {{index .Config.Labels "trpg.m2.project"}} {{index .Config.Labels "trpg.m2.source"}} {{.State.Running}} {{.State.ExitCode}}`, h.healthProbe)
	if e != nil || probeError == nil || strings.TrimSpace(string(raw)) != h.healthProbe+" "+h.image+" "+h.project+" "+h.source+" false 1" {
		return facts, m2.ErrPrivate
	}
	facts["stopped_proxy_health_fails_while_platform_readiness_succeeds"] = true
	facts["own_proxy_command_container"] = h.healthProbe
	facts["mapped_listener_address"] = proxy.Address
	if _, e = h.command(ctx, "health-restore-exact-proxy", nil, "docker", "start", proxy.ID); e != nil {
		return facts, e
	}
	if _, e = h.waitHealthy(ctx); e != nil {
		return facts, e
	}
	after, e := h.healthDomainDigest(ctx)
	if e != nil || before != after || h.provider.Calls() != calls {
		return facts, m2.ErrPrivate
	}
	facts["game_task_budget_model_digest_unchanged"] = before == after
	facts["provider_calls_unchanged"] = true
	return facts, nil
}

func (h *lifecycle) actualNetworks(ctx context.Context) (any, error) {
	facts := []map[string]any{}
	for _, role := range []string{"private", "edge", "provider"} {
		name := h.project + "-" + role
		raw, e := h.command(ctx, "actual-network", nil, "docker", "network", "inspect", name)
		if e != nil {
			return nil, e
		}
		var result []struct {
			ID         string `json:"Id"`
			Name       string
			Internal   bool
			Labels     map[string]string
			Containers map[string]struct{ Name string }
		}
		if json.Unmarshal(raw, &result) != nil || len(result) != 1 {
			return nil, m2.ErrPrivate
		}
		v := result[0]
		if v.Name != name || v.Labels["trpg.m2.project"] != h.project || v.Labels["trpg.m2.source"] != h.source || v.Internal != (role != "edge") {
			return nil, m2.ErrPrivate
		}
		expected := map[string]bool{}
		members := []string{"reverse-proxy", "platformd", "workerd", "lua-runner", "object-storage", "postgres"}
		if role == "edge" {
			members = []string{"reverse-proxy"}
		}
		if role == "provider" {
			members = []string{"workerd"}
		}
		for _, member := range members {
			expected[h.project+"-"+member+"-1"] = true
		}
		if len(v.Containers) != len(expected) {
			return nil, m2.ErrPrivate
		}
		for _, member := range v.Containers {
			if !expected[member.Name] {
				return nil, m2.ErrPrivate
			}
		}
		facts = append(facts, map[string]any{"name": name, "id": v.ID, "internal": v.Internal, "members": members, "public_reachability": "NOT_CLAIMED"})
	}
	h.registry["actual_networks"] = facts
	return facts, atomicJSON(filepath.Join(h.out, "resources.json"), h.registry)
}
func (h *lifecycle) auditOrdinaryOutputs() error {
	if h.provider != nil {
		h.markers = append(h.markers, h.provider.PrivateMarkers()...)
	}
	if h.browser != nil {
		h.markers = append(h.markers, h.browser.PrivateMarkers()...)
	}
	return filepath.WalkDir(h.out, func(path string, d os.DirEntry, e error) error {
		if e != nil {
			return e
		}
		if d.IsDir() {
			return nil
		}
		if !d.Type().IsRegular() {
			return m2.ErrPrivate
		}
		b, e := os.ReadFile(path)
		if e != nil || len(b) > 16<<20 {
			return m2.ErrPrivate
		}
		defer clear(b)
		for _, marker := range h.markers {
			if len(marker) >= 8 && bytes.Contains(b, marker) {
				return m2.ErrPrivate
			}
		}
		return nil
	})
}
func (h *lifecycle) cleanup(ctx context.Context) error {
	if h.provider != nil {
		h.provider.Close()
	}
	var result error
	if h.healthProbe != "" {
		_, e := h.command(ctx, "remove-owned-health-probe", nil, "docker", "rm", "--force", h.healthProbe)
		result = errors.Join(result, e)
	}
	if h.composeOwned {
		// Record every actual PostgreSQL Mount.Name before Docker removes the
		// container. The production parent mount must have no anonymous volume.
		raw, e := h.composeCommand(ctx, "ps", "--all", "--quiet", "postgres")
		id := strings.TrimSpace(string(raw))
		if e == nil && id != "" {
			raw, e = h.command(ctx, "owned-postgres-mounts-before-cleanup", nil, "docker", "inspect", "--format", `{"id":{{json .Id}},"image":{{json .Image}},"project":{{json (index .Config.Labels "trpg.m2.project")}},"source":{{json (index .Config.Labels "trpg.m2.source")}},"mounts":{{json .Mounts}}}`, id)
			var v struct {
				ID, Image, Project, Source string
				Mounts                     []struct{ Type, Name, Source, Destination string }
			}
			if e == nil && (json.Unmarshal(raw, &v) != nil || v.ID != id || v.Image != postgresImage || v.Project != h.project || v.Source != h.source) {
				e = m2.ErrPrivate
			}
			if e == nil {
				for _, mount := range v.Mounts {
					if mount.Type == "volume" && mount.Name != h.project+"_postgres" {
						e = m2.ErrPrivate
					}
				}
				h.registry["postgres_mounts_before_cleanup"] = v.Mounts
				e = errors.Join(e, atomicJSON(filepath.Join(h.out, "resources.json"), h.registry))
			}
		}
		if e == nil {
			_, e = h.composeCommand(ctx, "down", "--volumes", "--timeout", "8")
		}
		result = errors.Join(result, e)
	}
	if h.imageOwned {
		raw, e := h.command(ctx, "owned-image-exists", nil, "docker", "image", "ls", "--quiet", "--filter", "reference=trpg-platform/m2:"+h.project)
		result = errors.Join(result, e)
		if e == nil && len(bytes.TrimSpace(raw)) != 0 {
			_, e = h.command(ctx, "remove-image", nil, "docker", "image", "rm", "trpg-platform/m2:"+h.project)
			result = errors.Join(result, e)
		}
		ids, e := h.command(ctx, "owned-image-build-cache", nil, "docker", "image", "ls", "--all", "--no-trunc", "--quiet", "--filter", "label=trpg.m2.project="+h.project, "--filter", "label=trpg.m2.source="+h.source)
		result = errors.Join(result, e)
		for _, id := range strings.Fields(string(ids)) {
			if !regexp.MustCompile(`^sha256:[0-9a-f]{64}$`).MatchString(id) {
				result = errors.Join(result, m2.ErrPrivate)
				break
			}
			_, e = h.command(ctx, "remove-owned-build-cache", nil, "docker", "image", "rm", id)
			result = errors.Join(result, e)
		}
	}
	if h.providerOwned {
		_, e := h.command(ctx, "remove-provider-network", nil, "docker", "network", "rm", h.project+"-provider")
		result = errors.Join(result, e)
	}
	if h.edgeOwned {
		_, e := h.command(ctx, "remove-edge-network", nil, "docker", "network", "rm", h.project+"-edge")
		result = errors.Join(result, e)
	}
	if h.networkOwned {
		_, e := h.command(ctx, "remove-network", nil, "docker", "network", "rm", h.network)
		result = errors.Join(result, e)
	}
	for _, kind := range []string{"container", "network", "volume", "image"} {
		args := []string{"docker", kind, "ls", "-q", "--filter", "label=trpg.m2.project=" + h.project}
		if kind == "container" || kind == "image" {
			args = append(args, "--all")
		}
		raw, e := h.command(ctx, "cleanup-inventory", nil, args...)
		if e != nil || len(bytes.TrimSpace(raw)) != 0 {
			result = errors.Join(result, m2.ErrPrivate, e)
		}
	}
	if h.sentinel != "" {
		raw, e := h.sentinelState(ctx)
		unchanged := e == nil && string(raw) == h.sentinelSnapshot
		h.report.Phases = append(h.report.Phases, phase{Name: "unrelated-sentinel-preserved", Passed: unchanged, Facts: map[string]any{"actual_postgres_unchanged": unchanged, "state_digest": object.Hash(raw)}})
		if !unchanged {
			result = errors.Join(result, m2.ErrPrivate, e)
		}
		_, e = h.command(ctx, "remove-owned-sentinel", nil, "docker", "rm", "--force", h.sentinel)
		result = errors.Join(result, e)
	}
	// A retained runtime is recoverable when Docker cleanup is not proven.
	if result == nil {
		result = os.RemoveAll(h.temp)
	}
	h.report.Cleanup = result == nil
	return result
}

// Implemented below with the actual existing APIs and kernel observations.
func (h *lifecycle) ownerAction(ctx context.Context, lobby smoke.Lobby) error {
	e := smoke.RunOwnerAction(ctx, h.root, h.temp, h.out, h.project, h.source, h.config, h.plan, lobby, h.composeCommand, h.waitHealthy)
	for _, name := range []string{"owner-cookie", "owner-csrf", "owner-provider-key"} {
		b, readErr := os.ReadFile(filepath.Join(h.temp, "operator/platformd", name))
		if readErr == nil {
			h.markers = append(h.markers, b)
		}
	}
	return e
}
func (h *lifecycle) kernelFaults(ctx context.Context) (any, error) {
	c := h.config
	c.TLS = h.files["platformd"]
	c.Runner = filepath.Join(h.temp, "rootfs/app/lua-runner")
	c.SupervisorSocket = filepath.Join(h.temp, "run/supervisor/service.sock")
	return smoke.RunKernelFaults(ctx, c, h.project, h.composeCommand, h.command)
}
func (h *lifecycle) restartAndFaults(ctx context.Context, lobby smoke.Lobby) (any, error) {
	c := h.config
	c.TLS = h.files["platformd"]
	c.ObjectSocket = filepath.Join(h.temp, "run/object/service.sock")
	return smoke.RunRestartAndFaults(ctx, h.browser, lobby, h.provider, h.plan, c, h.ownerAction, h.composeCommand, h.waitHealthy, h.command, h.stopWorkerd)
}

func (h *lifecycle) objectProbe(ctx context.Context, operation string) (any, error) {
	ctx, cancel := context.WithTimeout(ctx, 20*time.Second)
	defer cancel()
	if h.plan.Classification != "TEST_ONLY_STANDARD_CARRIER" || (operation != "baseline" && operation != "unavailable" && operation != "corrupt") {
		return nil, m2.ErrConfiguration
	}
	// Bind the currently running process container and immutable test image,
	// including source/project, before invoking the finite helper interface.
	raw, e := h.composeCommand(ctx, "ps", "--quiet", "platformd")
	container := strings.TrimSpace(string(raw))
	if e != nil || !regexp.MustCompile(`^[a-f0-9]{64}$`).MatchString(container) {
		return nil, m2.ErrPrivate
	}
	raw, e = h.command(ctx, "object-probe-container-binding", nil, "docker", "inspect", "--format", `{"id":{{json .Id}},"image":{{json .Image}},"project":{{json (index .Config.Labels "trpg.m2.project")}},"source":{{json (index .Config.Labels "trpg.m2.source")}},"running":{{.State.Running}}}`, container)
	var v struct {
		ID, Image, Project, Source string
		Running                    bool
	}
	if e != nil || json.Unmarshal(raw, &v) != nil || v.ID != container || v.Image != h.image || v.Project != h.project || v.Source != h.source || !v.Running {
		return nil, m2.ErrPrivate
	}
	packageBefore, domainBefore, e := smoke.ObjectProbeMetadata(ctx, h.plan, h.composeCommand)
	if e != nil {
		return nil, e
	}
	raw, e = h.composeCommand(ctx, "exec", "--no-TTY", "platformd", "/app/m2-object-probe", "--config=/run/operator/config.json", "--source="+h.source, "--project="+h.project, "--object-probe="+operation)
	facts, decodeError := smoke.DecodeObjectProbeOutput(raw)
	if e != nil || len(raw) > 4096 || decodeError != nil || !facts.ConsumersValidated || facts.Operation != operation {
		return facts, m2.ErrPrivate
	}
	packageAfter, domainAfter, e := smoke.ObjectProbeMetadata(ctx, h.plan, h.composeCommand)
	facts.PackageDigestUnchanged = e == nil && packageBefore == packageAfter
	facts.DomainDigestUnchanged = e == nil && domainBefore == domainAfter
	facts.Passed = facts.ConsumersValidated && facts.PackageDigestUnchanged && facts.DomainDigestUnchanged && ctx.Err() == nil
	if !facts.Passed {
		return facts, m2.ErrPrivate
	}
	return facts, nil
}

func (h *lifecycle) sentinelState(ctx context.Context) ([]byte, error) {
	raw, e := h.command(ctx, "sentinel-state", nil, "docker", "inspect", "--format", "{{.Id}} {{.State.Pid}} {{.State.StartedAt}} {{.Image}} {{.State.Running}} {{json .Mounts}}", h.sentinel)
	if e != nil {
		return nil, e
	}
	return canonicalSentinelState(raw)
}

func canonicalSentinelState(raw []byte) ([]byte, error) {
	fields := strings.SplitN(strings.TrimSpace(string(raw)), " ", 6)
	var mounts []json.RawMessage
	if len(fields) != 6 || json.Unmarshal([]byte(fields[5]), &mounts) != nil || len(mounts) == 0 {
		return nil, m2.ErrPrivate
	}
	for i, rawMount := range mounts {
		var mount map[string]json.RawMessage
		if json.Unmarshal(rawMount, &mount) != nil || len(mount) == 0 {
			return nil, m2.ErrPrivate
		}
		// Preserve every field and duplicate mount; only order is irrelevant.
		canonical, e := json.Marshal(mount)
		if e != nil {
			return nil, e
		}
		mounts[i] = canonical
	}
	sort.Slice(mounts, func(i, j int) bool { return bytes.Compare(mounts[i], mounts[j]) < 0 })
	canonical, e := json.Marshal(mounts)
	if e != nil {
		return nil, e
	}
	return []byte(strings.Join(fields[:5], " ") + " " + string(canonical) + "\n"), nil
}
func (h *lifecycle) startSentinel(ctx context.Context) (any, error) {
	name := h.project + "-sentinel"
	existing, e := h.command(ctx, "sentinel-preexist", nil, "docker", "ps", "--all", "--quiet", "--filter", "name=^/"+name+"$")
	if e != nil || len(bytes.TrimSpace(existing)) != 0 {
		return nil, m2.ErrPrivate
	}
	data := filepath.Join(h.temp, "sentinel/data")
	operator := filepath.Join(h.temp, "sentinel/operator")
	if e = os.MkdirAll(data, 0700); e != nil {
		return nil, e
	}
	if e = writePrivate(filepath.Join(operator, "password"), []byte("owned-synthetic-sentinel-password")); e != nil {
		return nil, e
	}
	raw, e := h.command(ctx, "sentinel-start", nil, "docker", "run", "--detach", "--name", name, "--network", "none", "--user", strconv.Itoa(os.Geteuid())+":"+strconv.Itoa(os.Getegid()), "--cap-drop", "ALL", "--security-opt", "no-new-privileges:true", "--pids-limit", "64", "--memory", "256m", "--label", "trpg.m2.sentinel="+h.project, "--env", "PGDATA=/var/lib/postgresql/data", "--env", "POSTGRES_DB=m2_sentinel", "--env", "POSTGRES_USER=m2_sentinel", "--env", "POSTGRES_PASSWORD_FILE=/run/operator/password", "--mount", "type=bind,src="+data+",dst=/var/lib/postgresql", "--mount", "type=bind,src="+operator+",dst=/run/operator,readonly", postgresImage)
	if e != nil {
		return nil, e
	}
	h.sentinel = strings.TrimSpace(string(raw))
	if len(h.sentinel) != 64 {
		return nil, m2.ErrPrivate
	}
	timeout := time.NewTimer(15 * time.Second)
	defer timeout.Stop()
	for {
		_, e = h.command(ctx, "sentinel-ready", nil, "docker", "exec", h.sentinel, "pg_isready", "-h", "127.0.0.1", "-U", "m2_sentinel", "-d", "m2_sentinel")
		if e == nil {
			break
		}
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-timeout.C:
			return nil, m2.ErrPrivate
		case <-time.After(100 * time.Millisecond):
		}
	}
	raw, e = h.sentinelState(ctx)
	if e != nil || !bytes.Contains(raw, []byte(" true")) {
		return nil, m2.ErrPrivate
	}
	h.sentinelSnapshot = string(raw)
	return map[string]any{"real_postgres": true, "container": h.sentinel, "separate_project_label": true, "state_digest": object.Hash(raw)}, nil
}

func (h *lifecycle) stopWorkerd(ctx context.Context, binding smoke.WorkerdStopBinding) (smoke.WorkerdStopFacts, error) {
	ctx, cancel := context.WithTimeout(ctx, 2*time.Second)
	defer cancel()
	if h.plan.Classification != "TEST_ONLY_STANDARD_CARRIER" {
		return smoke.WorkerdStopFacts{}, m2.ErrConfiguration
	}
	raw, e := h.composeCommand(ctx, "ps", "--quiet", "workerd")
	container := strings.TrimSpace(string(raw))
	if e != nil || !regexp.MustCompile(`^[a-f0-9]{64}$`).MatchString(container) {
		return smoke.WorkerdStopFacts{}, m2.ErrPrivate
	}
	raw, e = h.command(ctx, "signal-helper-owned-container", nil, "docker", "inspect", "--format", `{"id":{{json .Id}},"image":{{json .Image}},"project":{{json (index .Config.Labels "trpg.m2.project")}},"source":{{json (index .Config.Labels "trpg.m2.source")}},"running":{{.State.Running}}}`, container)
	var owner struct {
		ID, Image, Project, Source string
		Running                    bool
	}
	if e != nil || json.Unmarshal(raw, &owner) != nil || owner.ID != container || owner.Image != h.image || owner.Project != h.project || owner.Source != h.source || !owner.Running {
		return smoke.WorkerdStopFacts{}, m2.ErrPrivate
	}
	input, e := json.Marshal(binding)
	if e != nil {
		return smoke.WorkerdStopFacts{}, e
	}
	raw, e = h.command(ctx, "signal-helper-exact-workerd", input, "docker", "exec", "--interactive", container, "/app/m2-object-probe", "--stop-workerd", "--config=/run/operator/config.json", "--source="+h.source, "--project="+h.project)
	var facts smoke.WorkerdStopFacts
	if decodeError := json.Unmarshal(raw, &facts); e != nil || decodeError != nil || len(raw) > 2048 || !facts.Passed || !facts.ObservedT || !facts.CurrentBirthBound || facts.Code != "OBSERVED_T" {
		return facts, m2.ErrPrivate
	}
	return facts, nil
}
