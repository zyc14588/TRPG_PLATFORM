import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { BrowserPage, launchChrome, waitUntil } from "./browser-test.mjs";
import { accounts, aiCampaignId, characterId, chrome, credentials, evidenceRoot, humanCampaignId, humanSessionId, origin, privateCanary, result, runId, setChrome, tutorial } from "./live-browser-test/context.mjs";
import { assertPageIdentity, clickAndWait, issueInvite, joinCampaign, login, openCampaign, submitAndWait } from "./live-browser-test/page-actions.mjs";
import { browserFetch, deleteExportSubject, exportArtifact, validateExportViews } from "./live-browser-test/export-and-privacy.mjs";
import { allClientEvidence, delay, delayUntil, findPrivateDiceEvent, findWebsocketEventSequence, pendingActionId, productAccessToken, resetCapturedEvidence, responseEvidence } from "./live-browser-test/support.mjs";

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

  await submitAndWait(owner, 'form[data-form="admin-fork-authority"]', {
    parentCampaignId: humanCampaignId,
    childCampaignId: aiCampaignId,
    authorityMode: "AI_KP",
    authorityOwner: "ai_keeper_tutorial",
    campaignManagerUserId: accounts.keeper.userId,
  }, "Authority 子分支已派生", 20_000);

  await keeper.click('[data-action="logout"]');
  await keeper.waitForText("进入调查台");
  resetCapturedEvidence(keeper);
  await login(keeper, accounts.keeper);
  keeperAccessToken = await productAccessToken(keeper, accounts.keeper.userId);
  await submitAndWait(keeper, 'form[data-form="create-campaign"]', {
    campaignId: aiCampaignId,
    title: "AR11 AI Keeper Tutorial",
    roomId: `room_${aiCampaignId}`,
    roomName: "AI Tutorial Room",
    parentCampaignId: humanCampaignId,
    sourceSessionId: humanSessionId,
    forkReason: "从完成的 HUMAN_KP Tutorial 派生 AI_KP 教学分支",
  }, "战役已创建", 30_000);
  await openCampaign(keeper, aiCampaignId, "AI_KP");

  const aiSessionId = `session_${aiCampaignId}_${runId}`;
  const aiForkId = `fork_${aiCampaignId}`;
  const aiCharacterId = forkChildId(aiForkId, "character", characterId);
  const aiScenarioId = forkChildId(aiForkId, "scenario", "rf04_tutorial_scenario");
  const aiFrontSceneId = forkChildId(aiForkId, "scene", "rf04_archive_front");
  const aiBasementSceneId = forkChildId(aiForkId, "scene", "rf04_basement");
  if (rf04RealProvider) {
    const scenarioNonce = `scenario_import_${runId}_${Date.now().toString(36)}`;
    const scenarioImport = await browserFetch(
      keeper,
      `/api/api/v1/campaigns/${aiCampaignId}/scenarios/import`,
      {
        method: "POST",
        token: keeperAccessToken,
        body: {
          command: wireCommand(scenarioNonce),
          campaign_id: aiCampaignId,
          scenario_id: aiScenarioId,
          ruleset_id: "coc7",
          format_version: "0.1.0",
          content_hash: tutorialScenarioHash,
          document_json: tutorialScenarioDocument,
        },
      },
    );
    assert.equal(scenarioImport.status, 201, JSON.stringify(scenarioImport.body));
    await keeper.click('[data-action="setup"]');
    await submitAndWait(keeper, 'form[data-form="start-session"]', {
      sessionId: aiSessionId,
      roomId: `room_${aiCampaignId}`,
      scenarioId: aiScenarioId,
      sceneId: aiFrontSceneId,
      sceneKey: "scene_archive_front",
      sceneName: "灰港市政档案室前厅",
    }, "Session 已开始", 30_000);
  }

  await keeper.click('[data-action="setup"]');
  const aiInvite = await issueInvite(keeper, accounts.playerA.userId, "PLAYER");
  const aiPlayer = await joinCampaign(accounts.playerA, aiCampaignId, aiInvite);
  const aiPlayerAccessToken = await productAccessToken(aiPlayer, accounts.playerA.userId);
  await aiPlayer.setViewport(1600, 1000);
  await openCampaign(aiPlayer, aiCampaignId, "AI_KP · 用户可见");
  await aiPlayer.waitForText("调查员工具");
  assert.equal(
    await aiPlayer.evaluate("document.querySelectorAll('form[data-form=\"agent-job\"]').length"),
    0,
  );

  let decisionSequence;
  if (rf04RealProvider) {
    await submitAiGameplay(aiPlayer, {
      campaignId: aiCampaignId,
      characterId: aiCharacterId,
      sessionId: aiSessionId,
      sceneId: aiFrontSceneId,
      intentKind: "NPC_INTERACTION",
      description: "询问昨夜的访客记录",
      expectedProjection: "NPC 互动",
    });
    const switchNonce = `scene_switch_${runId}_${Date.now().toString(36)}`;
    const switched = await browserFetch(
      keeper,
      `/api/api/v1/campaigns/${aiCampaignId}/sessions/${aiSessionId}/scenes`,
      {
        method: "POST",
        token: keeperAccessToken,
        body: {
          command: wireCommand(switchNonce, 1),
          campaign_id: aiCampaignId,
          session_id: aiSessionId,
          next_scene_id: aiBasementSceneId,
          next_scene_key: "scene_basement",
          next_scene_name: "地下盐窖",
          switched_at_unix_ms: Date.now(),
        },
      },
    );
    assert.equal(switched.status, 201, JSON.stringify(switched.body));
    await submitAiGameplay(aiPlayer, {
      campaignId: aiCampaignId,
      characterId: aiCharacterId,
      sessionId: aiSessionId,
      sceneId: aiBasementSceneId,
      intentKind: "COMBAT_ROUND",
      description: "用近战阻止玛塔逃走",
      expectedProjection: "基础战斗轮",
    });
    const aiChaseProjection = await submitAiGameplay(aiPlayer, {
      campaignId: aiCampaignId,
      characterId: aiCharacterId,
      sessionId: aiSessionId,
      sceneId: aiBasementSceneId,
      intentKind: "CHASE_SEGMENT",
      description: "越过倒塌的盐架追赶玛塔",
      expectedProjection: "基础追逐段",
    });
    assert.ok(aiPlayer.websocketFrames.some((frame) => frame.includes("AgentJobRequested")));
    assert.ok(aiPlayer.websocketFrames.some((frame) => frame.includes("ToolExecutionSucceeded")));
    assert.ok(aiPlayer.websocketFrames.some((frame) => frame.includes("DecisionCommitted")));
    assert.ok(aiPlayer.websocketFrames.some((frame) => frame.includes("SERVER_OS_CSPRNG")));
    const framesBeforeAiReconnect = aiPlayer.websocketFrames.length;
    await aiPlayer.click('[data-action="reconnect"]');
    await waitUntil(
      async () => aiPlayer.websocketFrames.length > framesBeforeAiReconnect
        && (await aiPlayer.hasText("已同步")),
      20_000,
      "AI_KP realtime reconnect did not resume",
    );
    await clickAndWait(aiPlayer, '[data-action="refresh-events"]', "可见事件已刷新", 20_000);
    assert.equal(await waitForGameplayProjection(aiPlayer, "基础追逐段", 20_000), aiChaseProjection);
    result.checks.push("AI_KP completed NPC, combat, and chase only through Agent Gateway and preserved projection after reconnect/replay");
    decisionSequence = findWebsocketEventSequence(aiPlayer.websocketFrames, "DecisionCommitted");
  } else {
    const aiSummary = "AI Keeper 建议先核对档案索引，再检查封存书库的门锁。";
    const aiActionRequestStart = aiPlayer.requests.length;
    await submitAndWait(aiPlayer, 'form[data-form="submit-action"]', {
      characterId,
      sessionId: humanSessionId,
      sceneId: "scene_archive_front",
      intentKind: "INVESTIGATION",
      skillName: "Library Use",
      clueId: "clue_wrong_signature",
      clueImportance: "CORE",
      adjustment: "NONE",
      description: "核对档案索引并检查封存书库的门锁。",
    }, "行动已提交", 20_000);
    await aiPlayer.waitForText(aiSummary, 60_000);
    const aiActionRequests = aiPlayer.requests.slice(aiActionRequestStart);
    assert.ok(aiActionRequests.some((request) => request === `POST /api/api/v1/campaigns/${aiCampaignId}/agent-jobs`));
    assert.equal(aiActionRequests.some((request) => request.includes("/player-actions")), false);
    assert.ok(aiPlayer.websocketFrames.some((frame) => frame.includes(aiSummary)));
    decisionSequence = findWebsocketEventSequence(aiPlayer.websocketFrames, "DecisionCommitted");
  }
  assert.equal((await allClientEvidence(aiPlayer)).includes(privateCanary), false);
  assert.equal((await allClientEvidence(aiPlayer)).includes(privateDice.payload.roll_id), false);

  await submitAndWait(aiPlayer, 'form[data-form="reconsider"]', {
    eventSequence: String(decisionSequence),
    reason: "请按已公开的档案时间重新核对。",
  }, "重考虑请求已提交", 20_000);
  result.checks.push("ordinary PLAYER completed AI_KP action -> Agent runtime -> canonical event -> WS -> UI -> reconsideration");

  const playerExportId = `export_player_${runId}`;
  const keeperExportId = `export_keeper_${runId}`;
  const auditExportId = `export_audit_${runId}`;
  await aiPlayer.click('[data-action="evidence"]');
  const playerExport = await exportArtifact(aiPlayer, "PLAYER", playerExportId);
  await keeper.click('[data-action="evidence"]');
  const keeperExport = await exportArtifact(keeper, "KEEPER_PRIVATE", keeperExportId);
  const auditExport = await exportArtifact(keeper, "AUDIT", auditExportId);
  validateExportViews(playerExport, keeperExport, auditExport, {
    campaignId: aiCampaignId,
    parentCampaignId: humanCampaignId,
    playerId: accounts.playerA.userId,
    sourceSessionId: humanSessionId,
  });
  assert.equal(playerExport.raw.includes(privateDice.payload.roll_id), false);
  result.exports = [playerExport, keeperExport, auditExport].map(({ status }) => ({
    export_id: status.export_id,
    audience: status.audience,
    artifact_schema: status.artifact_schema,
    visibility_policy_version: status.visibility_policy_version,
    artifact_hash: status.artifact_hash,
    manifest_hash: status.manifest_hash,
    artifact_size: status.artifact_size,
    first_event_sequence: status.first_event_sequence,
    last_exported_event_sequence: status.last_exported_event_sequence,
    event_count: status.event_count,
    fork_id: status.fork_id,
    parent_campaign_id: status.parent_campaign_id,
  }));

  const expiringAuthorization = await browserFetch(
    aiPlayer,
    `/api/api/v1/campaigns/${aiCampaignId}/exports/${playerExportId}/download-authorizations`,
    { method: "POST", token: aiPlayerAccessToken },
  );
  assert.equal(expiringAuthorization.status, 201);
  assert.match(expiringAuthorization.body.token, /^[a-f0-9]{64}$/);
  assert.ok(expiringAuthorization.body.expires_at_unix_ms > Date.now());
  assert.ok(expiringAuthorization.body.expires_at_unix_ms <= Date.now() + 60_500);
  assert.equal(
    aiPlayer.requests.some((request) => request.includes("token=")),
    false,
    "one-time export credentials must never enter request URLs or proxy logs",
  );
  result.checks.push("PLAYER, KEEPER_PRIVATE, and AUDIT exports passed hash, provenance, visibility, authorization, and one-use checks");

  const aiScreenshot = path.join(evidenceRoot, "ai-kp-tutorial.png");
  await aiPlayer.evaluate("scrollTo(0, 0)");
  await aiPlayer.screenshot(aiScreenshot);
  result.screenshots.push(aiScreenshot);

  await keeper.click('[data-action="developer"]');
  await keeper.waitForText("Agent / Tool / Event 证据");
  assert.equal(await keeper.hasText(privateCanary), false);

  for (const page of [owner, keeper, playerA, playerB, spectator, aiPlayer]) {
    assert.equal(
      await page.evaluate("document.querySelectorAll('input,select,textarea').length === document.querySelectorAll('label input,label select,label textarea').length"),
      true,
    );
    assert.deepEqual(await page.evaluate("Object.keys(localStorage)"), []);
    assert.equal(
      page.errors.filter((error) => !error.includes("401 (Unauthorized)")).length,
      0,
      `browser console errors: ${page.errors.join(" | ")}`,
    );
  }

  const hiddenKeeperStatus = await browserFetch(
    aiPlayer,
    `/api/api/v1/campaigns/${aiCampaignId}/exports/${keeperExportId}`,
    { token: aiPlayerAccessToken },
  );
  assert.equal(hiddenKeeperStatus.status, 404);
  const reusedDownload = await browserFetch(
    aiPlayer,
    `/api/api/v1/campaigns/${aiCampaignId}/exports/${playerExportId}/download`,
    {
      token: aiPlayerAccessToken,
      headers: { "X-TRPG-Export-Authorization": playerExport.authorization.token },
    },
  );
  assert.equal(reusedDownload.status, 404);

  const invalid = await BrowserPage.open(chrome.debugOrigin, origin);
  await invalid.setViewport(390, 844);
  await invalid.submit('form[data-form="login"]', {
    login: "invalid-ar11@example.invalid",
    password: "invalid password long enough",
  });
  await invalid.waitForText("凭据无效，请重新检查", 20_000);
  assert.equal(
    invalid.errors.every((error) => error.includes("401 (Unauthorized)")),
    true,
    `invalid login emitted unexpected errors: ${invalid.errors.join(" | ")}`,
  );
  const metrics = await invalid.evaluate("({width: innerWidth, scrollWidth: document.documentElement.scrollWidth})");
  assert.equal(metrics.width, 390);
  assert.ok(metrics.scrollWidth <= 391, `mobile overflow: ${JSON.stringify(metrics)}`);
  await invalid.pressTab();
  assert.notEqual(await invalid.evaluate("document.activeElement.tagName"), "BODY");
  const mobileScreenshot = path.join(evidenceRoot, "login-error-mobile.png");
  await invalid.screenshot(mobileScreenshot);
  result.screenshots.push(mobileScreenshot);
  result.checks.push("keyboard focus, labels, responsive layout, and invalid-credential state passed");

  await delayUntil(expiringAuthorization.body.expires_at_unix_ms + 250);
  const expiredDownload = await browserFetch(
    aiPlayer,
    `/api/api/v1/campaigns/${aiCampaignId}/exports/${playerExportId}/download`,
    {
      token: aiPlayerAccessToken,
      headers: { "X-TRPG-Export-Authorization": expiringAuthorization.body.token },
    },
  );
  assert.equal(expiredDownload.status, 404);

  await deleteExportSubject({
    requester: aiPlayer,
    requesterAccessToken: aiPlayerAccessToken,
    owner,
    ownerAccessToken,
    keeper,
    keeperAccessToken,
    campaignId: aiCampaignId,
    subjectId: accounts.playerA.userId,
    exportIds: [playerExportId, keeperExportId, auditExportId],
  });
  result.checks.push("expired download authorization failed and privacy erasure destroyed every subject-bound export artifact");
}

