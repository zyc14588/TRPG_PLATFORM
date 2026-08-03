import assert from "node:assert/strict";
import { waitUntil } from "../browser-test.mjs";

export function parsedResponses(bodies) {
  const parsed = [];
  for (const raw of bodies) {
    try {
      parsed.push({ raw, value: JSON.parse(raw) });
    } catch {}
  }
  return parsed;
}

export async function productAccessToken(page, userId) {
  let response;
  await waitUntil(() => {
    response = parsedResponses(page.responseBodies)
    .map(({ value }) => value)
    .find((value) => value?.user_id === userId && typeof value?.access_token === "string");
    return Boolean(response);
  }, 5_000, `product access token response missing for ${userId}`);
  assert.ok(response, `product access token response missing for ${userId}`);
  return response.access_token;
}

export async function delayUntil(unixMs) {
  const remaining = unixMs - Date.now();
  if (remaining > 0) await delay(remaining);
}

export async function assertNoAlert(page) {
  const alert = await page.evaluate("document.querySelector('.feedback.error')?.innerText || ''");
  assert.equal(alert, "", alert);
}

export async function pendingActionId(page, previousActionId = "") {
  let actionId = "";
  await waitUntil(async () => {
    const text = await page.evaluate("document.querySelector('.pending-action span')?.innerText || ''");
    const match = text.match(/^行动 ([a-zA-Z0-9_-]+) 等待确认$/);
    actionId = match?.[1] || "";
    return Boolean(actionId && actionId !== previousActionId);
  }, 20_000, "new pending action id was not rendered");
  return actionId;
}

export function findPrivateDiceEvent(bodies) {
  const event = responseEvents(bodies)
    .reverse()
    .find((candidate) => candidate.event_type === "DiceRolled"
      && candidate.visibility_label === "private_to_player");
  assert.ok(event, "private SAN DiceRolled event was not found in authorized replay");
  return event;
}

export function responseEvents(bodies) {
  const events = [];
  for (const body of bodies) {
    try {
      const value = JSON.parse(body);
      if (Array.isArray(value?.events)) events.push(...value.events);
    } catch {}
  }
  return events;
}

export function findWebsocketEventSequence(frames, eventType) {
  for (const frame of [...frames].reverse()) {
    try {
      const value = JSON.parse(frame);
      if (value?.type === "event" && value.event?.event_type === eventType) {
        return Number(value.event.cursor || value.cursor);
      }
    } catch {}
  }
  throw new Error(`websocket event not found: ${eventType}`);
}

export function responseEvidence(page) {
  return page.responseBodies.join("\n");
}

export function resetCapturedEvidence(page) {
  page.responseBodies.length = 0;
  page.websocketFrames.length = 0;
  page.networkFailures.length = 0;
  page.errors.length = 0;
  page.requests.length = 0;
}

export async function allClientEvidence(page) {
  return [
    responseEvidence(page),
    page.websocketFrames.join("\n"),
    page.networkFailures.join("\n"),
    await page.text(),
    JSON.stringify(await page.evaluate("Object.fromEntries(Object.entries(localStorage))")),
  ].join("\n");
}

export function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
