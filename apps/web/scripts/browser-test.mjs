import assert from "node:assert/strict";
import { createHash, randomBytes } from "node:crypto";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

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
    await rm(path.join(evidenceRoot, "chrome-profile"), { recursive: true, force: true });
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

class ProductMock {
  constructor(dist) {
    this.dist = dist;
    this.server = createServer((request, response) => void this.handle(request, response));
    this.server.on("upgrade", (request, socket) => this.upgrade(request, socket));
    this.events = new Map([
      ["campaign_human", [event(1, "SceneSwitched", "party_visible", { scene_name: "档案室 · 雾夜" })]],
      ["campaign_ai", [event(10, "SceneSwitched", "party_visible", { scene_name: "潮汐观测站" })]],
    ]);
    this.clients = new Set();
    this.connections = new Map();
    this.groups = new Map();
    this.requests = [];
  }

  async start() {
    await new Promise((resolve, reject) => {
      this.server.once("error", reject);
      this.server.listen(0, "127.0.0.1", resolve);
    });
    const address = this.server.address();
    this.origin = `http://127.0.0.1:${address.port}`;
  }

  close() {
    for (const client of this.clients) client.socket.destroy();
    return new Promise((resolve) => this.server.close(resolve));
  }

  connectionCount(token) {
    return this.connections.get(token) || 0;
  }

  disconnectToken(token) {
    for (const client of this.clients) {
      if (client.token === token) client.socket.destroy();
    }
  }

