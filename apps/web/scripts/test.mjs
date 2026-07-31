import assert from "node:assert/strict";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

class TestWebSocket {
  static instances = [];

  constructor(url, protocols) {
    this.url = url;
    this.protocols = protocols;
    this.readyState = 0;
    this.listeners = new Map();
    this.sent = [];
    TestWebSocket.instances.push(this);
  }

  addEventListener(name, listener) {
    const listeners = this.listeners.get(name) || [];
    listeners.push(listener);
    this.listeners.set(name, listeners);
  }

  emit(name, value) {
    if (name === "open") this.readyState = 1;
    for (const listener of this.listeners.get(name) || []) listener(value);
  }

  send(value) {
    this.sent.push(value);
  }

  close() {
    this.readyState = 3;
  }
}

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const config = JSON.parse(await readFile(path.join(root, "dist/config.json"), "utf8"));
const { inspectServiceHealth } = await import(
  new URL("../dist/src/health.js", import.meta.url)
);
const { ProductApi, ProductApiError, createCommand } = await import(
  new URL("../dist/src/api.js", import.meta.url)
);
const { RealtimeClient, REALTIME_PROTOCOL, websocketUrl } = await import(
  new URL("../dist/src/realtime.js", import.meta.url)
);
const { decisionPresentation, eventPresentation, redactEvidence, safeJson } = await import(
  new URL("../dist/src/presentation.js", import.meta.url)
);

assert.equal(config.services.length, 5);
assert.equal(new Set(config.services.map(({ url }) => url)).size, 5);
assert.deepEqual(
  [config.apiBase, config.v1Base, config.realtimeBase, config.adminBase],
  ["/api", "/api/api/v1", "/realtime", "/admin/admin/v1"],
);
assert.deepEqual(
  decisionPresentation(
    [
      {
        sequence: 8,
        event_type: "AgentJobRequested",
        payload: {
          model_id: "local-model-1",
          route_authorization_event_id: "route-authorization-1",
        },
      },
      {
        sequence: 9,
        event_type: "DecisionCommitted",
        payload: {
          DecisionCommitted: {
            player_visible_text: "先核对港口日志。",
            linked_records: ["DecisionRecord", "GameEvent"],
            private_prompt: "CANARY_PROMPT",
          },
        },
      },
    ],
    { snapshot: { ai_provider_snapshot: "fallback-model" } },
  ),
  {
    summary: "先核对港口日志。",
    basis: ["DecisionRecord", "GameEvent"],
    model: "local-model-1",
    certification: "服务器路由已授权",
    eventSequence: 9,
  },
);

const command = createCommand("character create", 3);
assert.equal(command.expected_version, 3);
for (const value of Object.values(command)) {
  if (typeof value === "string") assert.match(value, /^[a-z0-9_]+$/);
}

