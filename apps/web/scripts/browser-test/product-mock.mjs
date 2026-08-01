import { createHash, randomBytes } from "node:crypto";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import path from "node:path";

import {
  CAMPAIGNS, USERS, authority, bearer, campaign, event, json, parseClientFrame,
  realtimeEvent, requestBody, serverFrame, userByToken,
} from "./product-fixtures.mjs";

export class ProductMock {
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
