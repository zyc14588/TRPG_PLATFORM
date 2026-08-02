import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { BrowserPage, launchChrome, waitUntil } from "./browser-test/browser-driver.mjs";
import { ProductMock } from "./browser-test/product-mock.mjs";

export { BrowserPage, launchChrome, waitUntil } from "./browser-test/browser-driver.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
let mock;
let result;

async function main() {
  const evidenceRoot = await mkdtemp(path.join(tmpdir(), "ar11-browser-evidence-"));
  mock = new ProductMock(path.join(root, "dist"));
  await mock.start();
  const chrome = await launchChrome(evidenceRoot);

  result = {
    browser: "Google Chrome headless via DevTools Protocol",
    url: mock.origin,
    viewports: ["1600x1000", "390x844"],
    screenshots: [],
    checks: [],
  };

  try {
  const keeper = await BrowserPage.open(chrome.debugOrigin, mock.origin);
  await keeper.setViewport(1600, 1000);
  await assertPageIdentity(keeper);
  await login(keeper, "keeper@example.test");
  await openCampaign(keeper, "campaign_human");
  await keeper.waitForText("KP 工具");
  result.checks.push("HUMAN_KP workspace rendered");

  const playerB = await BrowserPage.open(chrome.debugOrigin, mock.origin);
  await login(playerB, "player-b@example.test");
  await openCampaign(playerB, "campaign_human");
  await playerB.waitForText("调查员工具");

  const spectator = await BrowserPage.open(chrome.debugOrigin, mock.origin);
  await login(spectator, "spectator@example.test");
  await openCampaign(spectator, "campaign_human");
  await spectator.waitForText("旁观模式");
  result.checks.push("two players and spectator sessions rendered");

  await keeper.click('[data-action="setup"]');
  await keeper.waitForText("角色卡");
  await keeper.submit('form[data-form="create-character"]');
  await keeper.waitForText("角色草稿已保存");
  await keeper.click('[data-action="submit-character"]');
  await keeper.waitForText("角色已提交审核");
  await keeper.submit('form[data-form="review-character"]', {
    characterId: "character_browser",
    expectedVersion: "2",
  });
  await keeper.waitForText("角色已批准");
  await keeper.submit('form[data-form="start-session"]');
  await keeper.waitForText("档案室 · 雾夜");
  await keeper.submit('form[data-form="submit-action"]');
  await keeper.waitForText("等待确认");
  await keeper.click('[data-action="confirm-action"]');
  await keeper.waitForText("行动结果已确认");
  result.checks.push("character -> review -> Session -> action -> confirmation completed");

  for (const [gameplayKind, expected] of [
    ["NPC_INTERACTION", "与档案管理员玛塔的互动已形成正式记录"],
    ["COMBAT_ROUND", "基础战斗轮已结算"],
    ["CHASE_SEGMENT", "基础追逐段已结算"],
  ]) {
    await keeper.submit('form[data-form="public-gameplay"]', { gameplayKind });
    await keeper.waitForText(expected);
    await waitUntil(
      () => keeper.evaluate('document.querySelector("#app")?.getAttribute("aria-busy") === "false"'),
      8_000,
      `${gameplayKind} submission did not settle`,
    );
  }
  assert.equal(
    await playerB.evaluate('document.querySelectorAll("form[data-form=\\"public-gameplay\\"]").length'),
    0,
  );
  assert.equal(
    await spectator.evaluate('document.querySelectorAll("form[data-form=\\"public-gameplay\\"]").length'),
    0,
  );
  for (const token of ["token_player_b", "token_spectator"]) {
    const denied = await fetch(`${mock.origin}/api/api/v1/campaigns/campaign_human/gameplay-actions`, {
      method: "POST",
      headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
      body: JSON.stringify({
        command: { expected_version: 0 },
        campaign_id: "campaign_human",
        session_id: "session_browser",
        action_id: "gameplay_escalation",
        action: { kind: "COMBAT_ROUND" },
      }),
    });
    assert.equal(denied.status, 403);
  }
  result.checks.push("HUMAN_KP NPC/combat/chase completed and PLAYER/SPECTATOR escalation failed");

  await keeper.submit('form[data-form="group"]', {
    groupId: "team_red",
    userId: "player_b",
  });
  await keeper.waitForText("分队成员已更新");
  await playerB.waitForText("红队发现封存索引");
  assert.equal(await spectator.hasText("红队发现封存索引"), false);

  await keeper.submit('form[data-form="agent-job"]');
  await keeper.waitForText("Agent 工作已请求");
  await keeper.waitForText("私密检定已记录");
  assert.equal(await spectator.hasText("私密检定已记录"), false);
  result.checks.push("split-party and keeper-only events remained server-filtered");

  const playerConnectionsBefore = mock.connectionCount("token_player_b");
  mock.disconnectToken("token_player_b");
  await playerB.waitForText("正在重连");
  await waitUntil(() => mock.connectionCount("token_player_b") > playerConnectionsBefore);
  await playerB.waitForText("已同步");
  await playerB.waitForText("基础追逐段已结算");
  result.checks.push("disconnect -> cursor resume -> synced completed");

  const desktopPath = path.join(evidenceRoot, "human-kp-desktop.png");
  await keeper.evaluate("scrollTo(0, 0)");
  await keeper.screenshot(desktopPath);
  result.screenshots.push(desktopPath);

  const aiPlayer = await BrowserPage.open(chrome.debugOrigin, mock.origin);
  await login(aiPlayer, "ai-player@example.test");
  await openCampaign(aiPlayer, "campaign_ai");
  await aiPlayer.waitForText("AI_KP · 用户可见");
  const aiActionRequestStart = mock.requests.length;
  await aiPlayer.submit('form[data-form="submit-action"]', {
    characterId: "character_ai_player",
    sessionId: "session_ai",
    sceneId: "scene_tide",
  });
  await aiPlayer.waitForText("建议先核对潮汐日志");
  await aiPlayer.waitForText("服务器路由已授权");
  const aiActionRequests = mock.requests.slice(aiActionRequestStart);
  assert.equal(
    aiActionRequests.includes("POST /api/api/v1/campaigns/campaign_ai/agent-jobs"),
    true,
    `AI_KP player action must enter Agent Gateway: ${aiActionRequests.join(" | ")}`,
  );
  for (const [intentKind, expected] of [
    ["NPC_INTERACTION", "AI NPC 互动已记录"],
    ["COMBAT_ROUND", "AI 基础战斗轮已结算"],
    ["CHASE_SEGMENT", "AI 基础追逐段已结算"],
  ]) {
    const requestStart = mock.requests.length;
    await aiPlayer.submit('form[data-form="submit-action"]', { intentKind });
    await aiPlayer.waitForText(expected);
    await waitUntil(
      () => aiPlayer.evaluate('document.querySelector("#app")?.getAttribute("aria-busy") === "false"'),
      8_000,
      `${intentKind} Agent Job submission did not settle`,
    );
    const requests = mock.requests.slice(requestStart);
    assert.ok(requests.includes("POST /api/api/v1/campaigns/campaign_ai/agent-jobs"));
    assert.equal(requests.some((request) => request.includes("/gameplay-actions")), false);
  }
  result.checks.push("AI_KP NPC/combat/chase remained behind Agent Gateway and projected tool results");
  assert.equal(
    aiActionRequests.includes("POST /api/api/v1/campaigns/campaign_ai/player-actions"),
    false,
    `AI_KP player action must not enter HUMAN_KP workflow: ${aiActionRequests.join(" | ")}`,
  );
  await aiPlayer.submit('form[data-form="reconsider"]', {
    eventSequence: "21",
    reason: "请按已公开的现场时间重新核对。",
  });
  await waitUntil(async () => {
    const notice = await aiPlayer.evaluate("document.querySelector('.feedback.notice')?.innerText || ''");
    const alert = await aiPlayer.evaluate("document.querySelector('.feedback.error')?.innerText || ''");
    if (alert) throw new Error(alert);
    return notice.includes("重考虑请求已提交");
  }, 8_000, "reconsideration did not complete");
  result.checks.push("ordinary PLAYER AI action -> Agent Gateway -> canonical event -> WS -> user-visible explanation completed");

  const aiManager = await BrowserPage.open(chrome.debugOrigin, mock.origin);
  await login(aiManager, "keeper@example.test");
  await openCampaign(aiManager, "campaign_ai");
  await aiManager.waitForText("KP 工具");
  assert.equal(
    await aiManager.evaluate("document.querySelectorAll('form[data-form=\"reconsider\"]').length"),
    1,
  );
  result.checks.push("AI_KP campaign manager can submit reconsideration from the keeper rail");

  await keeper.click('[data-action="admin"]');
  await keeper.waitForText("管理与运维证据");
  await keeper.submit('form[data-form="admin-login"]', {
    login: "admin@example.test",
    password: "admin password long enough",
  });
  await keeper.waitForText("deployment_ready");
  await keeper.click('[data-action="admin-audit"]');
  await keeper.waitForText("diagnostics.read");
  result.checks.push("Admin diagnostics and audit evidence loaded through Admin API");

  await aiPlayer.click('[data-action="developer"]');
  await aiPlayer.waitForText("Agent / Tool / Event 证据");
  assert.equal(await aiPlayer.hasText("CANARY_REASONING"), false);
  result.checks.push("Developer evidence rendered with reasoning/prompt redaction");

  for (const page of [keeper, playerB, spectator, aiPlayer, aiManager]) {
    assert.deepEqual(await page.evaluate("Object.keys(localStorage)"), []);
    assert.equal(await page.evaluate("document.querySelectorAll('input,select,textarea').length === document.querySelectorAll('label input,label select,label textarea').length"), true);
    assert.equal(page.errors.length, 0, `browser console errors: ${page.errors.join(" | ")}`);
  }

  const spectatorBodies = spectator.responseBodies.join("\n");
  const spectatorFrames = spectator.websocketFrames.join("\n");
  const spectatorDom = await spectator.text();
  for (const evidence of [spectatorBodies, spectatorFrames, spectatorDom]) {
    assert.equal(evidence.includes("CANARY_PRIVATE"), false);
    assert.equal(evidence.includes("CANARY_GROUP_RED"), false);
    assert.equal(evidence.includes("CANARY_REASONING"), false);
  }
  result.checks.push("unauthorized DOM, network responses, WS frames, and localStorage contain no canary");

  await aiPlayer.setViewport(390, 844);
  await aiPlayer.navigate(mock.origin);
  await aiPlayer.waitForText("进入调查台");
  await aiPlayer.evaluate("scrollTo(0, 0)");
  const mobileMetrics = await aiPlayer.evaluate("({width: innerWidth, scrollWidth: document.documentElement.scrollWidth})");
  assert.equal(mobileMetrics.width, 390);
  assert.ok(mobileMetrics.scrollWidth <= 391, `mobile overflow: ${JSON.stringify(mobileMetrics)}`);
  await aiPlayer.pressTab();
  assert.notEqual(await aiPlayer.evaluate("document.activeElement.tagName"), "BODY");
  const mobilePath = path.join(evidenceRoot, "login-mobile.png");
  await aiPlayer.screenshot(mobilePath);
  result.screenshots.push(mobilePath);
  result.checks.push("mobile viewport, keyboard focus, labels, and horizontal overflow passed");

  const invalid = await BrowserPage.open(chrome.debugOrigin, mock.origin);
  await invalid.submit('form[data-form="login"]', {
    login: "invalid@example.test",
    password: "wrong password",
  });
  await invalid.waitForText("凭据无效，请重新检查");
  assert.equal(
    invalid.errors.every((error) => error.includes("401 (Unauthorized)")),
    true,
    `invalid-login unexpected console errors: ${invalid.errors.join(" | ")}`,
  );
  result.checks.push("invalid credential error state rendered without application exceptions");

  console.log(`AR11_BROWSER_RESULT=${JSON.stringify(result)}`);
  console.log(`AR11_BROWSER_EVIDENCE=${evidenceRoot}`);
  } finally {
    await chrome.close();
    await mock.close();
    await rm(path.join(evidenceRoot, "chrome-profile"), {
      recursive: true,
      force: true,
      maxRetries: 10,
      retryDelay: 100,
    });
  }
}

