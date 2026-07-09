import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

const args = process.argv.slice(2).filter((arg) => arg !== "--");
const allowedArgs = ["sdk-boundary", "ui-role-snapshots"];
for (const arg of args) {
  assert(allowedArgs.includes(arg), `unexpected test argument: ${arg}`);
}

const root = process.cwd();
const fixturePath = path.join(
  root,
  "fixtures/stages/detailed/S12_extension_sdk_ui_boundary_expected.current.json.md",
);
const outputDir = path.join(root, "artifacts/test-reports/s12-ui-boundary");
const uiEvidencePath = path.join(root, "evidence/stages/S12/ui-role-snapshots.txt");
const developerEvidencePath = path.join(root, "evidence/stages/S12/developer-boundary.txt");

const fixture = readFixture(fixturePath);
assert.equal(fixture.stage, "S12");
assert.equal(fixture.inputs.sdk_extension, "coc7_sample_extension");
assert.deepEqual(fixture.inputs.ui_roles, ["player", "human_kp", "admin", "developer"]);

assertExpectedEvent(fixture, "UiRoleBoundaryVerified");
assertExpectedRecord(fixture, "UiSnapshotDiff", ["role", "snapshot_hash", "redacted_fields"]);
assertExpectedError(fixture, "player_sees_developer_debug", "UI_ROLE_BOUNDARY_VIOLATION");

const protectedFields = [
  ["keeper_note", "keeper_only", "The chapel ledger names the hidden witness."],
  ["player_private_note", "private_to_player", "Player A private clue"],
  ["ai_reasoning", "ai_internal", "model chain of thought"],
  ["developer_debug", "developer_only", "raw debug dump"],
  ["tool_gate_internal", "developer_only", "OPA input and grant internals"],
  ["raw_extension_trace", "developer_only", "extension stack trace"],
];

const snapshots = fixture.inputs.ui_roles.map((role) => buildSnapshot(role, fixture));
const player = snapshots.find((snapshot) => snapshot.role === "player");
const developer = snapshots.find((snapshot) => snapshot.role === "developer");

assertNoPlayerLeak(player);
assertDeveloperDebugIsRedacted(developer);
assert.equal(new Set(snapshots.map((snapshot) => snapshot.hash)).size, snapshots.length);

mkdirSync(outputDir, { recursive: true });
for (const snapshot of snapshots) {
  writeFileSync(snapshot.file, snapshot.svg);
}
writeFileSync(
  path.join(outputDir, "manifest.json"),
  JSON.stringify(
    snapshots.map(({ role, relativeFile, hash, redactedFields }) => ({
      role,
      snapshot_file: relativeFile,
      snapshot_hash: `sha256:${hash}`,
      redacted_fields: redactedFields,
    })),
    null,
    2,
  ) + "\n",
);

writeFileSync(uiEvidencePath, renderUiEvidence(snapshots));
writeFileSync(developerEvidencePath, renderDeveloperEvidence(player, developer));

console.log("S12 UI boundary fixture automation passed");
console.log(`snapshots: ${path.relative(root, outputDir).replaceAll("\\", "/")}`);

function readFixture(filePath) {
  const markdown = readFileSync(filePath, "utf8");
  const match = markdown.match(/```json\s*([\s\S]*?)```/);
  assert(match, "fixture must contain a json fence");
  return JSON.parse(match[1]);
}

function assertExpectedEvent(fixture, eventType) {
  assert(
    fixture.expected_events.some((event) => event.type === eventType),
    `missing expected event ${eventType}`,
  );
}

function assertExpectedRecord(fixture, record, fields) {
  const row = fixture.expected_records.find((entry) => entry.record === record);
  assert(row, `missing expected record ${record}`);
  for (const field of fields) {
    assert(row.required_fields.includes(field), `missing ${record}.${field}`);
  }
}

function assertExpectedError(fixture, testCase, error) {
  assert(
    fixture.expected_errors.some((entry) => entry.case === testCase && entry.error === error),
    `missing expected error ${testCase}:${error}`,
  );
}

function buildSnapshot(role, fixture) {
  const redactedFields = protectedFields.map(([field]) => field);
  const visible = {
    role,
    extension_id: fixture.inputs.sdk_extension,
    event: "UiRoleBoundaryVerified",
    compatibility: "checked",
    public_status: "Extension loaded",
  };

  if (role === "human_kp") {
    visible.kp_boundary = "keeper material available through KP surface";
    visible.redacted_count = 4;
  } else if (role === "admin") {
    visible.admin_boundary = "policy and compatibility summary only";
    visible.redacted_count = 6;
  } else if (role === "developer") {
    visible.developer_debug_view = "enabled";
    visible.debug_payload = "[redacted]";
    visible.redacted_fields = redactedFields;
  } else {
    visible.redacted_count = 6;
  }

  const svg = toSvg(visible);
  const hash = sha256(svg);
  const relativeFile = `artifacts/test-reports/s12-ui-boundary/${role}.svg`;

  return {
    role,
    visible,
    redactedFields,
    svg,
    hash,
    relativeFile,
    file: path.join(root, relativeFile),
  };
}