  async handle(request, response) {
    try {
      const url = new URL(request.url, this.origin || "http://localhost");
      this.requests.push(`${request.method} ${url.pathname}`);
      if (url.pathname === "/config.json") return this.file(response, "config.json", "application/json");
      if (!url.pathname.startsWith("/api/") && !url.pathname.startsWith("/admin/")) {
        const relative = url.pathname === "/" ? "index.html" : url.pathname.replace(/^\//, "");
        const type = relative.endsWith(".css") ? "text/css" : relative.endsWith(".js") ? "text/javascript" : "text/html";
        return this.file(response, relative, type);
      }
      const token = bearer(request.headers.authorization);
      const body = await requestBody(request);
      const reply = (status, payload) => json(response, status, payload);

      if (request.method === "POST" && url.pathname === "/api/auth/login") {
        const session = USERS[body.login];
        if (!session) return reply(401, { error: "INVALID_CREDENTIALS" });
        return reply(200, {
          access_token: session.token,
          token_type: "Bearer",
          expires_at_unix_ms: Date.now() + 60_000,
          user_id: session.userId,
          global_role: session.globalRole,
        });
      }
      if (request.method === "POST" && url.pathname === "/api/auth/logout") return reply(204, {});
      if (request.method === "POST" && url.pathname === "/admin/admin/v1/sessions") {
        if (body.login !== "admin@example.test") return reply(403, { error: "ADMIN_SERVER_OWNER_REQUIRED" });
        return reply(200, { access_token: "admin_browser_token", expires_at_unix_ms: Date.now() + 60_000 });
      }
      if (url.pathname === "/admin/admin/v1/bootstrap/status") {
        return reply(200, { state: "deployment_ready", state_version: 7 });
      }
      if (url.pathname === "/admin/admin/v1/diagnostics") return reply(200, { status: "deployment_ready", services: ["api", "realtime", "agent"] });
      if (url.pathname === "/admin/admin/v1/audit") return reply(200, { records: [{ action: "diagnostics.read", decision: "PERMIT" }] });

      const user = userByToken(token);
      if (!user) return reply(401, { error: "SESSION_REQUIRED" });
      if (request.method === "GET" && url.pathname === "/api/api/v1/campaigns") {
        return reply(200, { campaigns: CAMPAIGNS.filter((campaign) => user.roles[campaign.campaign_id]) });
      }
      let match = url.pathname.match(/^\/api\/api\/v1\/campaigns\/([^/]+)$/);
      if (request.method === "GET" && match) return reply(200, campaign(match[1]));
      match = url.pathname.match(/^\/api\/campaigns\/([^/]+)\/authority$/);
      if (request.method === "GET" && match) return reply(200, authority(match[1]));
      match = url.pathname.match(/^\/api\/campaigns\/([^/]+)\/membership$/);
      if (request.method === "GET" && match) return reply(200, { campaign_id: match[1], user_id: user.userId, role: user.roles[match[1]] });
      match = url.pathname.match(/^\/api\/campaigns\/([^/]+)\/events$/);
      if (request.method === "GET" && match) {
        const visible = (this.events.get(match[1]) || []).filter((item) => this.visible(item, user));
        return reply(200, { campaign_id: match[1], events: visible, scanned_through_sequence: this.lastCursor(match[1]) });
      }

      match = url.pathname.match(/^\/api\/campaigns\/([^/]+)\/groups\/([^/]+)$/);
      if (request.method === "POST" && match) return reply(201, { campaign_id: match[1], group_id: match[2] });
      match = url.pathname.match(/^\/api\/campaigns\/([^/]+)\/groups\/([^/]+)\/memberships\/([^/]+)$/);
      if (request.method === "PUT" && match) {
        const [, campaignId, groupId, userId] = match;
        this.groups.set(`${campaignId}:${userId}`, groupId);
        const recorded = this.record(campaignId, "SplitPartyClueRevealed", "private_to_group", { summary: "红队发现封存索引", canary: "CANARY_GROUP_RED" }, groupId);
        this.broadcast(campaignId, recorded);
        return reply(200, { campaign_id: campaignId, group_id: groupId, user_id: userId });
      }

      match = url.pathname.match(/^\/api\/api\/v1\/campaigns\/([^/]+)\/(characters|sessions|player-actions|agent-jobs|reconsiderations|exports)(?:\/.*)?$/);
      if (match) {
        const campaignId = match[1];
        const resource = match[2];
        if (resource === "player-actions" && request.method === "POST" && !url.pathname.endsWith("/confirm")) {
          const submitted = this.record(campaignId, "PlayerActionSubmitted", "party_visible", { summary: "调查行动已提交" });
          this.broadcast(campaignId, submitted);
          return reply(202, { state: "PENDING_CONFIRMATION", aggregate_version: 1, first_event_sequence: submitted.sequence, last_event_sequence: submitted.sequence });
        }
        if (resource === "player-actions" && url.pathname.endsWith("/confirm")) {
          const recorded = this.record(campaignId, "PlayerActionResolved", "party_visible", { summary: "服务端检定结果已确认" });
          this.broadcast(campaignId, recorded);
          return reply(200, { state: "RESOLVED", aggregate_version: 2 });
        }
        if (resource === "agent-jobs" && !url.pathname.endsWith("/approve")) {
          if (campaignId === "campaign_ai" && body.input?.kind === "player_action") {
            const requested = this.record(campaignId, "AgentJobRequested", "party_visible", {
              model_id: "local-mistral-coc7",
              route_authorization_event_id: "route_authorization_browser_1",
            });
            this.broadcast(campaignId, requested);
            const decision = this.record(campaignId, "DecisionCommitted", "party_visible", {
              DecisionCommitted: {
                player_visible_text: "建议先核对潮汐日志，再检查钟楼地下入口。",
                linked_records: ["DecisionRecord", "GameEvent", "ToolResult"],
              },
            });
            this.broadcast(campaignId, decision);
            return reply(202, { state: "REQUESTED", job_id: body.job_id, input_event_sequence: requested.sequence });
          }
          const recorded = this.record(campaignId, "SecretRollResolved", "keeper_only", { summary: "私密检定已记录", canary: "CANARY_PRIVATE" });
          this.broadcast(campaignId, recorded);
          return reply(202, { state: "REQUESTED", job_id: body.job_id, input_event_sequence: recorded.sequence });
        }
        if (resource === "exports") {
          if (request.method === "GET") return reply(200, { export_id: url.pathname.split("/").at(-1), campaign_id: campaignId, state: "REQUESTED", audience: "CAMPAIGN_ARCHIVE" });
          return reply(202, { aggregate_version: 1, last_event_sequence: this.lastCursor(campaignId) });
        }
        if (resource === "reconsiderations") {
          const recorded = this.record(campaignId, "ReconsiderationRequested", "party_visible", { summary: "重考虑请求已记录" });
          this.broadcast(campaignId, recorded);
          return reply(202, { aggregate_version: 1, last_event_sequence: recorded.sequence });
        }
        const eventType = resource === "characters"
          ? url.pathname.endsWith("/submit") ? "CharacterSubmitted" : url.pathname.endsWith("/review") ? "CharacterReviewed" : "CharacterCreated"
          : resource === "sessions" ? url.pathname.endsWith("/scenes") ? "SceneSwitched" : request.method === "PATCH" ? "SessionPaused" : "SessionStarted"
          : "CommandRecorded";
        const recorded = this.record(campaignId, eventType, "party_visible", { scene_name: body.scene_name || body.next_scene_name, summary: `${eventType} 已记录` });
        this.broadcast(campaignId, recorded);
        return reply(url.pathname.includes("/submit") ? 202 : 201, { aggregate_version: 1 + Number(url.pathname.includes("/submit") || url.pathname.includes("/review")), last_event_sequence: recorded.sequence });
      }
      return reply(404, { error: "NOT_FOUND" });
    } catch (error) {
      json(response, 500, { error: `MOCK_FAILURE_${error.message}` });
    }
  }

  async file(response, relative, type) {
    const normalized = path.normalize(relative);
    if (normalized.startsWith("..")) return json(response, 404, { error: "NOT_FOUND" });
    try {
      const data = await readFile(path.join(this.dist, normalized));
      response.writeHead(200, { "Content-Type": type, "Cache-Control": "no-store" });
      response.end(data);
    } catch {
      json(response, 404, { error: "NOT_FOUND" });
    }
  }

  upgrade(request, socket) {
    const url = new URL(request.url, this.origin);
    const match = url.pathname.match(/^\/realtime\/ws\/v1\/campaigns\/([^/]+)\/rooms\/([^/]+)$/);
    const protocols = String(request.headers["sec-websocket-protocol"] || "").split(",").map((value) => value.trim());
    const authProtocol = protocols.find((value) => value.startsWith("trpg.auth."));
    const token = authProtocol?.slice("trpg.auth.".length);
    const user = userByToken(token);
    if (!match || !protocols.includes("trpg.realtime.v1") || !user) {
      socket.end("HTTP/1.1 401 Unauthorized\r\nConnection: close\r\n\r\n");
      return;
    }
    const accept = createHash("sha1").update(`${request.headers["sec-websocket-key"]}258EAFA5-E914-47DA-95CA-C5AB0DC85B11`).digest("base64");
    socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\nSec-WebSocket-Protocol: trpg.realtime.v1\r\n\r\n`);
    const client = { socket, token, user, campaignId: match[1], buffer: Buffer.alloc(0), sequence: 0 };
    this.clients.add(client);
    this.connections.set(token, this.connectionCount(token) + 1);
    socket.on("data", (chunk) => this.receive(client, chunk));
    socket.on("close", () => this.clients.delete(client));
    socket.on("error", () => this.clients.delete(client));
  }

  receive(client, chunk) {
    client.buffer = Buffer.concat([client.buffer, chunk]);
    while (true) {
      const parsed = parseClientFrame(client.buffer);
      if (!parsed) return;
      client.buffer = client.buffer.subarray(parsed.bytes);
      if (parsed.opcode === 8) return client.socket.end();
      if (parsed.opcode !== 1) continue;
      const message = JSON.parse(parsed.payload.toString("utf8"));
      if (message.type === "subscribe") {
        this.send(client, { type: "connected", binding: { connection_id: `browser_${randomBytes(4).toString("hex")}`, tenant_id: "tenant_browser", user_id: client.user.userId, campaign_id: client.campaignId, seat: client.user.roles[client.campaignId].toLowerCase(), authority_mode: authority(client.campaignId).mode.toLowerCase(), authority_epoch: 1 }, heartbeat_interval_ms: 15_000 });
        this.send(client, { type: "subscribed", request_id: message.request_id, subscription: message.subscription, cursor: message.cursor || 0, resume_token: "resume_browser_safe" });
        for (const item of this.events.get(client.campaignId) || []) {
          if (item.sequence > (message.cursor || 0) && this.visible(item, client.user)) this.send(client, { type: "event", cursor: item.sequence, event: realtimeEvent(item, client.campaignId) });
        }
        this.send(client, { type: "checkpoint", cursor: this.lastCursor(client.campaignId), resume_token: "resume_browser_safe" });
      } else if (message.type === "ack") {
        this.send(client, { type: "acked", request_id: message.request_id, cursor: message.cursor, resume_token: "resume_browser_safe" });
      }
    }
  }

  send(client, message) {
    client.sequence += 1;
    client.socket.write(serverFrame(JSON.stringify({ version: "trpg.realtime.v1", sequence: client.sequence, ...message })));
  }

  broadcast(campaignId, item) {
    for (const client of this.clients) {
      if (client.campaignId === campaignId && this.visible(item, client.user)) {
        this.send(client, { type: "event", cursor: item.sequence, event: realtimeEvent(item, campaignId) });
        this.send(client, { type: "checkpoint", cursor: item.sequence, resume_token: "resume_browser_safe" });
      }
    }
  }

  visible(item, user) {
    if (["public", "party_visible"].includes(item.visibility_label)) return true;
    if (item.visibility_label === "keeper_only") return user.roles.campaign_human === "HUMAN_KEEPER";
    if (item.visibility_label === "private_to_group") return this.groups.get(`campaign_human:${user.userId}`) === item.visibility_subject;
    return false;
  }

  record(campaignId, eventType, visibilityLabel, payload, visibilitySubject = null) {
    const recorded = event(this.lastCursor(campaignId) + 1, eventType, visibilityLabel, payload, visibilitySubject);
    this.events.get(campaignId).push(recorded);
    return recorded;
  }

  lastCursor(campaignId) {
    return (this.events.get(campaignId) || []).at(-1)?.sequence || 0;
  }
}

export class BrowserPage {
  static async open(debugOrigin, url) {
    const target = await fetch(`${debugOrigin}/json/new?${encodeURIComponent(url)}`, { method: "PUT" }).then((response) => response.json());
    const page = new BrowserPage(target.webSocketDebuggerUrl);
    await page.ready;
    await Promise.all([
      page.send("Page.enable"),
      page.send("Runtime.enable"),
      page.send("Network.enable"),
      page.send("Log.enable"),
    ]);
    page.on("Runtime.exceptionThrown", (event) => page.errors.push(event.exceptionDetails?.text || "runtime exception"));
    page.on("Log.entryAdded", (event) => {
      if (["error", "warning"].includes(event.entry.level)) page.errors.push(event.entry.text);
    });
    page.on("Network.responseReceived", (event) => page.responses.set(event.requestId, event.response.url));
    page.on("Network.requestWillBeSent", (event) => {
      page.requests.push(`${event.request.method} ${new URL(event.request.url).pathname}`);
    });
    page.on("Network.loadingFailed", (event) => page.networkFailures.push(`${event.errorText}:${event.blockedReason || ""}`));
    page.on("Network.loadingFinished", async (event) => {
      if (!page.responses.has(event.requestId)) return;
      try {
        const body = await page.send("Network.getResponseBody", { requestId: event.requestId });
        page.responseBodies.push(body.body || "");
      } catch {}
    });
    page.on("Network.webSocketFrameReceived", (event) => page.websocketFrames.push(event.response.payloadData || ""));
    await page.waitForText("进入调查台");
    return page;
  }

  constructor(websocketUrl) {
    this.socket = new WebSocket(websocketUrl);
    this.pending = new Map();
    this.listeners = new Map();
    this.nextId = 0;
    this.errors = [];
    this.requests = [];
    this.responses = new Map();
    this.responseBodies = [];
    this.networkFailures = [];
    this.websocketFrames = [];
    this.ready = new Promise((resolve, reject) => {
      this.socket.addEventListener("open", resolve, { once: true });
      this.socket.addEventListener("error", reject, { once: true });
    });
    this.socket.addEventListener("message", (event) => {
      const message = JSON.parse(event.data);
      if (message.id) {
        const pending = this.pending.get(message.id);
        this.pending.delete(message.id);
        if (message.error) pending?.reject(new Error(message.error.message));
        else pending?.resolve(message.result || {});
      } else {
        for (const listener of this.listeners.get(message.method) || []) listener(message.params || {});
      }
    });
  }

  send(method, params = {}) {
    const id = ++this.nextId;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.socket.send(JSON.stringify({ id, method, params }));
    });
  }

  on(method, listener) {
    const listeners = this.listeners.get(method) || [];
    listeners.push(listener);
    this.listeners.set(method, listeners);
  }

  async evaluate(expression) {
    const result = await this.send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
    if (result.exceptionDetails) {
      throw new Error(
        result.exceptionDetails.exception?.description || result.exceptionDetails.text,
      );
    }
    return result.result?.value;
  }

  text() {
    return this.evaluate("document.body.innerText");
  }

  async hasText(text) {
    return (await this.text()).includes(text);
  }

  async waitForText(text, timeout = 8_000) {
    await waitUntil(() => this.hasText(text), timeout, `text not found: ${text}`);
  }

  click(selector) {
    return this.evaluate(`(() => { const element = document.querySelector(${JSON.stringify(selector)}); if (!element) throw new Error('missing selector'); element.click(); })()`);
  }

  submit(selector, values = {}) {
    return this.evaluate(`(() => {
      const form = document.querySelector(${JSON.stringify(selector)});
      if (!form) throw new Error('missing form ${selector}');
      const values = ${JSON.stringify(values)};
      for (const [name, value] of Object.entries(values)) {
        const field = form.elements.namedItem(name);
        if (!field) throw new Error('missing field ' + name);
        field.value = value;
        field.dispatchEvent(new Event('input', { bubbles: true }));
        field.dispatchEvent(new Event('change', { bubbles: true }));
      }
      form.requestSubmit();
    })()`);
  }

  setViewport(width, height) {
    return this.send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: width < 600 });
  }

  async navigate(url) {
    const loaded = new Promise((resolve) => {
      const handler = () => resolve();
      this.on("Page.loadEventFired", handler);
    });
    await this.send("Page.navigate", { url });
    await Promise.race([loaded, timeout(8_000, "navigation timeout")]);
  }

  async screenshot(destination) {
    const result = await this.send("Page.captureScreenshot", { format: "png", fromSurface: true });
    await writeFile(destination, Buffer.from(result.data, "base64"));
  }

  async pressTab() {
    await this.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Tab", code: "Tab", windowsVirtualKeyCode: 9 });
    await this.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Tab", code: "Tab", windowsVirtualKeyCode: 9 });
  }
}

export async function launchChrome(outputRoot) {
  const profile = path.join(outputRoot, "chrome-profile");
  const executable = process.env.CHROME_BIN || "/usr/bin/google-chrome";
  const certificatePin = process.env.AR11_CERTIFICATE_SPKI;
  const child = spawn(executable, [
    "--headless=new",
    "--disable-gpu",
    "--disable-dev-shm-usage",
    "--no-first-run",
    "--no-default-browser-check",
    ...(certificatePin ? [`--ignore-certificate-errors-spki-list=${certificatePin}`] : []),
    `--user-data-dir=${profile}`,
    "--remote-debugging-port=0",
    "about:blank",
  ], { stdio: ["ignore", "ignore", "pipe"] });
  let stderr = "";
  const debugUrl = await Promise.race([
    new Promise((resolve, reject) => {
      child.stderr.setEncoding("utf8");
      child.stderr.on("data", (chunk) => {
        stderr += chunk;
        const match = stderr.match(/DevTools listening on (ws:\/\/[^\s]+)/);
        if (match) resolve(match[1]);
      });
      child.once("exit", (code) => reject(new Error(`Chrome exited ${code}: ${stderr}`)));
    }),
    timeout(10_000, "Chrome DevTools endpoint timeout"),
  ]);
  const parsed = new URL(debugUrl);
  return {
    child,
    debugOrigin: `http://${parsed.host}`,
    async close() {
      child.kill("SIGTERM");
      await Promise.race([new Promise((resolve) => child.once("exit", resolve)), new Promise((resolve) => setTimeout(resolve, 2_000))]);
      if (child.exitCode === null) child.kill("SIGKILL");
    },
  };
}

export async function waitUntil(check, waitMs = 8_000, message = "condition timeout") {
  const deadline = Date.now() + waitMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      if (await check()) return;
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 80));
  }
  throw lastError || new Error(message);
}