async function assertPageIdentity(page) {
  assert.equal(await page.evaluate("location.origin"), mock.origin);
  assert.equal(await page.evaluate("document.title"), "雾港调查台");
  await page.waitForText("进入调查台");
  assert.ok((await page.text()).length > 100);
  assert.equal(await page.hasText("Internal Server Error"), false);
  assert.equal(page.errors.length, 0, `browser console errors: ${page.errors.join(" | ")}`);
  result.checks.push("page identity, non-blank shell, no overlay, and console health passed");
}

async function login(page, loginName) {
  await page.submit('form[data-form="login"]', {
    login: loginName,
    password: "browser test password long enough",
  });
  try {
    await page.waitForText("选择一场调查");
  } catch (error) {
    const dom = (await page.text()).slice(0, 2_000);
    const responses = page.responseBodies.slice(-5).map((body) => body.slice(0, 500));
    throw new Error(`${error.message}\nDOM=${dom}\nCONSOLE=${page.errors.join(" | ")}\nNETWORK=${page.networkFailures.slice(-5).join(" | ")}\nREQUESTS=${mock.requests.slice(-10).join(" | ")}\nRESPONSES=${responses.join(" | ")}`);
  }
}

async function openCampaign(page, campaignId) {
  await page.click(`[data-campaign-id="${campaignId}"]`);
  await page.waitForText(campaignId === "campaign_ai" ? "潮汐下的钟声" : "灰港档案室");
}


if (path.resolve(process.argv[1] || "") === fileURLToPath(import.meta.url)) {
  await main();
}
