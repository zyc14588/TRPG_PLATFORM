// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//go:build linux && m1_acceptance

package m1_test

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	minimal "github.com/zyc14588/TRPG_PLATFORM/tests/fixture-minimal"
	"github.com/zyc14588/TRPG_PLATFORM/tests/m1/evidence"
	"gopkg.in/yaml.v3"
)

type step struct {
	Args     []string `json:"argv"`
	Exit     int      `json:"exit_code"`
	Log, SHA string
	Named    map[string]int `json:"named_cases"`
	Verdict  string
}
type gate struct {
	ID, Owner, Criterion string
	Steps                []step
	Verdict              string
}
type proof struct {
	Candidate, NativePlanSHA, Platform, Verdict string
	Gates                                       []gate
	Qualifications                              []string
}

func goArgs(tags string, pkgs ...string) []string {
	a := []string{"go", "test", "-count=1", "-json"}
	if tags != "" {
		a = append(a, "-tags="+tags)
	}
	return append(a, pkgs...)
}

func TestM1ExitGate16(t *testing.T) {
	root := minimal.RepoRoot()
	sha, err := evidence.Candidate(root, "HEAD")
	if err != nil {
		t.Fatal(err)
	}
	base := os.Getenv("TRPG_M1_EVIDENCE_DIR")
	if base == "" {
		base = filepath.Join(root, "tests/m1/.artifacts")
	}
	if !filepath.IsAbs(base) {
		t.Fatal("evidence directory must be absolute")
	}
	dir, err := os.MkdirTemp(baseOrCreate(t, base), "run-")
	if err != nil {
		t.Fatal(err)
	}
	raw, err := os.ReadFile(filepath.Join(root, ".codex/state/MILESTONE_PLAN.yaml"))
	if err != nil {
		t.Fatal(err)
	}
	var plan struct {
		Batches []struct {
			ID        string   `yaml:"batch_id"`
			Sequence  int      `yaml:"sequence"`
			Contracts []string `yaml:"machine_contracts"`
		} `yaml:"batches"`
	}
	if yaml.Unmarshal(raw, &plan) != nil {
		t.Fatal("native plan cannot be decoded")
	}
	p := proof{Candidate: sha, NativePlanSHA: checkpoint.Hash(raw), Platform: "Linux", Verdict: "FAIL", Qualifications: []string{"Each step executes fresh against this clean candidate; no prior receipt is reused.", "Windows/macOS NOT_RUN; full V1 is incomplete.", "Fuzz targets cover bounded entrypoints, not a generated Session state machine."}}
	specs := []struct {
		id, owner, criterion string
		commands             [][]string
	}{
		{"TEST-PACKAGE-001", "M1-B001", "All five roles and Bundle parse and round-trip canonically", [][]string{goArgs("", "./internal/package/...")}},
		{"TEST-PACKAGE-002", "M1-B001", "Immutable IDs, artifact provenance, hashes and exact locks reject substitution", [][]string{goArgs("", "./internal/package/...")}},
		{"TEST-PACKAGE-003", "M1-B001", "Three-layer intersection and default-zero capability grants fail closed", [][]string{goArgs("", "./internal/package/capability")}},
		{"TEST-PACKAGE-004", "M1-B001", "Exact transitive locks reject cycles and conflicting versions", [][]string{goArgs("", "./internal/package/dependency")}},
		{"TEST-PACKAGE-005", "M1-B003", "Actual SQL/object installation commits atomically after required gates", [][]string{goArgs("", "./internal/package/install"), goArgs("integration", "./tests/integration/package_install")}},
		{"TEST-PACKAGE-008", "M1-B007", "Actual safe migration, rehearsals, SQL rollback, restart and recorded point restoration", [][]string{goArgs("integration", "./tests/integration/migration")}},
		{"TEST-SESSION-002", "M1-B005", "Bounded single-writer mailbox isolates Sessions and actual SQL commands", [][]string{goArgs("", "./internal/session/actor"), goArgs("integration", "./tests/integration/session")}},
		{"TEST-SESSION-003", "M1-B005", "Actual commit barriers, idempotency and filtered two-seat broadcasts", [][]string{goArgs("integration", "./tests/integration/session")}},
		{"TEST-LUA-001", "M1-B002", "Production Lua 5.5 source-only Profile denies dangerous libraries and Debug", [][]string{goArgs("", "./internal/luaruntime/profile", "./cmd/lua-runner")}},
		{"TEST-LUA-002", "M1-B002", "Dedicated VM lifecycle, checkpoints, faults and process reaping", [][]string{goArgs("", "./internal/luaruntime/vm", "./internal/luaruntime/checkpoint")}},
		{"TEST-LUA-003", "M1-B004", "Seven effects, rollback, cancellation and SQL acknowledgement loss are atomic", [][]string{goArgs("", "./internal/hostapi"), goArgs("integration", "./tests/integration/hostapi")}},
		{"TEST-LUA-004", "M1-B004", "Namespace and named relational permissions deny DDL/raw SQL and cross-tenant access", [][]string{goArgs("integration", "./tests/integration/hostapi")}},
		{"TEST-LUA-006", "M1-B004", "Host/Runner budgets and mandatory minimum audits cannot be disabled", [][]string{goArgs("", "./internal/hostapi", "./internal/luaruntime/...")}},
		{"TEST-DATA-001", "M1-B006", "Original event/input history reconstructs every recovery boundary without model calls", [][]string{goArgs("integration", "./tests/integration/replay")}},
		{"TEST-SEC-001", "M1-B008", "Eleven actual bypass matrices, private-diagnostic checks and real SQL isolation", [][]string{goArgs("security", "./tests/security")}},
		{"TEST-QUALITY-003", "M1-B009", "Fixed complete snapshots/views/checkpoints and eight committed bounded fuzz corpora", [][]string{goArgs("m1_acceptance", "./tests/fixture-minimal")}},
	}
	for _, f := range []struct{ pkg, name string }{{"protocol", "FuzzProtocolEnvelope"}, {"package", "FuzzPackageArchive"}, {"package", "FuzzPackageManifestTOML"}, {"package", "FuzzPackageSchema"}, {"callback", "FuzzHostCallbackParameters"}, {"event", "FuzzEventDeserialization"}, {"event", "FuzzEventUpcaster"}, {"migration", "FuzzMigrationEntrypoint"}} {
		corpus := filepath.Join(root, "tests/fuzz", f.pkg, "testdata/fuzz", f.name)
		entries, e := os.ReadDir(corpus)
		if e != nil || len(entries) < 2 {
			t.Fatal("committed corpus missing", f.name)
		}
		specs[15].commands = append(specs[15].commands, []string{"go", "test", "-json", "-run=^$", "-fuzz=^" + f.name + "$", "-fuzztime=30s", "./tests/fuzz/" + f.pkg})
	}
	// Check the lowest-sequence primary owner; downstream supporting batches
	// may repeat an ID but cannot become its primary owner.
	for _, s := range specs {
		owner := ""
		seq := int(^uint(0) >> 1)
		for _, b := range plan.Batches {
			for _, id := range b.Contracts {
				if id == s.id && b.Sequence < seq {
					seq = b.Sequence
					owner = b.ID
				}
			}
		}
		if owner != s.owner {
			t.Fatalf("primary owner mismatch for %s", s.id)
		}
	}
	for _, key := range []string{"B003_POSTGRES_DSN", "B005_POSTGRES_DSN", "B006_POSTGRES_DSN", "B007_POSTGRES_DSN", "B008_POSTGRES_DSN", "TRPG_HOSTAPI_PG_DSN", "TRPG_M1_FIXTURE_PG_DSN"} {
		u, e := url.Parse(os.Getenv(key))
		if e != nil || u.Scheme != "postgres" || u.Hostname() != "127.0.0.1" || u.Port() == "" {
			t.Fatalf("explicit owned local database required for %s", key)
		}
	}
	write := func() {
		if e := evidence.Write(filepath.Join(dir, "m1-exit-gate.json"), p); e != nil {
			t.Fatal("machine evidence write failed", e)
		}
	}
	write()
	for _, s := range specs {
		g := gate{ID: s.id, Owner: s.owner, Criterion: s.criterion, Verdict: "FAIL"}
		passed := t.Run(s.id, func(t *testing.T) {
			for n, args := range s.commands {
				ctx, cancel := context.WithTimeout(context.Background(), 8*time.Minute)
				cmd := exec.CommandContext(ctx, args[0], args[1:]...)
				cmd.Dir = root
				cmd.Env = os.Environ()
				cmd.Env = append(cmd.Env, "TRPG_SECURITY_EVIDENCE_DIR="+filepath.Join(dir, "security"), "TRPG_M1_FIXED_EVIDENCE="+filepath.Join(dir, "fixed-replay.json"))
				if strings.Contains(strings.Join(args, " "), "-fuzz=") {
					cmd.Env = append(cmd.Env, "GOMAXPROCS=2")
				}
				out, e := cmd.CombinedOutput()
				cancel()
				exit := 0
				if e != nil {
					exit = 1
					if x, ok := e.(*exec.ExitError); ok {
						exit = x.ExitCode()
					}
				}
				path := filepath.Join(dir, fmt.Sprintf("%s-%02d.jsonl", s.id, n))
				if e = os.WriteFile(path, out, 0600); e != nil {
					t.Fatal(e)
				}
				counts := map[string]int{"run": 0, "pass": 0, "fail": 0, "skip": 0}
				scan := bufio.NewScanner(bytes.NewReader(out))
				scan.Buffer(make([]byte, 4096), 4<<20)
				pkgFail := 0
				for scan.Scan() {
					var v struct{ Action, Test string }
					if json.Unmarshal(scan.Bytes(), &v) != nil {
						t.Fatal("invalid go JSON artifact")
					}
					if v.Test != "" {
						if _, ok := counts[v.Action]; ok {
							counts[v.Action]++
						}
					} else if v.Action == "fail" {
						pkgFail++
					}
				}
				st := step{Args: args, Exit: exit, Log: path, SHA: checkpoint.Hash(out), Named: counts, Verdict: "FAIL"}
				valid := e == nil && scan.Err() == nil && exit == 0 && pkgFail == 0 && counts["run"] > 0 && counts["run"] == counts["pass"] && counts["fail"] == 0 && counts["skip"] == 0
				for _, marker := range []string{"fixture-gm-private-value", "sensitive-fixture-value", "must-not-reach-runner", "migration-fixture-gm-private"} {
					if bytes.Contains(out, []byte(marker)) {
						valid = false
					}
				}
				if valid {
					st.Verdict = "PASS"
				}
				g.Steps = append(g.Steps, st)
				if e := evidence.Write(filepath.Join(dir, s.id+".json"), struct {
					Candidate string
					Gate      gate
				}{sha, g}); e != nil {
					t.Fatal(e)
				}
				if !valid {
					t.Fatalf("fresh gate step failed; exit=%d named=%v artifact=%s", exit, counts, path)
				}
				if _, e := evidence.Candidate(root, sha); e != nil {
					t.Fatal(e)
				}
			}
			g.Verdict = "PASS"
		})
		p.Gates = append(p.Gates, g)
		if e := evidence.Write(filepath.Join(dir, s.id+".json"), struct {
			Candidate string
			Gate      gate
		}{sha, g}); e != nil {
			t.Fatal(e)
		}
		write()
		if !passed {
			t.FailNow()
		}
	}
	if len(p.Gates) != 16 {
		t.Fatal("incomplete M1 gate coverage")
	}
	p.Verdict = "PASS"
	write()
	t.Logf("M1 Linux 16/16 fresh gates candidate=%s artifact=%s", sha, filepath.Join(dir, "m1-exit-gate.json"))
}
func baseOrCreate(t *testing.T, path string) string {
	t.Helper()
	if e := os.MkdirAll(path, 0700); e != nil {
		t.Fatal(e)
	}
	return path
}