function timeout(waitMs, message) {
  return new Promise((_, reject) => setTimeout(() => reject(new Error(message)), waitMs));
}

function requestBody(request) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    request.on("data", (chunk) => chunks.push(chunk));
    request.on("end", () => {
      if (!chunks.length) return resolve({});
      try { resolve(JSON.parse(Buffer.concat(chunks).toString("utf8"))); } catch (error) { reject(error); }
    });
    request.on("error", reject);
  });
}

function json(response, status, payload) {
  const body = JSON.stringify(payload);
  response.writeHead(status, { "Content-Type": "application/json", "Content-Length": Buffer.byteLength(body), "Cache-Control": "no-store" });
  response.end(body);
}

function bearer(value = "") {
  return value.startsWith("Bearer ") ? value.slice(7) : "";
}

function userByToken(token) {
  return Object.values(USERS).find((user) => user.token === token);
}

function campaign(campaignId) {
  const value = CAMPAIGNS.find((item) => item.campaign_id === campaignId);
  if (!value) throw new Error("CAMPAIGN_NOT_FOUND");
  return value;
}

function authority(campaignId) {
  const ai = campaignId === "campaign_ai";
  return {
    contract_id: `authority_${campaignId}`,
    campaign_id: campaignId,
    mode: ai ? "AI_KP" : "HUMAN_KP",
    authority_owner: ai ? "ai_keeper_orchestrator" : "keeper_01",
    version: 1,
    created_at_unix_ms: ai ? 1_700_000_000_001 : 1_700_000_000_000,
    locked: true,
    change_policy: "FORK_ONLY",
    snapshot: {
      ruleset_version: "coc7_rules_1",
      house_rules_version: "house_rules_none_1",
      scenario_version: "tutorial_mist_archive_1",
      prompt_version: "keeper_prompt_1",
      agent_pack_version: "keeper_agent_pack_1",
      tool_schema_version: "tool_schema_1",
      safety_profile_version: "safety_profile_1",
      ai_provider_snapshot: "local-mistral-coc7",
      model_route_snapshot: "route_level4_1",
      character_sheet_template_version: "coc7_investigator_1",
    },
  };
}

