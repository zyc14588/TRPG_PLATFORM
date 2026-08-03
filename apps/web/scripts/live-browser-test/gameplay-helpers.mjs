import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { waitUntil } from "../browser-test.mjs";
import { submitAndWait } from "./page-actions.mjs";

export async function waitForGameplayProjection(page, expectedText, timeoutMs) {
  let projection = "";
  await waitUntil(async () => {
    projection = await page.evaluate(
      "document.querySelector('[data-testid=\"gameplay-result\"]')?.innerText || ''",
    );
    return projection.includes(expectedText);
  }, timeoutMs, `gameplay projection did not render: ${expectedText}`);
  return projection;
}

export function discardExpectedFetchStatusError(page, start, statusText) {
  const observed = page.errors.slice(start);
  const unexpected = observed.filter((error) =>
    !(
      error.startsWith("Failed to load resource:")
      && error.includes(`status of ${statusText}`)
    )
  );
  page.errors.splice(start, observed.length, ...unexpected);
}

export async function submitAiGameplay(page, {
  campaignId,
  characterId: submittedCharacterId,
  sessionId,
  sceneId,
  intentKind,
  description,
  expectedProjection,
}) {
  const requestStart = page.requests.length;
  await submitAndWait(page, 'form[data-form="submit-action"]', {
    characterId: submittedCharacterId,
    sessionId,
    sceneId,
    intentKind,
    npcId: "npc_marta",
    combatActionKind: "MELEE",
    combatDefense: "DODGE",
    initialRange: "2",
    obstacleId: "collapsing_salt_shelf",
    obstacleCost: "1",
    description,
  }, "行动已提交", 30_000);
  const projection = await waitForGameplayProjection(page, expectedProjection, 230_000);
  const requests = page.requests.slice(requestStart);
  assert.ok(requests.some((request) =>
    request === `POST /api/api/v1/campaigns/${campaignId}/agent-jobs`
  ));
  assert.equal(requests.some((request) => request.includes("/player-actions")), false);
  assert.equal(requests.some((request) => request.includes("/gameplay-actions")), false);
  return projection;
}

export function directGameplayRequest(actionId, campaignId, sessionId, submittedCharacterId) {
  const nonce = `${actionId}_${Date.now().toString(36)}`;
  return {
    command: wireCommand(nonce),
    campaign_id: campaignId,
    session_id: sessionId,
    action_id: actionId,
    action: {
      kind: "COMBAT_ROUND",
      character_id: submittedCharacterId,
      npc_id: "npc_marta",
      action_kind: "MELEE",
      defense: "DODGE",
    },
  };
}

export function wireCommand(nonce, expectedVersion = 0) {
  return {
    command_id: `${nonce}_command`,
    idempotency_key: `${nonce}_idempotency`,
    expected_version: expectedVersion,
    correlation_id: `${nonce}_correlation`,
    causation_id: `${nonce}_causation`,
    trace_id: `${nonce}_trace`,
  };
}

export function forkChildId(forkId, kind, sourceId) {
  const digest = createHash("sha256").update(`${forkId}:${kind}:${sourceId}`).digest("hex");
  return `${kind}_${digest.slice(0, 32)}`;
}
