// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
// CI-only evidence driver; executed from RUNNER_TEMP, never from candidate sources.
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';

const candidate = '473c94190b4eb9e203ab4ea823eaf72829960ad9';
const tree = 'ea87972af11c8c5956c116b58ff946291a0b3bdf';
const contract = '5dbcccd9ddda20bbcbdcbff473fb9197b83bb087a10f1ee802ed8ad547d74677';
const profiles = {
  Windows: { platform: 'win32', arch: 'x64', packages: ['./internal/projectctl/...', './internal/package/...', './cmd/projectctl/...', './cmd/creator-cli/...', './apps/creator-studio/...'] },
  macOS: { platform: 'darwin', arch: 'arm64', packages: ['./internal/projectctl/...', './cmd/projectctl/...'] },
  Linux: { platform: 'linux', arch: 'x64', packages: [] },
};
const output = path.join(process.env.RUNNER_TEMP, 'm1-b002-native-evidence');
process.env.GOCACHE = path.join(process.env.RUNNER_TEMP, 'm1-b002-go-build');
process.env.GOMODCACHE = path.join(process.env.RUNNER_TEMP, 'm1-b002-go-mod');
for (const prefix of ['npm_config_', 'pnpm_config_']) {
  process.env[prefix + 'store_dir'] = path.join(process.env.RUNNER_TEMP, 'm1-b002-pnpm-store');
  process.env[prefix + 'cache_dir'] = path.join(process.env.RUNNER_TEMP, 'm1-b002-pnpm-cache');
}
fs.mkdirSync(output, { recursive: true });
const digest = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const writeJSON = (name, value) => fs.writeFileSync(path.join(output, name), JSON.stringify(value, null, 2) + '\n');
const report = {
  candidate_sha: candidate, candidate_tree: tree, contract_digest: contract,
  event: process.env.GITHUB_EVENT_NAME, run_id: process.env.GITHUB_RUN_ID,
  run_attempt: process.env.GITHUB_RUN_ATTEMPT, job_key: process.env.GITHUB_JOB,
  workflow_ref: process.env.GITHUB_WORKFLOW_REF, workflow_sha: process.env.GITHUB_WORKFLOW_SHA,
  event_sha: process.env.GITHUB_SHA, event_ref: process.env.GITHUB_REF,
  repository: process.env.GITHUB_REPOSITORY,
  driver_sha256: digest(fs.readFileSync(new URL(import.meta.url))),
  runner: { name: process.env.RUNNER_NAME, os: process.env.RUNNER_OS, arch: process.env.RUNNER_ARCH,
    platform: process.platform, process_arch: process.arch, release: os.release(), version: os.version(),
    image: process.env.ImageOS, image_version: process.env.ImageVersion },
  environment: Object.fromEntries(['CI','GOENV','GOFLAGS','GOCACHE','GOMODCACHE','GOTOOLCHAIN','GOOS','GOARCH'].map(k => [k, process.env[k] ?? null])),
  node_version: process.version, commands: [], started_utc: new Date().toISOString(),
  canonical_result: 'NOT_RUN', native_acceptance: 'PENDING_INDEPENDENT_REVIEW',
};
function command(label, executable, args) {
  const started = new Date().toISOString();
  const child = spawnSync(executable, args, { cwd: process.cwd(), env: process.env,
    timeout: 20 * 60 * 1000, maxBuffer: 64 * 1024 * 1024 });
  const stdout = child.stdout ?? Buffer.alloc(0), stderr = child.stderr ?? Buffer.alloc(0);
  fs.writeFileSync(path.join(output, label + '.stdout'), stdout);
  fs.writeFileSync(path.join(output, label + '.stderr'), stderr);
  const record = { label, argv: [executable, ...args], cwd: process.cwd(), started_utc: started,
    finished_utc: new Date().toISOString(), exit_code: child.status, signal: child.signal,
    spawn_error: child.error?.message ?? null,
    stdout: { file: label + '.stdout', bytes: stdout.length, sha256: digest(stdout) },
    stderr: { file: label + '.stderr', bytes: stderr.length, sha256: digest(stderr) } };
  report.commands.push(record);
  console.log(JSON.stringify(record));
  process.stdout.write(stdout); process.stderr.write(stderr);
  if (child.status !== 0 || child.error || child.signal) {
    throw new Error(`${label} failed; see exact exit and output records`);
  }
  return stdout.toString('utf8');
}
function sourceState(label) {
  const sha = command(label + '-commit', 'git', ['rev-parse', 'HEAD']).trim();
  const actualTree = command(label + '-tree', 'git', ['rev-parse', 'HEAD^{tree}']).trim();
  const status = command(label + '-status', 'git', ['status', '--porcelain=v1', '--untracked-files=no']);
  command(label + '-worktree-diff', 'git', ['diff', '--exit-code', 'HEAD', '--']);
  command(label + '-index-diff', 'git', ['diff', '--cached', '--exit-code', '--']);
  const state = { sha, tree: actualTree, tracked_status: status, tracked_clean: status.trim() === '' };
  report[label] = state;
  if (sha !== candidate || actualTree !== tree || !state.tracked_clean) throw new Error(`${label}: fixed candidate/source mismatch`);
  return state;
}
function summarizeGoTests(raw) {
  const events = raw.split(/\r?\n/).filter(Boolean).map(line => JSON.parse(line));
  const count = action => events.filter(e => e.Test && e.Action === action).length;
  const result = { run_events: count('run'), pass_events: count('pass'), fail_events: count('fail'),
    skip_events: events.filter(e => e.Action === 'skip'),
    top_level_runs: events.filter(e => e.Test && !e.Test.includes('/') && e.Action === 'run').length,
    packages: events.filter(e => !e.Test && ['pass','fail','skip'].includes(e.Action)) };
  writeJSON('native-test-counts.json', result);
  return result;
}
let exitCode = 0;
try {
  if (report.repository !== 'zyc14588/TRPG_PLATFORM' || report.event !== 'workflow_dispatch') throw new Error('wrong repository/event');
  if (!/^[0-9a-f]{40}$/.test(report.event_sha ?? '') || report.workflow_sha !== report.event_sha) throw new Error('driver/event revision mismatch');
  const profile = profiles[report.runner.os];
  if (!profile || process.platform !== profile.platform || process.arch !== profile.arch) throw new Error('native OS/architecture mismatch');
  if (process.env.GOFLAGS !== '' || process.env.GOENV !== 'off') throw new Error('effective Go overrides must remain clean');
  sourceState('source_before');
  report.lock = JSON.parse(fs.readFileSync('tools/toolchain.lock.json', 'utf8'));
  report.go_environment = JSON.parse(command('go-environment', 'go', ['env','-json','GOVERSION','GOOS','GOARCH','GOHOSTOS','GOHOSTARCH','GOFLAGS','GOENV','GOCACHE','GOMODCACHE','CGO_ENABLED']));
  if (report.go_environment.GOOS !== profile.platform.replace('win32','windows') || report.go_environment.GOARCH !== profile.arch.replace('x64','amd64') || report.go_environment.GOFLAGS !== '') throw new Error('cross-build or suppressive Go settings detected');
  command('bootstrap', 'just', ['bootstrap']);
  sourceState('source_before_tests');
  command('canonical-ci', 'just', ['ci']);
  report.canonical_result = 'PASS';
  if (profile.packages.length) {
    // The canonical command hides successful subtest/skip events. Repeat only its
    // native Go allowlist to expose actual counts, with no cached test results.
    const raw = command('native-go-observability', 'go', ['test','-json','-count=1', ...profile.packages]);
    report.native_test_counts = summarizeGoTests(raw);
    if (report.native_test_counts.skip_events.length) report.native_acceptance = 'VISIBLE_SKIPS_REQUIRE_INDEPENDENT_DISPOSITION';
  }
} catch (error) {
  report.error = String(error); exitCode = 1;
} finally {
  const tests = path.join(output, 'native-go-observability.stdout');
  if (fs.existsSync(tests) && !report.native_test_counts) {
    try { report.native_test_counts = summarizeGoTests(fs.readFileSync(tests, 'utf8')); }
    catch (error) { report.test_count_error = String(error); }
  }
  try { sourceState('source_after'); } catch (error) { report.source_after_error = String(error); exitCode = 1; }
  function bytesAt(file) {
    if (!fs.existsSync(file)) return 0;
    const stat = fs.lstatSync(file);
    if (stat.isSymbolicLink()) throw new Error('Artifact symlink is outside the upload contract');
    return stat.isDirectory() ? fs.readdirSync(file).reduce((n, child) => n + bytesAt(path.join(file, child)), 0) : stat.size;
  }
  try {
    report.web_artifact_bytes = ['apps/web-player/dist', 'apps/creator-studio/frontend/dist', 'tools/toolchain.lock.json'].reduce((n, file) => n + bytesAt(file), 0);
    report.web_upload_ok = report.web_artifact_bytes <= 25 * 1024 * 1024;
    if (!report.web_upload_ok) { report.web_artifact_error = 'Exceeded approved 25 MiB per-job web artifact limit'; exitCode = 1; }
  } catch (error) { report.web_artifact_error = String(error); report.web_upload_ok = false; exitCode = 1; }
  if (process.env.GITHUB_OUTPUT) fs.appendFileSync(process.env.GITHUB_OUTPUT, `web_upload_ok=${report.web_upload_ok}\n`);
  report.finished_utc = new Date().toISOString(); report.driver_exit_code = exitCode;
  writeJSON('NATIVE_JOB_EVIDENCE.json', report);
  const artifacts = fs.readdirSync(output).map(name => {
    const bytes = fs.readFileSync(path.join(output, name)); return { name, bytes: bytes.length, sha256: digest(bytes) };
  });
  writeJSON('ARTIFACT_MANIFEST.json', { files: artifacts });
  console.log(JSON.stringify({ evidence_directory: output, report, artifacts }));
  const uploadOK = artifacts.reduce((n, f) => n + f.bytes, 0) + fs.statSync(path.join(output, 'ARTIFACT_MANIFEST.json')).size <= 25 * 1024 * 1024;
  if (process.env.GITHUB_OUTPUT) fs.appendFileSync(process.env.GITHUB_OUTPUT, `upload_ok=${uploadOK}\n`);
  if (!uploadOK) {
    console.error('Evidence exceeds the approved per-job 25 MiB upload limit'); exitCode = 1;
  }
}
process.exitCode = exitCode;