function event(sequence, eventType, visibilityLabel, payload, visibilitySubject = null) {
  return {
    sequence,
    cursor: sequence,
    stream_version: sequence,
    event_type: eventType,
    event_schema_version: 1,
    resource_type: "scene",
    resource_id: "scene_browser",
    visibility_label: visibilityLabel,
    visibility_subject: visibilitySubject,
    provenance_kind: eventType.startsWith("Agent") ? "agent_output" : "user_statement",
    provenance_reference: `reference_${sequence}`,
    provenance_recorded_by: "browser_fixture",
    payload,
  };
}

function realtimeEvent(item, campaignId) {
  return {
    cursor: item.sequence,
    stream_version: item.stream_version,
    event_type: item.event_type,
    event_schema_version: 1,
    campaign_id: campaignId,
    resource_type: item.resource_type,
    resource_id: item.resource_id,
    authority_mode: authority(campaignId).mode.toLowerCase(),
    authority_epoch: 1,
    visibility_label: item.visibility_label,
    visibility_subject: item.visibility_subject,
    provenance_kind: item.provenance_kind,
    provenance_reference: item.provenance_reference,
    provenance_recorded_by: item.provenance_recorded_by,
    correlation_id: `correlation_${item.sequence}`,
    causation_id: `causation_${item.sequence}`,
    trace_id: `trace_${item.sequence}`,
    payload: item.payload,
  };
}

