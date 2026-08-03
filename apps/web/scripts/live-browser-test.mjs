import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { BrowserPage, launchChrome, waitUntil } from "./browser-test.mjs";
import { accounts, aiCampaignId, characterId, chrome, credentials, evidenceRoot, humanCampaignId, humanSessionId, origin, privateCanary, result, runId, setChrome, tutorial } from "./live-browser-test/context.mjs";
import { assertPageIdentity, clickAndWait, issueInvite, joinCampaign, login, openCampaign, submitAndWait } from "./live-browser-test/page-actions.mjs";
import { browserFetch, deleteExportSubject, exportArtifact, validateExportViews } from "./live-browser-test/export-and-privacy.mjs";
import { allClientEvidence, delay, delayUntil, findPrivateDiceEvent, findWebsocketEventSequence, pendingActionId, productAccessToken, resetCapturedEvidence, responseEvidence } from "./live-browser-test/support.mjs";
import { directGameplayRequest, discardExpectedFetchStatusError, waitForGameplayProjection } from "./live-browser-test/gameplay-helpers.mjs";
import { runAiTutorial } from "./live-browser-test/ai-tutorial.mjs";

const rf04RealProvider = process.env.RF04_REAL_PROVIDER === "1";
const tutorialScenarioDocument = JSON.stringify(JSON.parse(await readFile(
  new URL("../../../fixtures/scenarios/tutorial_mist_archive.scenario.json", import.meta.url),
  "utf8",
)));
const tutorialScenarioHash = `sha256:${createHash("sha256")
  .update(tutorialScenarioDocument)
  .digest("hex")}`;

