import { validateHealthPayload } from "./health.js";

const CONFIG_URL = "./app.config.json";
const HEALTH_PATHS = Object.freeze({ live: "/health/live", ready: "/health/ready" });
const REQUEST_TIMEOUT_MS = 4_000;

const body = document.querySelector("#health-body");
const refreshButton = document.querySelector("#refresh");
const configStatus = document.querySelector("#config-status");
const webVersion = document.querySelector("#web-version");
const announcer = document.querySelector("#announcer");

let services = [];
let refreshSequence = 0;

function validateConfig(config) {
  if (!config || typeof config.version !== "string" || !Array.isArray(config.services)) {
    throw new Error("invalid_config_shape");
  }
  if (config.services.length !== 5) throw new Error("invalid_service_count");

  const ids = new Set();
  for (const service of config.services) {
    if (!service || typeof service.id !== "string" || typeof service.name !== "string" || typeof service.processName !== "string") {
      throw new Error("invalid_service_identity");
    }
    const url = new URL(service.healthBaseUrl);
    if (url.protocol !== "http:" || url.hostname !== "127.0.0.1" || url.pathname !== "/") {
      throw new Error("invalid_health_base_url");
    }
    if (ids.has(service.id)) throw new Error("duplicate_service_id");
    ids.add(service.id);
  }

  return config;
}

function stateMarkup(state, kind) {
  const labels = {
    live: { loading: "检查中", available: "运行中", unavailable: "不可用" },
    ready: { loading: "检查中", available: "就绪", unavailable: "不可用" },
  };
  return `<span class="state state-${state}"><span class="dot" aria-hidden="true"></span>${labels[kind][state]}</span>`;
}

function renderRows(state = "loading") {
  body.replaceChildren(...services.map((service) => {
    const row = document.createElement("tr");
    row.dataset.service = service.id;
    const heading = document.createElement("th");
    heading.scope = "row";
    heading.textContent = service.name;
    row.append(heading);
    for (const kind of Object.keys(HEALTH_PATHS)) {
      const cell = document.createElement("td");
      cell.dataset.kind = kind;
      cell.innerHTML = stateMarkup(state, kind);
      row.append(cell);
    }
    return row;
  }));
}

function updateCell(serviceId, kind, state, detail = "") {
  const cell = body.querySelector(`[data-service="${serviceId}"] [data-kind="${kind}"]`);
  if (!cell) return;
  cell.innerHTML = stateMarkup(state, kind);
  const endpointLabel = kind === "live" ? "存活" : "就绪";
  cell.setAttribute("aria-label", `${endpointLabel}状态：${cell.textContent.trim()}${detail ? `，${detail}` : ""}`);
  if (detail) cell.title = detail;
  else cell.removeAttribute("title");
}

async function probe(url, kind, processName) {
  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
  try {
    const response = await fetch(url, {
      cache: "no-store",
      headers: { Accept: "application/json" },
      signal: controller.signal,
    });
    if (!response.ok) return { state: "unavailable", detail: `HTTP ${response.status}` };
    const payload = await response.json();
    return validateHealthPayload(payload, kind, processName)
      ? { state: "available", detail: `HTTP ${response.status}` }
      : { state: "unavailable", detail: "响应契约无效" };
  } catch (error) {
    return {
      state: "unavailable",
      detail: error?.name === "AbortError" ? "请求超时" : "无法连接",
    };
  } finally {
    window.clearTimeout(timeout);
  }
}

async function refreshHealth() {
  const sequence = ++refreshSequence;
  refreshButton.disabled = true;
  refreshButton.setAttribute("aria-busy", "true");
  renderRows("loading");

  const results = await Promise.all(services.flatMap((service) => (
    Object.entries(HEALTH_PATHS).map(async ([kind, path]) => ({
      service,
      kind,
      result: await probe(new URL(path, service.healthBaseUrl).href, kind, service.processName),
    }))
  )));

  if (sequence !== refreshSequence) return;
  for (const { service, kind, result } of results) {
    updateCell(service.id, kind, result.state, result.detail);
  }

  const available = results.filter(({ result }) => result.state === "available").length;
  announcer.textContent = `健康状态刷新完成，${results.length} 个端点中 ${available} 个可用。`;
  refreshButton.disabled = false;
  refreshButton.removeAttribute("aria-busy");
}

async function loadConfig() {
  try {
    const response = await fetch(CONFIG_URL, { cache: "no-store" });
    if (!response.ok) throw new Error(`config_http_${response.status}`);
    const config = validateConfig(await response.json());
    services = config.services;
    configStatus.textContent = "已加载";
    configStatus.className = "config-loaded";
    webVersion.textContent = `v${config.version}`;
    webVersion.className = "version-loaded";
    renderRows();
    await refreshHealth();
  } catch {
    services = [];
    configStatus.textContent = "不可用";
    configStatus.className = "config-error";
    webVersion.textContent = "—";
    body.innerHTML = `
      <tr>
        <th scope="row">服务配置</th>
        <td>${stateMarkup("unavailable", "live")}</td>
        <td>${stateMarkup("unavailable", "ready")}</td>
      </tr>
    `;
    announcer.textContent = "服务配置加载失败，健康状态不可用。";
    refreshButton.disabled = true;
  }
}

refreshButton.addEventListener("click", refreshHealth);
loadConfig();