function parseClientFrame(buffer) {
  if (buffer.length < 2) return null;
  const opcode = buffer[0] & 0x0f;
  const masked = (buffer[1] & 0x80) !== 0;
  let length = buffer[1] & 0x7f;
  let offset = 2;
  if (length === 126) {
    if (buffer.length < 4) return null;
    length = buffer.readUInt16BE(2);
    offset = 4;
  } else if (length === 127) {
    if (buffer.length < 10) return null;
    const large = buffer.readBigUInt64BE(2);
    if (large > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error("frame too large");
    length = Number(large);
    offset = 10;
  }
  const maskBytes = masked ? 4 : 0;
  if (buffer.length < offset + maskBytes + length) return null;
  const payload = Buffer.from(buffer.subarray(offset + maskBytes, offset + maskBytes + length));
  if (masked) {
    const mask = buffer.subarray(offset, offset + 4);
    for (let index = 0; index < payload.length; index += 1) payload[index] ^= mask[index % 4];
  }
  return { opcode, payload, bytes: offset + maskBytes + length };
}

function serverFrame(text) {
  const payload = Buffer.from(text);
  if (payload.length < 126) return Buffer.concat([Buffer.from([0x81, payload.length]), payload]);
  const header = Buffer.alloc(4);
  header[0] = 0x81;
  header[1] = 126;
  header.writeUInt16BE(payload.length, 2);
  return Buffer.concat([header, payload]);
}

const CAMPAIGNS = [
  { campaign_id: "campaign_human", owner_user_id: "keeper_01", authority_contract_id: "authority_campaign_human", title: "灰港档案室", state: "ACTIVE", aggregate_version: 1, last_event_sequence: 1 },
  { campaign_id: "campaign_ai", owner_user_id: "ai_owner", authority_contract_id: "authority_campaign_ai", title: "潮汐下的钟声", state: "ACTIVE", aggregate_version: 1, last_event_sequence: 10 },
];

const USERS = {
  "keeper@example.test": { token: "token_keeper", userId: "keeper_01", globalRole: "USER", roles: { campaign_human: "HUMAN_KEEPER", campaign_ai: "HUMAN_KEEPER" } },
  "player-b@example.test": { token: "token_player_b", userId: "player_b", globalRole: "USER", roles: { campaign_human: "PLAYER" } },
  "spectator@example.test": { token: "token_spectator", userId: "spectator_01", globalRole: "USER", roles: { campaign_human: "SPECTATOR" } },
  "ai-player@example.test": { token: "token_ai_player", userId: "ai_player", globalRole: "USER", roles: { campaign_ai: "PLAYER" } },
};

if (path.resolve(process.argv[1] || "") === fileURLToPath(import.meta.url)) {
  await main();
}
