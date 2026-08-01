import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { waitUntil } from "../browser-test.mjs";
import { privateCanary } from "./context.mjs";
import { assertNoAlert } from "./support.mjs";
import { parsedResponses } from "./support.mjs";

export async function exportArtifact(page, audience, exportId) {
  const responseStart = page.responseBodies.length;
  const requestedAt = Date.now();
  await page.submit('form[data-form="export"]', { exportId, audience });
  await waitUntil(async () => {
    const alert = await page.evaluate("document.querySelector('.feedback.error')?.innerText || ''");
    if (alert) throw new Error(alert);
    const rendered = await page.evaluate(
      "document.querySelector('[data-testid=\"campaign-export\"]')?.innerText || ''",
    );
    return rendered.includes(exportId)
      && parsedResponses(page.responseBodies.slice(responseStart))
        .some(({ value }) => value?.manifest?.export_id === exportId);
  }, 60_000, `export did not complete: ${exportId}`);
  await assertNoAlert(page);
  const responses = parsedResponses(page.responseBodies.slice(responseStart));
  const artifactEntry = responses.find(({ value }) => value?.manifest?.export_id === exportId);
  const status = responses
    .filter(({ value }) => value?.export_id === exportId && typeof value?.state === "string")
    .at(-1)?.value;
  const authorization = responses.find(({ value }) =>
    typeof value?.token === "string" && typeof value?.expires_at_unix_ms === "number"
  )?.value;
  assert.ok(artifactEntry, `downloaded artifact missing for ${exportId}`);
  assert.ok(status, `READY status missing for ${exportId}`);
  assert.ok(authorization, `download authorization missing for ${exportId}`);
  assert.equal(status.state, "READY");
  assert.equal(status.audience, audience);
  assert.equal(status.attempt_count, 1);
  assert.equal(status.max_attempts, 5);
  assert.equal(status.failure_code, null);
  assert.match(authorization.token, /^[a-f0-9]{64}$/);
  assert.ok(authorization.expires_at_unix_ms > requestedAt);
  assert.ok(authorization.expires_at_unix_ms <= Date.now() + 60_500);
  validateArtifactIntegrity(artifactEntry.raw, artifactEntry.value, status);
  return { artifact: artifactEntry.value, authorization, raw: artifactEntry.raw, status };
}

export function validateArtifactIntegrity(raw, artifact, status) {
  const artifactHash = `sha256:${createHash("sha256").update(raw).digest("hex")}`;
  const manifestHash = `sha256:${createHash("sha256")
    .update(JSON.stringify(artifact.content))
    .digest("hex")}`;
  assert.equal(status.artifact_schema, "trpg.campaign-export.v1");
  assert.equal(status.visibility_policy_version, "visibility-policy-v1");
  assert.equal(status.artifact_hash, artifactHash);
  assert.equal(status.manifest_hash, manifestHash);
  assert.equal(status.artifact_size, Buffer.byteLength(raw));
  assert.equal(artifact.manifest.artifact_schema, status.artifact_schema);
  assert.equal(artifact.manifest.visibility_policy_version, status.visibility_policy_version);
  assert.equal(artifact.manifest.manifest_hash, status.manifest_hash);
  assert.equal(artifact.manifest.hash_algorithm, "sha256");
  assert.equal(artifact.manifest.manifest_hash_scope, "canonical-json:content");
  assert.equal(artifact.manifest.event_range.first_sequence, status.first_event_sequence);
  assert.equal(artifact.manifest.event_range.last_sequence, status.last_exported_event_sequence);
  assert.equal(artifact.manifest.event_range.count, status.event_count);
  assert.ok(status.retention_expires_at_unix_ms > Date.now());
}

