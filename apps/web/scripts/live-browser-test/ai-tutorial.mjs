import assert from "node:assert/strict";
import path from "node:path";
import { BrowserPage, waitUntil } from "../browser-test.mjs";
import {
  accounts,
  aiCampaignId,
  characterId,
  chrome,
  evidenceRoot,
  humanCampaignId,
  humanSessionId,
  origin,
  privateCanary,
  result,
  runId,
} from "./context.mjs";
import { clickAndWait, issueInvite, joinCampaign, login, openCampaign, submitAndWait } from "./page-actions.mjs";
import { browserFetch, deleteExportSubject, exportArtifact, validateExportViews } from "./export-and-privacy.mjs";
import {
  allClientEvidence,
  delayUntil,
  findWebsocketEventSequence,
  productAccessToken,
  resetCapturedEvidence,
} from "./support.mjs";
import {
  forkChildId,
  submitAiGameplay,
  waitForGameplayProjection,
  wireCommand,
} from "./gameplay-helpers.mjs";

export async function runAiTutorial({
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
}) {
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