async function waitForGameplayProjection(page, expectedText, timeoutMs) {
  let projection = "";
  await waitUntil(async () => {
    projection = await page.evaluate(
      "document.querySelector('[data-testid=\"gameplay-result\"]')?.innerText || ''",
    );
    return projection.includes(expectedText);
  }, timeoutMs, `gameplay projection did not render: ${expectedText}`);
  return projection;
}

function discardExpectedFetchStatusError(page, start, statusText) {
  const observed = page.errors.slice(start);
  const unexpected = observed.filter((error) =>
    !(
      error.startsWith("Failed to load resource:")
      && error.includes(`status of ${statusText}`)
    )
  );
  page.errors.splice(start, observed.length, ...unexpected);
}

async function submitAiGameplay(page, {
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

function directGameplayRequest(actionId, campaignId, sessionId, submittedCharacterId) {
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

function wireCommand(nonce, expectedVersion = 0) {
  return {
    command_id: `${nonce}_command`,
    idempotency_key: `${nonce}_idempotency`,
    expected_version: expectedVersion,
    correlation_id: `${nonce}_correlation`,
    causation_id: `${nonce}_causation`,
    trace_id: `${nonce}_trace`,
  };
}

function forkChildId(forkId, kind, sourceId) {
  const digest = createHash("sha256").update(`${forkId}:${kind}:${sourceId}`).digest("hex");
  return `${kind}_${digest.slice(0, 32)}`;
}