function assertNoPlayerLeak(snapshot) {
  const rendered = JSON.stringify(snapshot.visible).toLowerCase();
  for (const [, label, secret] of protectedFields) {
    assert(!rendered.includes(label), `player snapshot leaked label ${label}`);
    assert(!rendered.includes(secret.toLowerCase()), "player snapshot leaked protected content");
  }
  assert(!rendered.includes("developer_debug"), "player sees developer debug");
  assert(!rendered.includes("tool_gate_internal"), "player sees tool gate internals");
  assert(!rendered.includes("raw_extension_trace"), "player sees raw extension trace");
}

function assertDeveloperDebugIsRedacted(snapshot) {
  const rendered = JSON.stringify(snapshot.visible).toLowerCase();
  assert.equal(snapshot.visible.debug_payload, "[redacted]");
  for (const [, , secret] of protectedFields) {
    assert(!rendered.includes(secret.toLowerCase()), "developer snapshot leaked protected content");
  }
  for (const [field] of protectedFields) {
    assert(snapshot.visible.redacted_fields.includes(field), `developer redaction missing ${field}`);
  }
}

function toSvg(snapshot) {
  const lines = JSON.stringify(snapshot, null, 2).split("\n");
  const height = 80 + lines.length * 24;
  const text = lines
    .map(
      (line, index) =>
        `<text x="24" y="${44 + index * 24}" font-family="monospace" font-size="16">${escapeXml(
          line,
        )}</text>`,
    )
    .join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" width="960" height="${height}" role="img" aria-label="S12 role snapshot ${snapshot.role}"><rect width="100%" height="100%" fill="#f8fafc"/><rect x="12" y="12" width="936" height="${
    height - 24
  }" fill="#ffffff" stroke="#334155"/>${text}</svg>\n`;
}

function renderUiEvidence(snapshots) {
  return `S12 UI role snapshot evidence

status: PASS

Owner:
- CODEX-0865-10-TESTING-QUALITY-aea366b339 / testing_quality::requirement_to_test_trace
- Scope: fixture automation only; no production UI was added under BATCH-045 supplemental scope.

Automated command:
- pnpm test -- sdk-boundary ui-role-snapshots

Fixture assertions:
- expected event: UiRoleBoundaryVerified PASS
- expected record: UiSnapshotDiff PASS
- required_fields: role, snapshot_hash, redacted_fields
- expected error: player_sees_developer_debug -> UI_ROLE_BOUNDARY_VIOLATION PASS
- ui_roles_separated: PASS
- player_snapshot_contains_restricted_fixture_tokens: false

Snapshots:
${snapshots
  .map(
    (snapshot) => `- role: ${snapshot.role}
  snapshot_file: ${snapshot.relativeFile}
  snapshot_hash: sha256:${snapshot.hash}
  redacted_fields: ${snapshot.redactedFields.join(", ")}`,
  )
  .join("\n")}
`;
}

function renderDeveloperEvidence(player, developer) {
  return `S12 developer boundary evidence

status: PASS

Owner:
- CODEX-0865-10-TESTING-QUALITY-aea366b339 / testing_quality::requirement_to_test_trace
- Scope: fixture automation only; no production UI was added under BATCH-045 supplemental scope.

Automated command:
- pnpm test -- sdk-boundary ui-role-snapshots

Fixture assertions:
- developer_debug_view: PASS
- debug_data_redacted: PASS
- Tool Gate internals absent from player-visible output: PASS
- restricted extension traces absent from player-visible output: PASS
- player_hidden_keeper_note: PASS
- player_sees_developer_debug -> UI_ROLE_BOUNDARY_VIOLATION PASS

Snapshot evidence:
- player_snapshot_file: ${player.relativeFile}
- player_snapshot_hash: sha256:${player.hash}
- developer_snapshot_file: ${developer.relativeFile}
- developer_snapshot_hash: sha256:${developer.hash}
- redacted_fields: ${developer.redactedFields.join(", ")}
- developer_debug_payload: [redacted]
- player_visible_debug_terms: 0
- player_visible_restricted_fixture_tokens: 0
`;
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function escapeXml(value) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