const requests = [];
async function fakeFetch(url, init) {
  assert.equal(this, globalThis, "fetch implementation must retain its global receiver");
  requests.push({ url, init });
  if (url.endsWith("/auth/login")) {
    return jsonResponse(200, {
      access_token: "a".repeat(64),
      expires_at_unix_ms: Date.now() + 60_000,
      user_id: "investigator_01",
      global_role: "USER",
    });
  }
  if (url.endsWith("/admin/v1/sessions")) {
    return jsonResponse(200, { access_token: "c".repeat(64) });
  }
  if (url.endsWith("/admin/v1/bootstrap/status")) {
    return jsonResponse(200, { status: "completed", state_version: 7 });
  }
  if (url.endsWith("/admin/v1/users")) {
    return jsonResponse(201, { result: "USER_CREATED", state_version: 8 });
  }
  if (url.endsWith("/admin/v1/authority-forks")) {
    return jsonResponse(201, { result: "AUTHORITY_FORKED", state_version: 9 });
  }
  if (url.endsWith("/campaigns")) return jsonResponse(200, { campaigns: [] });
  return jsonResponse(403, { error: "CAMPAIGN_MEMBERSHIP_DENIED" });
}
const productApi = new ProductApi(config, fakeFetch);
const session = await productApi.login("investigator@example.test", "password long enough");
assert.equal(session.userId, "investigator_01");
await productApi.listCampaigns();
assert.equal(requests[0].init.headers.has("Authorization"), false);
assert.equal(requests[1].init.headers.get("Authorization"), `Bearer ${"a".repeat(64)}`);
assert.equal(requests[1].url, "/api/api/v1/campaigns");
await assert.rejects(
  () => productApi.getAuthority("campaign_a"),
  (error) => error instanceof ProductApiError && error.status === 403,
);
await productApi.adminLogin("owner@example.test", "password long enough");
assert.equal((await productApi.adminStatus()).state_version, 7);
await productApi.adminCreateUser(
  { user_id: "player_02", login: "player-02@example.test", password: "password long enough" },
  7,
);
const userRequest = requests.at(-1);
assert.equal(userRequest.url, "/admin/admin/v1/users");
assert.equal(userRequest.init.headers.get("Authorization"), `Bearer ${"c".repeat(64)}`);
assert.equal(userRequest.init.headers.get("X-Expected-Version"), "7");
assert.match(userRequest.init.headers.get("Idempotency-Key"), /^web-user-create-/);
await productApi.adminForkAuthority(
  {
    parent_campaign_id: "campaign_human",
    child_campaign_id: "campaign_ai",
    authority_mode: "AI_KP",
    authority_owner: "ai_keeper_tutorial",
    campaign_manager_user_id: "keeper_01",
  },
  8,
);
const authorityRequest = requests.at(-1);
assert.equal(authorityRequest.url, "/admin/admin/v1/authority-forks");
assert.equal(authorityRequest.init.headers.get("X-Expected-Version"), "8");
await assert.rejects(
  () => new ProductApi({ ...config, apiBase: "//outside.example" }, fakeFetch).login("user", "password"),
  (error) => error instanceof ProductApiError && error.code === "CONFIG_APIBASE_NOT_SAME_ORIGIN",
);
assert.throws(
  () => websocketUrl("//outside.example", "campaign_a", "room_a"),
  /REALTIME_BASE_NOT_SAME_ORIGIN/,
);

const redacted = redactEvidence({
  summary: "safe",
  chain_of_thought: "CANARY_REASONING",
  nested: { token: "CANARY_TOKEN", fact: "visible" },
});
assert.deepEqual(redacted, { summary: "safe", nested: { fact: "visible" } });
assert.equal(safeJson(redacted).includes("CANARY"), false);
assert.deepEqual(
  eventPresentation({
    cursor: 7,
    event_type: "AgentDecisionProduced",
    visibility_label: "party_visible",
    provenance_kind: "agent_output",
    payload: { user_visible_summary: "检查档案索引", private_prompt: "CANARY_PROMPT" },
  }),
  {
    sequence: 7,
    type: "AgentDecisionProduced",
    title: "AI 决策已生成",
    summary: "检查档案索引",
    visibility: "party_visible",
    provenance: "agent_output",
    payload: { user_visible_summary: "检查档案索引" },
  },
);
assert.equal(
  eventPresentation({
    cursor: 8,
    event_type: "AgentDraftApproved",
    visibility_label: "keeper_only",
    provenance_kind: "human_keeper_statement",
    payload: {
      approved_by: "keeper_01",
      decision: {
        player_visible_text: "仅当前 Keeper 可见的草案结果",
        private_prompt: "CANARY_PROMPT",
      },
    },
  }).summary,
  "仅当前 Keeper 可见的草案结果",
);

