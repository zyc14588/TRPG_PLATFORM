import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { BrowserPage, launchChrome, waitUntil } from "./browser-test.mjs";

const origin = requiredEnvironment("AR11_LIVE_ORIGIN");
const credentials = parseEnvironmentFile(
  await readFile(requiredEnvironment("AR11_CREDENTIALS_FILE"), "utf8"),
);
const tutorial = parseEnvironmentFile(
  await readFile(requiredEnvironment("AR11_TUTORIAL_FILE"), "utf8"),
);
const privateCanary = requiredEnvironment("AR11_PRIVATE_CANARY");
const runId = process.env.AR11_BROWSER_RUN_ID || "primary";
const evidenceRoot = process.env.AR11_EVIDENCE_DIRECTORY
  ? path.resolve(process.env.AR11_EVIDENCE_DIRECTORY)
  : await mkdtemp(path.join(tmpdir(), "ar11-live-browser-evidence-"));
await mkdir(evidenceRoot, { recursive: true, mode: 0o700 });

const humanCampaignId = tutorial.TUTORIAL_CAMPAIGN_ID;
const aiCampaignId = `${humanCampaignId}_ai_${runId}`;
const humanSessionId = `session_${humanCampaignId}_${runId}`;
const characterId = `investigator_${humanCampaignId}_${runId}`;
const accounts = {
  owner: {
    userId: credentials.ADMIN_USER_ID,
    login: credentials.ADMIN_LOGIN,
    password: credentials.ADMIN_PASSWORD,
  },
  keeper: {
    userId: credentials.BUSINESS_USER_ID,
    login: credentials.BUSINESS_LOGIN,
    password: credentials.BUSINESS_PASSWORD,
  },
  playerA: generatedAccount("ar11_player_a"),
  playerB: generatedAccount("ar11_player_b"),
  spectator: generatedAccount("ar11_spectator"),
};

const result = {
  browser: "Google Chrome headless via DevTools Protocol",
  composition: "production Docker Compose APIs, policy, event store, realtime, and agent worker",
  url: origin,
  viewports: ["1600x1000", "390x844"],
  screenshots: [],
  exports: [],
  checks: [],
};