export function validateExportViews(player, keeper, audit, context) {
  for (const exported of [player, keeper, audit]) {
    const { manifest } = exported.artifact;
    assert.equal(manifest.campaign_id, context.campaignId);
    assert.equal(manifest.authority.mode, "AI_KP");
    assert.ok(manifest.authority.contract_id);
    assert.ok(manifest.authority.contract_version > 0);
    assert.ok(manifest.authority.ruleset_version);
    assert.equal(manifest.fork_provenance.fork_id, exported.status.fork_id);
    assert.equal(manifest.fork_provenance.parent_campaign_id, context.parentCampaignId);
    assert.equal(manifest.fork_provenance.source_session_id, context.sourceSessionId);
    assert.match(manifest.fork_provenance.source_snapshot_hash, /^sha256:[a-f0-9]{64}$/);
    assert.match(manifest.fork_provenance.child_snapshot_hash, /^sha256:[a-f0-9]{64}$/);
    assert.equal(exported.status.parent_campaign_id, context.parentCampaignId);
  }

  assert.equal(player.artifact.manifest.view, "PLAYER");
  assert.deepEqual(
    Object.keys(player.artifact.content.sections).sort(),
    ["discovered_clues", "public_scene_summary", "visible_dice_rolls"],
  );
  for (const record of player.artifact.content.records) {
    assert.ok([
      "public",
      "party_visible",
      "spectator_visible",
      "spectator_hidden",
      "private_to_player",
      "investigator_private",
      "private_to_group",
    ].includes(record.visibility.label));
    if (["private_to_player", "investigator_private"].includes(record.visibility.label)) {
      assert.equal(record.visibility.subject, context.playerId);
    }
  }
  assert.equal(player.raw.includes(privateCanary), false);
  assert.equal(player.raw.includes("keeper_only"), false);
  assert.equal(player.raw.includes("ai_internal"), false);
  assert.equal(player.raw.includes("system_private"), false);

  assert.equal(keeper.artifact.manifest.view, "KEEPER_PRIVATE");
  assert.deepEqual(
    Object.keys(keeper.artifact.content.sections).sort(),
    ["all_public_events", "hidden_clues", "keeper_truth", "npc_secrets"],
  );
  assert.equal(
    keeper.artifact.content.records.some((record) =>
      ["ai_internal", "system_only", "system_private"].includes(record.visibility.label)
    ),
    false,
  );

  assert.equal(audit.artifact.manifest.view, "AUDIT");
  assert.deepEqual(
    Object.keys(audit.artifact.content.sections).sort(),
    ["decision_records", "dice_rolls", "model_route_snapshot", "tool_calls", "visibility_labels"],
  );
  assert.equal(audit.artifact.content.records.length, audit.artifact.manifest.event_range.count);
  const restricted = audit.artifact.content.records.filter((record) =>
    [
      "private_to_player",
      "investigator_private",
      "private_to_group",
      "keeper_only",
      "ai_internal",
      "system_only",
      "system_private",
    ].includes(record.visibility.label)
  );
  assert.ok(restricted.length > 0);
  for (const record of restricted) {
    assert.deepEqual(Object.keys(record.payload).sort(), ["payload_hash", "redacted"]);
    assert.equal(record.payload.redacted, true);
    assert.match(record.payload.payload_hash, /^sha256:[a-f0-9]{64}$/);
  }
}

export async function deleteExportSubject({
  requester,
  requesterAccessToken,
  owner,
  ownerAccessToken,
  keeper,
  keeperAccessToken,
  campaignId,
  subjectId,
  exportIds,
}) {
  const nonce = Date.now().toString(36);
  const jobId = `privacy_export_${nonce}`;
  const request = await browserFetch(
    requester,
    `/api/campaigns/${campaignId}/privacy/deletions`,
    {
      method: "POST",
      token: requesterAccessToken,
      headers: { "Idempotency-Key": `privacy_export_${nonce}_idempotency` },
      body: {
        job_id: jobId,
        subject_id: subjectId,
        retention_policy: "user_erasure_v1",
        reason: "AR12 verifies subject-bound export artifact erasure",
        command_id: `privacy_export_${nonce}_command`,
        correlation_id: `privacy_export_${nonce}_correlation`,
        causation_id: `privacy_export_${nonce}_causation`,
        expected_version: 0,
      },
    },
  );
  assert.equal(request.status, 202, JSON.stringify(request.body));
  let completed;
  await waitUntil(async () => {
    const response = await browserFetch(
      owner,
      `/api/campaigns/${campaignId}/privacy/deletions/${jobId}`,
      { token: ownerAccessToken },
    );
    if (response.status !== 200) throw new Error(`deletion status ${response.status}`);
    if (response.body.status === "failed") {
      throw new Error(`privacy deletion failed: ${JSON.stringify(response.body)}`);
    }
    if (response.body.status === "completed") {
      completed = response.body;
      return true;
    }
    return false;
  }, 60_000, "privacy deletion did not complete");
  assert.equal(completed.evidence_status, "confirmed");
  assert.equal(completed.targets.length, 7);
  assert.ok(completed.targets.every((target) => target.status === "verified"));
  assert.ok(completed.targets.some((target) => target.target === "export"));

  for (const exportId of exportIds) {
    const status = await browserFetch(
      keeper,
      `/api/api/v1/campaigns/${campaignId}/exports/${exportId}`,
      { token: keeperAccessToken },
    );
    assert.equal(status.status, 200);
    assert.equal(status.body.state, "DELETED");
    const authorization = await browserFetch(
      keeper,
      `/api/api/v1/campaigns/${campaignId}/exports/${exportId}/download-authorizations`,
      { method: "POST", token: keeperAccessToken },
    );
    assert.equal(authorization.status, 404);
  }
}

export async function browserFetch(page, requestPath, { method = "GET", token, headers = {}, body } = {}) {
  return page.evaluate(`(async () => {
    const headers = ${JSON.stringify(headers)};
    if (${JSON.stringify(Boolean(token))}) headers.Authorization = ${JSON.stringify(token ? `Bearer ${token}` : "")};
    const hasBody = ${JSON.stringify(body !== undefined)};
    if (hasBody) headers["Content-Type"] = "application/json";
    const response = await fetch(${JSON.stringify(requestPath)}, {
      method: ${JSON.stringify(method)},
      headers,
      body: hasBody ? JSON.stringify(${JSON.stringify(body ?? null)}) : undefined,
    });
    const text = await response.text();
    let parsed = null;
    try { parsed = text ? JSON.parse(text) : null; } catch { parsed = text; }
    return { status: response.status, body: parsed, raw: text };
  })()`);
}