try {
  setChrome(await launchChrome(evidenceRoot));
  await runTutorials();
  await writeFile(
    path.join(evidenceRoot, "live-browser-result.json"),
    `${JSON.stringify(result, null, 2)}\n`,
    { encoding: "utf8", mode: 0o600 },
  );
  console.log(`AR11_LIVE_BROWSER_RESULT=${JSON.stringify(result)}`);
  console.log(`AR11_LIVE_BROWSER_EVIDENCE=${evidenceRoot}`);
} finally {
  await chrome?.close();
  await rm(path.join(evidenceRoot, "chrome-profile"), {
    recursive: true,
    force: true,
    maxRetries: 10,
    retryDelay: 100,
  });
}
async function runTutorials() {
  const owner = await BrowserPage.open(chrome.debugOrigin, origin);
  await owner.setViewport(1600, 1000);
  await assertPageIdentity(owner);
  await login(owner, accounts.owner);
  const ownerAccessToken = await productAccessToken(owner, accounts.owner.userId);
  await owner.click('[data-action="admin"]');
  await owner.waitForText("管理与运维证据");
  await submitAndWait(owner, 'form[data-form="admin-login"]', {
    login: accounts.owner.login,
    password: accounts.owner.password,
  }, "Admin session 已建立", 20_000);
  await owner.waitForText("DIAGNOSTICS_HEALTHY", 20_000);

  for (const account of [accounts.playerA, accounts.playerB, accounts.spectator]) {
    await submitAndWait(owner, 'form[data-form="admin-create-user"]', {
      userId: account.userId,
      login: account.login,
      password: account.password,
    }, "普通用户已创建", 20_000);
  }
  result.checks.push("ServerOwner created three ordinary users through the audited Admin UI");

  const keeper = await BrowserPage.open(chrome.debugOrigin, origin);
  await keeper.setViewport(1600, 1000);
  await login(keeper, accounts.keeper);
  let keeperAccessToken = await productAccessToken(keeper, accounts.keeper.userId);
  await openCampaign(keeper, humanCampaignId, "HUMAN_KP");
  await keeper.click('[data-action="setup"]');
  await keeper.waitForText("角色卡");

  const invites = new Map();
  for (const [account, role] of [
    [accounts.playerA, "PLAYER"],
    [accounts.playerB, "PLAYER"],
    [accounts.spectator, "SPECTATOR"],
  ]) {
    invites.set(account.userId, await issueInvite(keeper, account.userId, role));
  }
  const playerA = await joinCampaign(accounts.playerA, humanCampaignId, invites.get(accounts.playerA.userId));
  const playerB = await joinCampaign(accounts.playerB, humanCampaignId, invites.get(accounts.playerB.userId));
  const spectator = await joinCampaign(accounts.spectator, humanCampaignId, invites.get(accounts.spectator.userId));
  await openCampaign(playerA, humanCampaignId, "调查员工具");
  await openCampaign(playerB, humanCampaignId, "调查员工具");
  await openCampaign(spectator, humanCampaignId, "旁观模式");
  const playerAAccessToken = await productAccessToken(playerA, accounts.playerA.userId);
  const spectatorAccessToken = await productAccessToken(spectator, accounts.spectator.userId);

  await keeper.click('[data-action="play"]');
  await submitAndWait(keeper, 'form[data-form="group"]', {
    groupId: "ar11_team_red",
    userId: accounts.playerA.userId,
  }, "分队成员已更新");
  await submitAndWait(keeper, 'form[data-form="group"]', {
    groupId: "ar11_team_blue",
    userId: accounts.playerB.userId,
  }, "分队成员已更新");
  assert.ok(responseEvidence(keeper).includes("ar11_team_red"));
  assert.ok(responseEvidence(keeper).includes("ar11_team_blue"));
  result.checks.push("two least-privilege players were assigned to distinct server-side groups");

  await playerA.click('[data-action="setup"]');
  await submitAndWait(playerA, 'form[data-form="create-character"]', {
    displayName: "AR11 调查员",
    characterId,
  }, "角色草稿已保存");
  await clickAndWait(playerA, '[data-action="submit-character"]', "角色已提交审核");
  await keeper.click('[data-action="setup"]');
  await submitAndWait(keeper, 'form[data-form="review-character"]', {
    characterId,
    expectedVersion: "2",
  }, "角色已批准");
  await submitAndWait(keeper, 'form[data-form="start-session"]', {
    sessionId: humanSessionId,
    roomId: tutorial.TUTORIAL_ROOM_ID,
    scenarioId: "tutorial_mist_archive",
    sceneId: "scene_archive_front",
    sceneKey: "scene_archive_front",
    sceneName: "灰港市政档案室前厅",
  }, "Session 已开始", 20_000);
  await keeper.waitForText("灰港市政档案室前厅");

  await playerA.click('[data-action="play"]');
  await submitAndWait(playerA, 'form[data-form="submit-action"]', {
      characterId,
      sessionId: humanSessionId,
      sceneId: "scene_archive_front",
      intentKind: "INVESTIGATION",
      skillName: "Library Use",
      clueId: "clue_wrong_signature",
      clueImportance: "CORE",
      adjustment: "NONE",
      description: "核对档案索引并检查缺页。",
  }, "行动已提交", 20_000);
  const investigationActionId = await pendingActionId(playerA);
  await keeper.click('[data-action="play"]');
  await submitAndWait(keeper, 'form[data-form="confirm-player-action"]', {
    actionId: investigationActionId,
    expectedVersion: "1",
  }, "行动结果已确认", 20_000);

  await submitAndWait(playerA, 'form[data-form="submit-action"]', {
      characterId,
      sessionId: humanSessionId,
      sceneId: "scene_archive_front",
      intentKind: "SANITY_CHECK",
      successLoss: "0",
      failureLoss: "1",
      dayKey: "ar11_tutorial_day",
      description: "面对封存记录进行私密理智检定。",
  }, "行动已提交", 20_000);
  const sanityActionId = await pendingActionId(playerA, investigationActionId);
  await submitAndWait(keeper, 'form[data-form="confirm-player-action"]', {
    actionId: sanityActionId,
    expectedVersion: "1",
  }, "行动结果已确认", 20_000);
  await clickAndWait(playerA, '[data-action="refresh-events"]', "可见事件已刷新", 20_000);
  const privateDice = findPrivateDiceEvent(playerA.responseBodies);
  assert.equal(privateDice.payload.random_source, "SERVER_OS_CSPRNG");
  assert.ok(privateDice.payload.roll_id);
  result.checks.push("HUMAN_KP Tutorial used canonical server dice on a private_to_player SAN check");

  await submitAndWait(keeper, 'form[data-form="public-gameplay"]', {
    gameplayKind: "NPC_INTERACTION",
    sessionId: humanSessionId,
    characterId,
    npcId: "npc_marta",
    approach: "询问昨夜的访客记录",
    publicResponse: "玛塔避开视线，声称昨夜没有访客。",
  }, "公开玩法结果已记录", 30_000);
  await waitForGameplayProjection(keeper, "NPC 互动", 20_000);
  await submitAndWait(keeper, 'form[data-form="switch-scene"]', {
    sessionId: humanSessionId,
    sceneId: "scene_basement",
    sceneName: "地下盐窖",
  }, "场景推进已提交", 20_000);
  await submitAndWait(keeper, 'form[data-form="public-gameplay"]', {
    gameplayKind: "COMBAT_ROUND",
    sessionId: humanSessionId,
    characterId,
    npcId: "npc_marta",
    combatActionKind: "MELEE",
    combatDefense: "DODGE",
  }, "公开玩法结果已记录", 30_000);
  await waitForGameplayProjection(keeper, "基础战斗轮", 20_000);
  await submitAndWait(keeper, 'form[data-form="public-gameplay"]', {
    gameplayKind: "CHASE_SEGMENT",
    sessionId: humanSessionId,
    characterId,
    npcId: "npc_marta",
    initialRange: "2",
    obstacleId: "collapsing_salt_shelf",
    obstacleCost: "1",
  }, "公开玩法结果已记录", 30_000);
  const humanChaseProjection = await waitForGameplayProjection(keeper, "基础追逐段", 20_000);
  assert.ok(responseEvidence(keeper).includes("SERVER_OS_CSPRNG"));

  const deniedPlayerActionId = `gameplay_denied_player_${runId}`;
  const deniedSpectatorActionId = `gameplay_denied_spectator_${runId}`;
  const deniedPlayerConsoleStart = playerA.errors.length;
  const deniedSpectatorConsoleStart = spectator.errors.length;
  const deniedPlayer = await browserFetch(
    playerA,
    `/api/api/v1/campaigns/${humanCampaignId}/gameplay-actions`,
    {
      method: "POST",
      token: playerAAccessToken,
      body: directGameplayRequest(deniedPlayerActionId, humanCampaignId, humanSessionId, characterId),
    },
  );
  const deniedSpectator = await browserFetch(
    spectator,
    `/api/api/v1/campaigns/${humanCampaignId}/gameplay-actions`,
    {
      method: "POST",
      token: spectatorAccessToken,
      body: directGameplayRequest(deniedSpectatorActionId, humanCampaignId, humanSessionId, characterId),
    },
  );
  for (const denied of [deniedPlayer, deniedSpectator]) {
    assert.equal(denied.status, 403, JSON.stringify(denied.body));
    assert.equal(denied.body.error, "PUBLIC_GAMEPLAY_AUTHORITY_FORBIDDEN");
  }
  await delay(500);
  discardExpectedFetchStatusError(playerA, deniedPlayerConsoleStart, "403 (Forbidden)");
  discardExpectedFetchStatusError(spectator, deniedSpectatorConsoleStart, "403 (Forbidden)");
  for (const page of [keeper, playerA, playerB, spectator]) {
    assert.equal(page.websocketFrames.some((frame) =>
      frame.includes(deniedPlayerActionId) || frame.includes(deniedSpectatorActionId)
    ), false);
  }
  await clickAndWait(playerA, '[data-action="refresh-events"]', "可见事件已刷新", 20_000);
  assert.equal(await waitForGameplayProjection(playerA, "基础追逐段", 20_000), humanChaseProjection);
  result.checks.push("HUMAN_KP completed NPC, combat, and chase through server rules; PLAYER and SPECTATOR direct escalation returned 403 with no WS event");

  if (!rf04RealProvider) {
    const humanJobId = `job_human_${Date.now().toString(36)}`;
    await submitAndWait(keeper, 'form[data-form="agent-job"]', {
      jobId: humanJobId,
      ragSnapshotId: "tutorial_rag_human",
      privateNote: privateCanary,
    }, "Agent 工作已请求", 20_000);
    await delay(10_000);
    await submitAndWait(keeper, 'form[data-form="approve-agent"]', {
      jobId: humanJobId,
      expectedVersion: "1",
    }, "AI 草案批准已提交", 20_000);
    await keeper.waitForText(privateCanary, 20_000);
  }
  result.checks.push(rf04RealProvider
    ? "HUMAN_KP formal gameplay remained direct and never invoked the real model"
    : "HUMAN_KP private Agent draft required explicit keeper approval");

  for (const page of [playerA, playerB, spectator]) {
    await clickAndWait(page, '[data-action="refresh-events"]', "可见事件已刷新", 20_000);
    const evidence = await allClientEvidence(page);
    assert.equal(evidence.includes(privateCanary), false);
    assert.deepEqual(await page.evaluate("Object.keys(localStorage)"), []);
  }
  assert.equal((await allClientEvidence(playerA)).includes(privateDice.payload.roll_id), true);
  for (const page of [playerB, spectator]) {
    assert.equal((await allClientEvidence(page)).includes(privateDice.payload.roll_id), false);
  }
  assert.equal(await spectator.hasText("旁观模式"), true);
  result.checks.push("unauthorized DOM, HTTP responses, WS frames, and localStorage excluded private canaries");

  const framesBeforeReconnect = playerB.websocketFrames.length;
  await playerB.click('[data-action="reconnect"]');
  await waitUntil(
    async () => playerB.websocketFrames.length > framesBeforeReconnect
      && (await playerB.hasText("已同步")),
    20_000,
    "realtime reconnect did not resume",
  );
  await clickAndWait(playerB, '[data-action="refresh-events"]', "可见事件已刷新", 20_000);
  assert.equal(await waitForGameplayProjection(playerB, "基础追逐段", 20_000), humanChaseProjection);
  result.checks.push("disconnect/reconnect resumed the realtime cursor and replayed the same chase projection");

  await submitAndWait(keeper, 'form[data-form="end-session"]', {
    sessionId: humanSessionId,
  }, "Tutorial 结局已记录，Session 已结束", 20_000);
  const humanScreenshot = path.join(evidenceRoot, "human-kp-tutorial.png");
  await keeper.evaluate("scrollTo(0, 0)");
  await keeper.screenshot(humanScreenshot);
  result.screenshots.push(humanScreenshot);
  result.checks.push("HUMAN_KP Tutorial completed from an empty browser session");

  await runAiTutorial({
    owner,
    ownerAccessToken,
    keeper,
    keeperAccessToken,
    playerA,
    playerB,
    spectator,
    privateDice,
    rf04RealProvider,
    tutorialScenarioDocument,
    tutorialScenarioHash,
  });
}
