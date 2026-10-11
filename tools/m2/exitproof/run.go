// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package exitproof

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"time"
)

const postgresImage = "sha256:3a82e1f56c8f0f5616a11103ac3d47e632c3938698946a7ad26da0df1334744a"
const fixtureTask = "01a10c5c-1d16-7a91-8c2f-1909e2af4f43"

type Options struct{ Source, Output, Project string }
type executor struct {
	root, output, run string
	env               []string
	report            Report
	markers           []string
}

func Run(ctx context.Context, o Options) (result error) {
	if runtime.GOOS != "linux" || runtime.GOARCH != "amd64" || os.Geteuid() == 0 || !filepath.IsAbs(o.Output) || len(o.Source) != 40 || !strings.HasPrefix(o.Project, "trpg-m2-") {
		return errors.New("Linux owned absolute output and exact signed source required")
	}
	if _, e := os.Stat(o.Output); !os.IsNotExist(e) {
		return errors.New("output must be a fresh private directory")
	}
	if e := os.MkdirAll(o.Output, 0700); e != nil {
		return e
	}
	if e := os.Chmod(o.Output, 0700); e != nil {
		return e
	}
	root, e := os.Getwd()
	if e != nil {
		return e
	}
	var nonce [8]byte
	if _, e = rand.Read(nonce[:]); e != nil {
		return e
	}
	h := &executor{root: root, output: o.Output, run: "b012-" + hex.EncodeToString(nonce[:])}
	h.report = Report{Schema: Schema, Task: "M2-B012", Run: h.run, Verdict: "FAIL", CreatedUTC: time.Now().UTC().Format(time.RFC3339Nano), ToolVersions: map[string]string{}, Findings: []string{}, Limitations: []string{"after_reply daemon read/parse and complete Save/Post at fault remain NOT_PROVEN", "dead-caller ACK/Wait remains UNKNOWN", "local synthetic providers and TEST_ONLY minimal carrier only; public paid-provider, official tutorial/game, other platforms and full V1 qualification NOT_RUN", "producer result requires separate independent business ACCEPT and native state reception"}}
	h.env = os.Environ()
	h.markers = []string{"fixture-gm-private-value", "owned-B014-synthetic-private-provider-key", "B012-owned-synthetic-install-a", "B012-owned-synthetic-install-b"}
	// These caches and temporary paths are unique rebuildables owned by this run.
	for _, name := range []string{"tmp", "gocache"} {
		if e = os.MkdirAll(filepath.Join(o.Output, "rebuildables", name), 0700); e != nil {
			return e
		}
	}
	// Chrome's singleton uses TMPDIR for a Unix socket with a 108-byte path
	// limit. A short, unique owned directory also serves production Lua builds.
	runtimeRoot, e := os.MkdirTemp("/tmp", "m2-exit-"+h.run+"-")
	if e != nil {
		return e
	}
	if e = writeJSON(filepath.Join(o.Output, "runtime-owned.json"), map[string]string{"task": "M2-B012", "run": h.run, "temporary_root": runtimeRoot, "cache_root": filepath.Join(o.Output, "rebuildables", "gocache")}); e != nil {
		return e
	}
	h.env = setenv(h.env, "TMPDIR", runtimeRoot)
	h.env = setenv(h.env, "GOCACHE", filepath.Join(o.Output, "rebuildables", "gocache"))
	h.env = setenv(h.env, "GOTOOLCHAIN", "local")
	defer func() {
		if result != nil {
			h.report.Findings = append(h.report.Findings, "mandatory qualification failed: "+result.Error())
		}
		after, e := h.snapshot(ctx, o.Source, h.root)
		if e != nil {
			result = errors.Join(result, e)
		} else {
			h.report.After = after
		}
		h.report.Tests = TestResults(h.report.Stages)
		h.report.Cleanup = true
		for _, r := range h.report.Resources {
			h.report.Cleanup = h.report.Cleanup && r.Removed && r.Verified
		}
		if result == nil {
			result = Qualify(&h.report)
		}
		if result == nil {
			h.report.Verdict = "PASS"
		}
		if e = writeJSON(filepath.Join(o.Output, "report.json"), h.report); e != nil {
			result = errors.Join(result, e)
		}
		if e = h.seal(); e != nil {
			result = errors.Join(result, e)
		}
	}()
	h.report.Before, e = h.snapshot(ctx, o.Source, root)
	if e != nil {
		return e
	}
	if e = writeJSON(filepath.Join(o.Output, "source-manifest.json"), h.report.Before); e != nil {
		return e
	}
	// Recover the signed candidate physically before removing any owned fixture.
	if e = h.recover(ctx, o.Source); e != nil {
		return e
	}
	for _, tool := range []struct {
		Name string
		Args []string
	}{{"go", []string{"version"}}, {"git", []string{"--version"}}, {"node", []string{"--version"}}, {"pnpm", []string{"--version"}}, {"just", []string{"--version"}}, {"docker", []string{"version", "--format", "{{json .}}"}}, {"docker-compose", []string{"compose", "version"}}, {"google-chrome", []string{"--version"}}} {
		bin := tool.Name
		if bin == "docker-compose" {
			bin = "docker"
		}
		if bin == "google-chrome" && os.Getenv("M2B010_CHROME") != "" {
			bin = os.Getenv("M2B010_CHROME")
		}
		raw, _, e := h.command(ctx, "tool-"+tool.Name, h.root, h.env, false, append([]string{bin}, tool.Args...)...)
		if e != nil {
			return e
		}
		h.report.ToolVersions[tool.Name] = strings.TrimSpace(string(raw))
	}
	if !strings.Contains(h.report.ToolVersions["go"], "go1.26.5") || !strings.Contains(h.report.ToolVersions["google-chrome"], "Google Chrome") {
		return errors.New("pinned Go or actual Google Chrome absent")
	}
	for _, spec := range Stages() {
		if spec.Package == "./tests/e2e/player" {
			if e = h.frontend(ctx, o.Source); e != nil {
				return e
			}
		}
		fmt.Printf("M2_EXIT_STAGE %s\n", spec.Name)
		if e = h.goStage(ctx, o.Source, spec); e != nil {
			return e
		}
	}
	fmt.Println("M2_EXIT_STAGE compose")
	if e = h.compose(ctx, o); e != nil {
		return e
	}
	return nil
}
func setenv(env []string, key, value string) []string {
	out := []string{}
	for _, v := range env {
		if !strings.HasPrefix(v, key+"=") {
			out = append(out, v)
		}
	}
	return append(out, key+"="+value)
}
func digest(b []byte) string { h := sha256.Sum256(b); return hex.EncodeToString(h[:]) }
func writeJSON(path string, value any) error {
	raw, e := json.MarshalIndent(value, "", "  ")
	if e != nil {
		return e
	}
	return os.WriteFile(path, append(raw, '\n'), 0600)
}
func (h *executor) command(ctx context.Context, name, dir string, env []string, private bool, argv ...string) ([]byte, int, error) {
	i := len(h.report.Commands)
	base := fmt.Sprintf("command-%03d-%s", i, name)
	stdoutPath, stderrPath := filepath.Join(h.output, base+".stdout"), filepath.Join(h.output, base+".stderr")
	stdout, e := os.OpenFile(stdoutPath, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0600)
	if e != nil {
		return nil, i, e
	}
	defer stdout.Close()
	stderr, e := os.OpenFile(stderrPath, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0600)
	if e != nil {
		return nil, i, e
	}
	defer stderr.Close()
	start := time.Now()
	cmd := exec.CommandContext(ctx, argv[0], argv[1:]...)
	cmd.Dir = dir
	cmd.Env = env
	cmd.Stdout = stdout
	cmd.Stderr = stderr
	runError := cmd.Run()
	exit := 0
	if runError != nil {
		exit = -1
		var x *exec.ExitError
		if errors.As(runError, &x) {
			exit = x.ExitCode()
		}
	}
	raw, e := os.ReadFile(stdoutPath)
	errRaw, e2 := os.ReadFile(stderrPath)
	h.report.Commands = append(h.report.Commands, Command{Name: name, Argv: append([]string(nil), argv...), Exit: exit, StartedUTC: start.UTC().Format(time.RFC3339Nano), DurationMillis: time.Since(start).Milliseconds(), Stdout: Capture{Path: stdoutPath, SHA256: digest(raw), Bytes: int64(len(raw))}, Stderr: Capture{Path: stderrPath, SHA256: digest(errRaw), Bytes: int64(len(errRaw))}, Private: private})
	if !private {
		for _, marker := range h.markers {
			if strings.Contains(string(raw), marker) || strings.Contains(string(errRaw), marker) {
				h.report.Findings = append(h.report.Findings, "private marker in ordinary command output: "+name)
				return raw, i, errors.New("ordinary output privacy boundary failed")
			}
		}
	}
	if e != nil || e2 != nil || runError != nil {
		return raw, i, errors.New("command failed: " + name)
	}
	return raw, i, nil
}
func (h *executor) snapshot(ctx context.Context, source, root string) (Source, error) {
	s := Source{}
	for _, v := range []struct {
		name string
		args []string
	}{{"head", []string{"rev-parse", "HEAD"}}, {"tree", []string{"rev-parse", "HEAD^{tree}"}}, {"clean", []string{"status", "--porcelain=v1"}}, {"signature", []string{"verify-commit", "HEAD"}}} {
		raw, i, e := h.command(ctx, "source-"+v.name, root, h.env, false, append([]string{"git"}, v.args...)...)
		if e != nil {
			return s, e
		}
		switch v.name {
		case "head":
			s.Commit = strings.TrimSpace(string(raw))
		case "tree":
			s.Tree = strings.TrimSpace(string(raw))
		case "clean":
			s.Clean = len(raw) == 0
		case "signature":
			b, e := os.ReadFile(h.report.Commands[i].Stderr.Path)
			if e != nil || !strings.Contains(string(b), "Good \"git\" signature") || !strings.Contains(string(b), Fingerprint) {
				return s, errors.New("source signature trust mismatch")
			}
			s.Signature = Fingerprint
		}
	}
	if s.Commit != source || !s.Clean {
		return s, errors.New("source drift or dirty source")
	}
	raw, _, e := h.command(ctx, "source-files", root, h.env, false, "git", "ls-files", "--stage", "-z")
	if e != nil {
		return s, e
	}
	for _, line := range strings.Split(string(raw), "\x00") {
		if line == "" {
			continue
		}
		fields := strings.SplitN(line, "\t", 2)
		if len(fields) != 2 {
			return s, errors.New("tracked manifest malformed")
		}
		meta := strings.Fields(fields[0])
		if len(meta) != 3 || meta[2] != "0" {
			return s, errors.New("tracked unmerged source")
		}
		p := filepath.Join(root, fields[1])
		info, e := os.Lstat(p)
		if e != nil {
			return s, e
		}
		var b []byte
		if info.Mode()&os.ModeSymlink != 0 {
			target, e := os.Readlink(p)
			if e != nil {
				return s, e
			}
			b = []byte(target)
		} else {
			b, e = os.ReadFile(p)
			if e != nil {
				return s, e
			}
		}
		s.Files = append(s.Files, File{Path: fields[1], SHA256: digest(b), Bytes: int64(len(b)), Mode: meta[0] + ":" + info.Mode().String()})
	}
	b, e := json.Marshal(s.Files)
	if e != nil {
		return s, e
	}
	s.ManifestSHA256 = digest(b)
	return s, nil
}
func (h *executor) recover(ctx context.Context, source string) error {
	path := filepath.Join(h.output, "candidate.bundle")
	if _, _, e := h.command(ctx, "recovery-bundle", h.root, h.env, false, "git", "bundle", "create", path, "HEAD"); e != nil {
		return e
	}
	restore := filepath.Join(h.output, "recovered-source")
	if _, _, e := h.command(ctx, "recovery-clone", h.root, h.env, false, "git", "clone", "--no-hardlinks", path, restore); e != nil {
		return e
	}
	trust, _, e := h.command(ctx, "recovery-trust-ref", h.root, h.env, false, "git", "config", "--get", "gpg.ssh.allowedSignersFile")
	if e != nil {
		return e
	}
	// Trust is the already configured public allowlist; no private key access.
	raw, e := os.ReadFile(strings.TrimSpace(string(trust)))
	if e != nil {
		return e
	}
	trusted := filepath.Join(h.output, "existing-allowed-signers")
	if e = os.WriteFile(trusted, raw, 0600); e != nil {
		return e
	}
	for _, args := range [][]string{{"config", "--local", "gpg.format", "ssh"}, {"config", "--local", "gpg.ssh.allowedSignersFile", trusted}, {"checkout", "--detach", source}} {
		if _, _, e = h.command(ctx, "recovery-git", restore, h.env, false, append([]string{"git"}, args...)...); e != nil {
			return e
		}
	}
	s, e := h.snapshot(ctx, source, restore)
	if e != nil {
		return e
	}
	if !SameSource(h.report.Before, s) {
		return errors.New("physical restored candidate differs")
	}
	h.report.Recovered = s
	h.report.Recovery = true
	return writeJSON(filepath.Join(h.output, "recovery-manifest.json"), s)
}
func (h *executor) frontend(ctx context.Context, source string) error {
	before, e := h.snapshot(ctx, source, h.root)
	if e != nil {
		return e
	}
	_, i, e := h.command(ctx, "frontend-build", h.root, h.env, false, "pnpm", "--filter", "@trpg-platform/web-player", "build")
	after, ae := h.snapshot(ctx, source, h.root)
	h.report.Stages = append(h.report.Stages, Stage{Name: "frontend-build", Passed: e == nil && ae == nil, Cleanup: true, Commands: []int{i}, Assertions: map[string]string{}, SourceBefore: before, SourceAfter: after})
	return errors.Join(e, ae)
}
func (h *executor) goStage(ctx context.Context, source string, spec StageSpec) (result error) {
	before, e := h.snapshot(ctx, source, h.root)
	if e != nil {
		return e
	}
	stage := Stage{Name: spec.Name, Package: spec.Package, Required: append([]string(nil), spec.Assertions...), Assertions: map[string]string{}, SourceBefore: before, Cleanup: spec.Database == ""}
	env := h.env
	resourceIndex := -1
	defer func() {
		if resourceIndex >= 0 {
			cleanup, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()
			ce := h.removePostgres(cleanup, resourceIndex)
			stage.Cleanup = ce == nil
			result = errors.Join(result, ce)
		}
		after, e := h.snapshot(ctx, source, h.root)
		stage.SourceAfter = after
		result = errors.Join(result, e)
		stage.Passed = result == nil
		if result != nil {
			stage.Findings = []string{"mandatory actual package did not qualify"}
		}
		h.report.Stages = append(h.report.Stages, stage)
	}()
	if spec.Database != "" {
		resourceIndex, env, e = h.startPostgres(ctx, spec)
		if e != nil {
			return e
		}
	}
	if spec.Package == "./tests/e2e/player" {
		screen := filepath.Join(h.output, "chrome")
		if e = os.Mkdir(screen, 0700); e != nil {
			return e
		}
		env = setenv(env, "M2B010_EVIDENCE_DIR", screen)
	}
	args := []string{"go", "test", "-count=1", "-json", "-timeout=10m"}
	if spec.Database != "" {
		args = append(args, "-tags=integration")
	}
	args = append(args, spec.Package)
	raw, i, e := h.command(ctx, "go-"+spec.Name, h.root, env, false, args...)
	stage.Commands = []int{i}
	if e != nil {
		return e
	}
	stage.Assertions, e = ParseGo(raw, spec.Package, spec.Assertions)
	if e != nil {
		return e
	}
	if spec.Package == "./tests/e2e/player" {
		for _, name := range []string{"01-player-home.png", "02-player-filtered-game.png", "03-player-claimed-room-mobile.png", "04-player-unknown-command-before-pause.png", "05-player-unknown-command-paused.png", "06-player-ended-filtered-result.png", "filtered-personal-v5-c4.json"} {
			raw, e := os.ReadFile(filepath.Join(h.output, "chrome", name))
			if e != nil || len(raw) == 0 {
				return errors.New("missing actual Chrome capture")
			}
			h.report.Captures = append(h.report.Captures, Capture{Path: filepath.Join(h.output, "chrome", name), SHA256: digest(raw), Bytes: int64(len(raw))})
		}
	}
	return nil
}
func (h *executor) startPostgres(ctx context.Context, spec StageSpec) (int, []string, error) {
	env := h.env
	// Reserve a loopback port briefly; Docker must bind the same explicit port.
	l, e := net.Listen("tcp4", "127.0.0.1:0")
	if e != nil {
		return -1, env, e
	}
	port := strconv.Itoa(l.Addr().(*net.TCPAddr).Port)
	_ = l.Close()
	labels := map[string]string{"codex.task": fixtureTask, "codex.batch": spec.Batch, "codex.run": h.run + "-" + spec.Name}
	name := "trpg-m2-exit-" + h.run + "-" + spec.Name
	r := Resource{Stage: spec.Name, Batch: spec.Batch, Run: labels["codex.run"], Name: name, Image: postgresImage, Database: spec.Database, User: spec.User, Port: port, Labels: labels}
	i := len(h.report.Resources)
	h.report.Resources = append(h.report.Resources, r)
	if e = h.ledger(); e != nil {
		return i, env, e
	}
	var secret [24]byte
	if _, e = rand.Read(secret[:]); e != nil {
		return i, env, e
	}
	password := "synthetic-b012-" + hex.EncodeToString(secret[:])
	h.markers = append(h.markers, password)
	envPath := filepath.Join(h.output, "postgres-"+spec.Name+".env")
	if e = os.WriteFile(envPath, []byte("POSTGRES_USER="+spec.User+"\nPOSTGRES_DB="+spec.Database+"\nPOSTGRES_PASSWORD="+password+"\n"), 0600); e != nil {
		return i, env, e
	}
	raw, _, e := h.command(ctx, "postgres-create-"+spec.Name, h.root, h.env, false, "docker", "create", "--name", name, "--label", "codex.task="+fixtureTask, "--label", "codex.batch="+spec.Batch, "--label", "codex.run="+r.Run, "--env-file", envPath, "--publish", "127.0.0.1:"+port+":5432", postgresImage)
	if e != nil {
		return i, env, e
	}
	id := strings.TrimSpace(string(raw))
	if len(id) != 64 || strings.Trim(id, "0123456789abcdef") != "" {
		return i, env, errors.New("invalid created resource identity")
	}
	h.report.Resources[i].ID = id
	if e = h.ledger(); e != nil {
		return i, env, e
	}
	if _, _, e = h.command(ctx, "postgres-start-"+spec.Name, h.root, h.env, false, "docker", "start", id); e != nil {
		return i, env, e
	}
	ready := false
	for attempts := 0; attempts < 25; attempts++ {
		if _, _, e = h.command(ctx, "postgres-ready-"+spec.Name, h.root, h.env, false, "docker", "exec", id, "pg_isready", "-U", spec.User, "-d", spec.Database); e == nil {
			ready = true
			break
		}
		select {
		case <-ctx.Done():
			return i, env, ctx.Err()
		case <-time.After(200 * time.Millisecond):
		}
	}
	if !ready {
		return i, env, errors.New("owned PostgreSQL unavailable")
	}
	if e = h.verifyPostgres(ctx, i, true); e != nil {
		return i, env, e
	}
	h.report.Resources[i].Verified = true
	if e = h.ledger(); e != nil {
		return i, env, e
	}
	env = setenv(env, spec.Prefix+"_POSTGRES_DSN", "postgres://"+spec.User+":"+password+"@127.0.0.1:"+port+"/"+spec.Database+"?sslmode=disable")
	env = setenv(env, spec.Prefix+"_OWNED_DATABASE", "1")
	env = setenv(env, spec.Prefix+"_RUN_ID", r.Run)
	env = setenv(env, spec.Prefix+"_CONTAINER_ID", id)
	return i, env, nil
}
func (h *executor) verifyPostgres(ctx context.Context, i int, running bool) error {
	r := h.report.Resources[i]
	raw, _, e := h.command(ctx, "postgres-inspect-"+r.Stage, h.root, h.env, true, "docker", "inspect", r.ID)
	if e != nil {
		return e
	}
	var rows []struct {
		ID, Image, Name string
		State           struct{ Running bool }
		Config          struct {
			Labels map[string]string
			Env    []string
		}
		Mounts          []struct{ Type, Name string }
		NetworkSettings struct {
			Ports map[string][]struct{ HostIP, HostPort string }
		}
	}
	if json.Unmarshal(raw, &rows) != nil || len(rows) != 1 {
		return errors.New("physical resource inspect malformed")
	}
	x := rows[0]
	ports := x.NetworkSettings.Ports["5432/tcp"]
	if x.ID != r.ID || x.Image != r.Image || x.Name != "/"+r.Name || x.State.Running != running || len(x.Config.Labels) != 3 || len(ports) != 1 || ports[0].HostIP != "127.0.0.1" || ports[0].HostPort != r.Port {
		return errors.New("physical resource ownership or image/loopback mismatch")
	}
	volumes := []string{}
	for _, m := range x.Mounts {
		if m.Type == "volume" {
			if m.Name == "" {
				return errors.New("owned volume has no identity")
			}
			volumes = append(volumes, m.Name)
		}
	}
	if len(h.report.Resources[i].Volumes) == 0 {
		h.report.Resources[i].Volumes = volumes
	} else {
		a, _ := json.Marshal(h.report.Resources[i].Volumes)
		b, _ := json.Marshal(volumes)
		if string(a) != string(b) {
			return errors.New("owned volume association changed")
		}
	}
	for key, value := range r.Labels {
		if x.Config.Labels[key] != value {
			return errors.New("physical fixture exact labels mismatch")
		}
	}
	for _, pair := range []string{"POSTGRES_USER=" + r.User, "POSTGRES_DB=" + r.Database} {
		found := false
		for _, v := range x.Config.Env {
			found = found || v == pair
		}
		if !found {
			return errors.New("physical database/user identity mismatch")
		}
	}
	return nil
}
func (h *executor) removePostgres(ctx context.Context, i int) error {
	r := h.report.Resources[i]
	if r.ID == "" {
		return errors.New("fixture cleanup has no known identity")
	}
	if e := h.verifyPostgres(ctx, i, true); e != nil {
		return e
	}
	if _, _, e := h.command(ctx, "postgres-remove-"+r.Stage, h.root, h.env, false, "docker", "rm", "--force", "--volumes", r.ID); e != nil {
		return e
	}
	raw, _, e := h.command(ctx, "postgres-absence-"+r.Stage, h.root, h.env, false, "docker", "ps", "--all", "--quiet", "--no-trunc", "--filter", "label=codex.task="+fixtureTask, "--filter", "label=codex.batch="+r.Batch, "--filter", "label=codex.run="+r.Run)
	if e != nil || len(bytesTrim(raw)) != 0 {
		return errors.New("owned fixture remains after cleanup")
	}
	h.report.Resources[i].Removed = true
	return h.ledger()
}
func bytesTrim(raw []byte) []byte { return []byte(strings.TrimSpace(string(raw))) }
func (h *executor) ledger() error {
	return writeJSON(filepath.Join(h.output, "resource-ledger.json"), h.report.Resources)
}
func (h *executor) compose(ctx context.Context, o Options) error {
	before, e := h.snapshot(ctx, o.Source, h.root)
	if e != nil {
		return e
	}
	path := filepath.Join(h.output, "lifecycle")
	_, i, e := h.command(ctx, "formal-compose-lifecycle", h.root, h.env, false, "go", "run", "./tools/m2/lifecycle", "--source", o.Source, "--project", o.Project, "--output", path)
	stage := Stage{Name: "compose", Required: LifecycleAssertions(), Commands: []int{i}, Assertions: map[string]string{}, SourceBefore: before}
	after, ae := h.snapshot(ctx, o.Source, h.root)
	stage.SourceAfter = after
	var r struct {
		Candidate, Tree, Mode, Verdict string
		Cleanup                        bool
		Phases                         []struct {
			Name   string
			Passed bool
		}
	}
	raw, re := os.ReadFile(filepath.Join(path, "report.json"))
	if re == nil {
		re = json.Unmarshal(raw, &r)
	}
	if re != nil || r.Candidate != o.Source || r.Tree != before.Tree || r.Mode != "FORMAL_REQUIRED_VERIFICATION" || r.Verdict != "PASS" || !r.Cleanup {
		e = errors.Join(e, errors.New("actual formal lifecycle did not qualify"))
	}
	seen := map[string]bool{}
	for _, p := range r.Phases {
		if seen[p.Name] || !p.Passed {
			e = errors.Join(e, errors.New("failed or duplicate mandatory lifecycle phase"))
		}
		seen[p.Name] = true
		if p.Passed {
			stage.Assertions[p.Name] = "PASS"
		}
	}
	if len(r.Phases) != len(lifecycleAssertions) {
		e = errors.Join(e, errors.New("missing lifecycle phase"))
	}
	for _, name := range lifecycleAssertions {
		if !seen[name] {
			e = errors.Join(e, errors.New("missing required lifecycle phase "+name))
		}
	}
	stage.Cleanup = r.Cleanup
	stage.Passed = e == nil && ae == nil
	if !stage.Passed {
		stage.Findings = []string{"formal six-service lifecycle did not qualify"}
	}
	h.report.Stages = append(h.report.Stages, stage)
	return errors.Join(e, ae)
}
func (h *executor) seal() error {
	captures := []Capture{}
	e := filepath.WalkDir(h.output, func(path string, entry os.DirEntry, e error) error {
		if e != nil {
			return e
		}
		if entry.IsDir() {
			if path == filepath.Join(h.output, "rebuildables") || path == filepath.Join(h.output, "recovered-source") {
				return filepath.SkipDir
			}
			return nil
		}
		if entry.Type()&os.ModeSymlink != 0 || filepath.Base(path) == "seal.json" {
			return nil
		}
		b, e := os.ReadFile(path)
		if e != nil {
			return e
		}
		captures = append(captures, Capture{Path: path, SHA256: digest(b), Bytes: int64(len(b))})
		return nil
	})
	if e != nil {
		return e
	}
	return writeJSON(filepath.Join(h.output, "seal.json"), map[string]any{"schema": "m2-exit-private-seal/1", "source": h.report.Before, "files": captures})
}
