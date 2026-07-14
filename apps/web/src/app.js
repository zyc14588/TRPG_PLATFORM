import { inspectServiceHealth } from "/src/health.js";

const serviceList = document.querySelector("#service-list");
const summary = document.querySelector("#summary");
const checkedAt = document.querySelector("#checked-at");
const refreshButton = document.querySelector("#refresh-button");

let config;

function createServiceRow(service) {
  const row = document.createElement("article");
  row.className = "service-row";
  row.dataset.url = service.url;

  const name = document.createElement("strong");
  name.className = "service-name";
  name.textContent = service.name;

  const state = document.createElement("span");
  state.className = "state state-checking";
  state.textContent = "Checking";

  const version = document.createElement("span");
  version.className = "service-version";
  version.textContent = "-";

  const endpoint = document.createElement("code");
  endpoint.textContent = service.url.replace(/^https?:\/\//, "");

  row.append(name, state, version, endpoint);
  return { row, state, version };
}

async function inspectService(service, elements) {
  const health = await inspectServiceHealth(service);
  const presentation = {
    ready: ["state-ready", "Ready"],
    degraded: ["state-degraded", "Degraded"],
    unavailable: ["state-offline", "Unavailable"],
  }[health.state];
  elements.state.className = `state ${presentation[0]}`;
  elements.state.textContent = presentation[1];
  elements.version.textContent = health.version;
  return health.healthy;
}

async function refresh() {
  refreshButton.disabled = true;
  const rows = config.services.map((service) => {
    const elements = createServiceRow(service);
    serviceList.append(elements.row);
    return { service, elements };
  });
  while (serviceList.children.length > config.services.length) {
    serviceList.firstElementChild.remove();
  }

  const results = await Promise.all(
    rows.map(({ service, elements }) => inspectService(service, elements)),
  );
  const readyCount = results.filter(Boolean).length;
  summary.textContent = `${readyCount} of ${results.length} processes ready`;
  checkedAt.dateTime = new Date().toISOString();
  checkedAt.textContent = `Checked ${new Date().toLocaleTimeString()}`;
  refreshButton.disabled = false;
}

async function start() {
  const response = await fetch("/config.json", { cache: "no-store" });
  if (!response.ok) {
    throw new Error(`configuration request failed: ${response.status}`);
  }
  config = await response.json();
  document.querySelector("#environment-label").textContent = config.environment;
  document.querySelector("#web-version").textContent = config.version;
  serviceList.replaceChildren();
  await refresh();
}

refreshButton.addEventListener("click", () => {
  serviceList.replaceChildren();
  void refresh();
});

start().catch(() => {
  summary.textContent = "Web configuration unavailable";
  refreshButton.disabled = true;
});
