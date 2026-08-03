let commandCounter = 0;

export class ProductApiError extends Error {
  constructor(status, code, message = code) {
    super(message);
    this.name = "ProductApiError";
    this.status = status;
    this.code = code;
  }
}

export function createCommand(purpose, expectedVersion = 0) {
  commandCounter += 1;
  const normalized = String(purpose)
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, 46) || "web";
  const nonce = `${Date.now().toString(36)}_${commandCounter.toString(36)}`;
  const root = `web_${normalized}_${nonce}`;
  return {
    command_id: `${root}_command`,
    idempotency_key: `${root}_idempotency`,
    expected_version: Number(expectedVersion),
    correlation_id: `${root}_correlation`,
    causation_id: `${root}_causation`,
    trace_id: `${root}_trace`,
  };
}

export class ProductApi {
  #accessToken = null;
  #adminToken = null;

  constructor(config, fetchImplementation = globalThis.fetch) {
    this.config = config;
    this.fetch = fetchImplementation.bind(globalThis);
  }

  get accessToken() {
    return this.#accessToken;
  }

  clearSession() {
    this.#accessToken = null;
    this.#adminToken = null;
  }

  async login(login, password) {
    const session = await this.#request(this.#url("apiBase", "/auth/login"), {
      method: "POST",
      body: { login, password },
      authorization: "none",
    });
    this.#accessToken = requiredString(session.access_token, "SESSION_TOKEN_MISSING");
    return {
      userId: requiredString(session.user_id, "SESSION_USER_MISSING"),
      globalRole: requiredString(session.global_role, "SESSION_ROLE_MISSING"),
      expiresAtUnixMs: Number(session.expires_at_unix_ms),
    };
  }

  async logout() {
    if (!this.#accessToken) return;
    try {
      await this.#request(this.#url("apiBase", "/auth/logout"), {
        method: "POST",
      });
    } finally {
      this.clearSession();
    }
  }

  listCampaigns() {
    return this.#v1("GET", "/campaigns");
  }

  getCampaign(campaignId) {
    return this.#v1("GET", `/campaigns/${id(campaignId)}`);
  }

  createCampaign(body) {
    return this.#v1("POST", "/campaigns", body);
  }

  forkCampaign(parentCampaignId, body) {
    return this.#v1("POST", `/campaigns/${id(parentCampaignId)}/forks`, body);
  }

  createForkedCampaign(parentCampaignId, body) {
    return this.#v1("POST", `/campaigns/${id(parentCampaignId)}/forked-campaigns`, body);
  }

  getAuthority(campaignId) {
    return this.#product("GET", `/campaigns/${id(campaignId)}/authority`);
  }

  getMembership(campaignId) {
    return this.#product("GET", `/campaigns/${id(campaignId)}/membership`);
  }

  issueInvite(campaignId, body) {
    return this.#v1("POST", `/campaigns/${id(campaignId)}/invites`, body);
  }

  acceptInvite(campaignId, inviteId, body) {
    return this.#v1(
      "POST",
      `/campaigns/${id(campaignId)}/invites/${id(inviteId)}/accept`,
      body,
    );
  }

  createCharacter(campaignId, body) {
    return this.#v1("POST", `/campaigns/${id(campaignId)}/characters`, body);
  }

  submitCharacter(campaignId, characterId, body) {
    return this.#v1(
      "POST",
      `/campaigns/${id(campaignId)}/characters/${id(characterId)}/submit`,
      body,
    );
  }

  reviewCharacter(campaignId, characterId, body) {
    return this.#v1(
      "POST",
      `/campaigns/${id(campaignId)}/characters/${id(characterId)}/review`,
      body,
    );
  }

  startSession(campaignId, body) {
    return this.#v1("POST", `/campaigns/${id(campaignId)}/sessions`, body);
  }

  changeSession(campaignId, sessionId, body) {
    return this.#v1(
      "PATCH",
      `/campaigns/${id(campaignId)}/sessions/${id(sessionId)}`,
      body,
    );
  }

  switchScene(campaignId, sessionId, body) {
    return this.#v1(
      "POST",
      `/campaigns/${id(campaignId)}/sessions/${id(sessionId)}/scenes`,
      body,
    );
  }

  submitAction(campaignId, body) {
    return this.#v1("POST", `/campaigns/${id(campaignId)}/player-actions`, body);
  }

  confirmAction(campaignId, actionId, body) {
    return this.#v1(
      "POST",
      `/campaigns/${id(campaignId)}/player-actions/${id(actionId)}/confirm`,
      body,
    );
  }

  submitGameplayAction(campaignId, body) {
    return this.#v1("POST", `/campaigns/${id(campaignId)}/gameplay-actions`, body);
  }

  requestAgentJob(campaignId, body) {
    return this.#v1("POST", `/campaigns/${id(campaignId)}/agent-jobs`, body);
  }

  approveAgentJob(campaignId, jobId, body) {
    return this.#v1(
      "POST",
      `/campaigns/${id(campaignId)}/agent-jobs/${id(jobId)}/approve`,
      body,
    );
  }

  requestReconsideration(campaignId, body) {
    return this.#v1("POST", `/campaigns/${id(campaignId)}/reconsiderations`, body);
  }

  requestExport(campaignId, body) {
    return this.#v1("POST", `/campaigns/${id(campaignId)}/exports`, body);
  }

  getExport(campaignId, exportId) {
    return this.#v1("GET", `/campaigns/${id(campaignId)}/exports/${id(exportId)}`);
  }

  issueExportDownload(campaignId, exportId) {
    return this.#v1(
      "POST",
      `/campaigns/${id(campaignId)}/exports/${id(exportId)}/download-authorizations`,
    );
  }

  downloadExport(campaignId, exportId, token) {
    if (!/^[a-f0-9]{64}$/.test(String(token))) {
      throw new ProductApiError(400, "CAMPAIGN_EXPORT_DOWNLOAD_TOKEN_INVALID");
    }
    return this.#request(
      this.#url(
        "v1Base",
        `/campaigns/${id(campaignId)}/exports/${id(exportId)}/download`,
      ),
      {
        method: "GET",
        headers: { "X-TRPG-Export-Authorization": token },
      },
    );
  }

  replayEvents(campaignId, afterSequence = 0) {
    const cursor = Math.max(0, Number(afterSequence) || 0);
    return this.#product(
      "GET",
      `/campaigns/${id(campaignId)}/events?after_sequence=${cursor}&limit=200`,
    );
  }

  createGroup(campaignId, groupId) {
    return this.#product("POST", `/campaigns/${id(campaignId)}/groups/${id(groupId)}`);
  }

  assignGroup(campaignId, groupId, userId) {
    return this.#product(
      "PUT",
      `/campaigns/${id(campaignId)}/groups/${id(groupId)}/memberships/${id(userId)}`,
    );
  }

  async adminLogin(login, password) {
    const session = await this.#request(this.#url("adminBase", "/sessions"), {
      method: "POST",
      body: { login, password },
      authorization: "none",
    });
    this.#adminToken = requiredString(session.access_token, "ADMIN_SESSION_TOKEN_MISSING");
    return session;
  }

  adminEvidence(resource) {
    return this.#request(this.#url("adminBase", `/${resource}`), {
      method: "GET",
      authorization: "admin",
      headers: { "X-Correlation-Id": `web-admin-${Date.now().toString(36)}` },
    });
  }

  adminStatus() {
    return this.#request(this.#url("adminBase", "/bootstrap/status"), {
      method: "GET",
      authorization: "admin",
      headers: { "X-Correlation-Id": `web-admin-status-${Date.now().toString(36)}` },
    });
  }

  adminCreateUser(body, expectedVersion) {
    return this.#adminMutation("POST", "/users", body, expectedVersion, "user-create");
  }

  adminForkAuthority(body, expectedVersion) {
    return this.#adminMutation(
      "POST",
      "/authority-forks",
      body,
      expectedVersion,
      "authority-fork",
    );
  }

  #v1(method, path, body) {
    return this.#request(this.#url("v1Base", path), { method, body });
  }

  #product(method, path, body) {
    return this.#request(this.#url("apiBase", path), { method, body });
  }

  #adminMutation(method, path, body, expectedVersion, purpose) {
    const nonce = `${Date.now().toString(36)}-${++commandCounter}`;
    return this.#request(this.#url("adminBase", path), {
      method,
      body,
      authorization: "admin",
      headers: {
        "Idempotency-Key": `web-${purpose}-${nonce}`,
        "X-Expected-Version": String(Number(expectedVersion)),
        "X-Correlation-Id": `web-${purpose}-correlation-${nonce}`,
        "X-Causation-Id": `web-${purpose}-causation-${nonce}`,
      },
    });
  }

  #url(baseName, path) {
    const base = requiredString(this.config[baseName], `CONFIG_${baseName.toUpperCase()}_MISSING`);
    if (!base.startsWith("/") || base.startsWith("//") || base.includes("\\")) {
      throw new ProductApiError(500, `CONFIG_${baseName.toUpperCase()}_NOT_SAME_ORIGIN`);
    }
    return `${base.replace(/\/$/, "")}/${path.replace(/^\//, "")}`;
  }

  async #request(url, options) {
    const headers = new Headers({ Accept: "application/json", ...options.headers });
    const token = options.authorization === "admin" ? this.#adminToken : this.#accessToken;
    if (options.authorization !== "none") {
      if (!token) throw new ProductApiError(401, "SESSION_REQUIRED");
      headers.set("Authorization", `Bearer ${token}`);
    }
    const init = { method: options.method, headers, cache: "no-store" };
    if (options.body !== undefined) {
      headers.set("Content-Type", "application/json");
      init.body = JSON.stringify(options.body);
    }
    let response;
    try {
      response = await this.fetch(url, init);
    } catch {
      throw new ProductApiError(0, "NETWORK_UNAVAILABLE");
    }
    const text = await response.text();
    let payload = {};
    if (text) {
      try {
        payload = JSON.parse(text);
      } catch {
        throw new ProductApiError(response.status, "INVALID_SERVER_RESPONSE");
      }
    }
    if (!response.ok) {
      throw new ProductApiError(
        response.status,
        String(payload.error || payload.code || `HTTP_${response.status}`),
      );
    }
    return payload;
  }
}

function id(value) {
  const normalized = requiredString(value, "IDENTIFIER_REQUIRED");
  if (!/^[A-Za-z0-9_.-]{1,128}$/.test(normalized)) {
    throw new ProductApiError(400, "IDENTIFIER_INVALID");
  }
  return encodeURIComponent(normalized);
}

function requiredString(value, code) {
  if (typeof value !== "string" || !value.trim()) throw new ProductApiError(500, code);
  return value;
}