let chrome;
try {
  chrome = await launchChrome(evidenceRoot);
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
  await rm(path.join(evidenceRoot, "chrome-profile"), { recursive: true, force: true });
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
  const sanityActionId = await pendingActionId(playerA);
  await submitAndWait(keeper, 'form[data-form="confirm-player-action"]', {
    actionId: sanityActionId,
    expectedVersion: "1",
  }, "行动结果已确认", 20_000);
  await clickAndWait(playerA, '[data-action="refresh-events"]', "可见事件已刷新", 20_000);
  const privateDice = findPrivateDiceEvent(playerA.responseBodies);
  assert.equal(privateDice.payload.random_source, "SERVER_OS_CSPRNG");
  assert.ok(privateDice.payload.roll_id);
  result.checks.push("HUMAN_KP Tutorial used canonical server dice on a private_to_player SAN check");

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
  result.checks.push("HUMAN_KP private Agent draft required explicit keeper approval");

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
  result.checks.push("disconnect/reconnect resumed the realtime cursor");

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
  const keeperAccessToken = await productAccessToken(keeper, accounts.keeper.userId);
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
  assert.ok(aiPlayer.websocketFrames.some((frame) => frame.includes("AgentJobRequested")));
  assert.ok(aiPlayer.websocketFrames.some((frame) => frame.includes("DecisionCommitted")));
  assert.ok(aiPlayer.websocketFrames.some((frame) => frame.includes(aiSummary)));
  assert.equal((await allClientEvidence(aiPlayer)).includes(privateCanary), false);
  assert.equal((await allClientEvidence(aiPlayer)).includes(privateDice.payload.roll_id), false);

  const decisionSequence = findWebsocketEventSequence(aiPlayer.websocketFrames, "DecisionCommitted");
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

async function assertPageIdentity(page) {
  assert.equal(await page.evaluate("location.origin"), origin);
  assert.equal(await page.evaluate("document.title"), "雾港调查台");
  await page.waitForText("进入调查台");
  assert.ok((await page.text()).length > 100);
  assert.equal(await page.hasText("Internal Server Error"), false);
  assert.equal(page.errors.length, 0, `browser console errors: ${page.errors.join(" | ")}`);
  result.checks.push("real product shell loaded without overlay or console errors");
}

async function login(page, account) {
  await page.submit('form[data-form="login"]', {
    login: account.login,
    password: account.password,
  });
  await page.waitForText("选择一场调查", 20_000);
  await assertNoAlert(page);
}

async function openCampaign(page, campaignId, expectedText) {
  await clickAndWait(
    page,
    `[data-action="open-campaign"][data-campaign-id="${campaignId}"]`,
    "战役已打开",
    20_000,
  );
  await page.waitForText(expectedText, 20_000);
}

async function issueInvite(page, userId, role) {
  const previous = await page.evaluate(
    "Array.from(document.querySelectorAll('.secret-output code')).map((node) => node.innerText).join('\\n')",
  );
  await submitAndWait(page, 'form[data-form="issue-invite"]', { userId, role }, "邀请已签发", 20_000);
  await waitUntil(async () => {
    const current = await page.evaluate(
      "Array.from(document.querySelectorAll('.secret-output code')).map((node) => node.innerText).join('\\n')",
    );
    return current && current !== previous;
  }, 20_000, "invite token did not render");
  const [inviteId, rawToken] = await page.evaluate(
    "Array.from(document.querySelectorAll('.secret-output code')).map((node) => node.innerText)",
  );
  assert.ok(inviteId && rawToken);
  return { inviteId, rawToken };
}

async function joinCampaign(account, campaignId, invite) {
  const page = await loginPage(account);
  await submitAndWait(page, 'form[data-form="accept-invite"]', {
    campaignId,
    inviteId: invite.inviteId,
    rawToken: invite.rawToken,
  }, "已加入战役", 20_000);
  return page;
}

async function loginPage(account) {
  const page = await BrowserPage.open(chrome.debugOrigin, origin);
  await login(page, account);
  return page;
}

async function submitAndWait(page, selector, values, expectedNotice, timeoutMs = 12_000) {
  const responsesBefore = page.responseBodies.length;
  await page.submit(selector, values);
  await waitUntil(async () => {
    const notice = await page.evaluate("document.querySelector('.feedback.notice')?.innerText || ''");
    const alert = await page.evaluate("document.querySelector('.feedback.error')?.innerText || ''");
    if (alert) throw new Error(alert);
    return page.responseBodies.length > responsesBefore && notice.includes(expectedNotice);
  }, timeoutMs, `operation did not complete: ${expectedNotice}`);
  await assertNoAlert(page);
}

async function clickAndWait(page, selector, expectedNotice, timeoutMs = 12_000) {
  const responsesBefore = page.responseBodies.length;
  await page.click(selector);
  await waitUntil(async () => {
    const notice = await page.evaluate("document.querySelector('.feedback.notice')?.innerText || ''");
    const alert = await page.evaluate("document.querySelector('.feedback.error')?.innerText || ''");
    if (alert) throw new Error(alert);
    return page.responseBodies.length > responsesBefore && notice.includes(expectedNotice);
  }, timeoutMs, `operation did not complete: ${expectedNotice}`);
  await assertNoAlert(page);
}

async function exportArtifact(page, audience, exportId) {
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

function validateArtifactIntegrity(raw, artifact, status) {
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

function validateExportViews(player, keeper, audit, context) {
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

async function deleteExportSubject({
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

async function browserFetch(page, requestPath, { method = "GET", token, headers = {}, body } = {}) {
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

function parsedResponses(bodies) {
  const parsed = [];
  for (const raw of bodies) {
    try {
      parsed.push({ raw, value: JSON.parse(raw) });
    } catch {}
  }
  return parsed;
}

async function productAccessToken(page, userId) {
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

async function delayUntil(unixMs) {
  const remaining = unixMs - Date.now();
  if (remaining > 0) await delay(remaining);
}

async function assertNoAlert(page) {
  const alert = await page.evaluate("document.querySelector('.feedback.error')?.innerText || ''");
  assert.equal(alert, "", alert);
}

async function pendingActionId(page) {
  const text = await page.evaluate("document.querySelector('.pending-action span')?.innerText || ''");
  const match = text.match(/^行动 ([a-zA-Z0-9_-]+) 等待确认$/);
  assert.ok(match, `pending action id not rendered: ${text}`);
  return match[1];
}

function findPrivateDiceEvent(bodies) {
  const event = responseEvents(bodies)
    .reverse()
    .find((candidate) => candidate.event_type === "DiceRolled"
      && candidate.visibility_label === "private_to_player");
  assert.ok(event, "private SAN DiceRolled event was not found in authorized replay");
  return event;
}

function responseEvents(bodies) {
  const events = [];
  for (const body of bodies) {
    try {
      const value = JSON.parse(body);
      if (Array.isArray(value?.events)) events.push(...value.events);
    } catch {}
  }
  return events;
}

function findWebsocketEventSequence(frames, eventType) {
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

function responseEvidence(page) {
  return page.responseBodies.join("\n");
}

function resetCapturedEvidence(page) {
  page.responseBodies.length = 0;
  page.websocketFrames.length = 0;
  page.networkFailures.length = 0;
  page.errors.length = 0;
  page.requests.length = 0;
}

async function allClientEvidence(page) {
  return [
    responseEvidence(page),
    page.websocketFrames.join("\n"),
    page.networkFailures.join("\n"),
    await page.text(),
    JSON.stringify(await page.evaluate("Object.fromEntries(Object.entries(localStorage))")),
  ].join("\n");
}

function generatedAccount(userId) {
  return {
    userId,
    login: `${userId.replaceAll("_", "-")}@example.invalid`,
    password: createHash("sha256")
      .update(`ar11-browser:${humanCampaignId}:${userId}`)
      .digest("hex"),
  };
}

function parseEnvironmentFile(contents) {
  return Object.fromEntries(contents.trim().split("\n").map((line) => {
    const separator = line.indexOf("=");
    if (separator <= 0) throw new Error("invalid credentials file");
    const key = line.slice(0, separator);
    let value = line.slice(separator + 1);
    if ((value.startsWith("'") && value.endsWith("'"))
      || (value.startsWith('"') && value.endsWith('"'))) {
      value = value.slice(1, -1);
    }
    return [key, value];
  }));
}

function requiredEnvironment(name) {
  const value = process.env[name];
  if (!value) throw new Error(`${name} is required`);
  return value;
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