const realtimeStatuses = [];
const realtimeEvents = [];
const realtime = new RealtimeClient({
  baseUrl: "/realtime",
  WebSocketClass: TestWebSocket,
  onStatus: (status) => realtimeStatuses.push(status),
  onEvent: (event) => realtimeEvents.push(event),
});
realtime.connect({ token: "b".repeat(64), campaignId: "campaign_a" });
const socket = TestWebSocket.instances.at(-1);
assert.equal(socket.protocols[0], REALTIME_PROTOCOL);
assert.equal(socket.protocols[1], `trpg.auth.${"b".repeat(64)}`);
assert.equal(socket.url.includes("b".repeat(64)), false);
socket.emit("open", {});
assert.deepEqual(JSON.parse(socket.sent[0]).subscription, {
  kind: "campaign",
  room_id: "campaign_a",
});
socket.emit("message", {
  data: JSON.stringify({
    version: REALTIME_PROTOCOL,
    sequence: 1,
    type: "connected",
    binding: { user_id: "investigator_01" },
    heartbeat_interval_ms: 15_000,
  }),
});
socket.emit("message", {
  data: JSON.stringify({
    version: REALTIME_PROTOCOL,
    sequence: 2,
    type: "event",
    cursor: 9,
    event: { cursor: 9, event_type: "SceneSwitched" },
  }),
});
assert.equal(realtimeEvents.length, 1);
assert.equal(JSON.parse(socket.sent.at(-1)).type, "ack");
assert.equal(realtimeStatuses.some(({ state }) => state === "connected"), true);
socket.emit("message", {
  data: JSON.stringify({
    version: REALTIME_PROTOCOL,
    sequence: 3,
    type: "checkpoint",
    cursor: 12,
    resume_token: "resume_cursor_12",
  }),
});
socket.emit("message", {
  data: JSON.stringify({
    version: REALTIME_PROTOCOL,
    sequence: 4,
    type: "acked",
    request_id: "ack_cursor_9",
    cursor: 9,
    resume_token: "resume_cursor_9",
  }),
});
const connectionCountBeforeReconnect = TestWebSocket.instances.length;
realtime.reconnect();
assert.equal(socket.readyState, 3);
assert.equal(TestWebSocket.instances.length, connectionCountBeforeReconnect + 1);
socket.emit("close", { code: 1000 });
assert.equal(TestWebSocket.instances.length, connectionCountBeforeReconnect + 1);
const resumedSocket = TestWebSocket.instances.at(-1);
resumedSocket.emit("open", {});
assert.equal(JSON.parse(resumedSocket.sent[0]).cursor, 12);
assert.equal(JSON.parse(resumedSocket.sent[0]).resume_token, "resume_cursor_12");
realtime.connect({ token: "b".repeat(64), campaignId: "campaign_b" });
const nextCampaignSocket = TestWebSocket.instances.at(-1);
nextCampaignSocket.emit("open", {});
assert.equal(JSON.parse(nextCampaignSocket.sent[0]).cursor, 0);
assert.equal(JSON.parse(nextCampaignSocket.sent[0]).resume_token, null);
realtime.disconnect();

const builtIndex = await readFile(path.join(root, "dist/index.html"), "utf8");
const builtApplication = await readFile(path.join(root, "dist/src/app.js"), "utf8");
assert.match(builtIndex, /Content-Security-Policy/);
assert.equal(/\son[a-z]+\s*=/.test(builtIndex), false);
assert.match(builtApplication, /角色卡/);
assert.match(builtApplication, /AI_KP/);
assert.match(builtApplication, /Admin/);
assert.equal(builtApplication.includes("localStorage"), false);
assert.equal(builtApplication.includes("footer.innerHTML"), false);

const services = await Promise.all(
  config.services.map(({ name }, index) => startService(name, `0.1.${index}`, true)),
);
try {
  const health = await Promise.all(
    services.map(({ name, url }) => inspectServiceHealth({ name, url })),
  );
  assert.equal(health.filter(({ healthy }) => healthy).length, 5);
  assert.deepEqual(
    health.map(({ state }) => state),
    ["ready", "ready", "ready", "ready", "ready"],
  );

  services[0].ready = false;
  assert.deepEqual(await inspectServiceHealth(services[0]), {
    healthy: false,
    state: "degraded",
    version: "0.1.0",
  });
} finally {
  await Promise.all(services.map(({ server }) => close(server)));
}

assert.deepEqual(await inspectServiceHealth(services[0]), {
  healthy: false,
  state: "unavailable",
  version: "-",
});

console.log("web shell behavior: 5 ready, degraded, and unavailable paths passed");
console.log("web product contracts: auth, API, WS, redaction, and four-surface shell passed");

async function startService(name, version, ready) {
  const service = { name, version, ready, server: undefined, url: undefined };
  service.server = createServer((request, response) => {
    const isLive = request.url === "/health/live";
    const isReady = request.url === "/health/ready";
    if (!isLive && !isReady) {
      response.writeHead(404).end();
      return;
    }
    const healthy = isLive || service.ready;
    response.writeHead(healthy ? 200 : 503, { "Content-Type": "application/json" });
    response.end(
      JSON.stringify({
        service: name,
        version,
        status: isLive ? "live" : service.ready ? "ready" : "not_ready",
      }),
    );
  });
  await new Promise((resolve, reject) => {
    service.server.once("error", reject);
    service.server.listen(0, "127.0.0.1", resolve);
  });
  const address = service.server.address();
  service.url = `http://127.0.0.1:${address.port}`;
  return service;
}

function close(server) {
  return new Promise((resolve, reject) => {
    server.close((error) => (error ? reject(error) : resolve()));
  });
}

function jsonResponse(status, body) {
  return {
    ok: status >= 200 && status < 300,
    status,
    text: async () => JSON.stringify(body),
  };
}
