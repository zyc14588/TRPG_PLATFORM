import { escapeHtml } from "/src/presentation.js";
import { announcer, root, state } from "./context.js";

export function feedbackView() {
  if (state.error) return `<div class="feedback error" role="alert">${escapeHtml(state.error)}</div>`;
  if (state.notice) return `<div class="feedback notice" role="status">${escapeHtml(state.notice)}</div>`;
  return "";
}

export function navButton(action, label, icon, active, disabled = false) {
  return `<button type="button" data-action="${action}" class="nav-button ${active === action ? "active" : ""}" ${disabled ? "disabled" : ""}><span aria-hidden="true">${icon}</span>${label}</button>`;
}

export function tabButton(tab, label) {
  return `<button type="button" data-action="${tab}" aria-current="${state.workspaceTab === tab ? "page" : "false"}" class="${state.workspaceTab === tab ? "active" : ""}">${label}</button>`;
}

export function setBusy(value) {
  state.busy = value;
  root.setAttribute("aria-busy", String(value));
  root.querySelectorAll("button, input, select, textarea").forEach((element) => {
    if (value) element.setAttribute("data-was-disabled", String(element.disabled));
    element.disabled = value || element.dataset.wasDisabled === "true";
    if (!value) delete element.dataset.wasDisabled;
  });
}

export function clearFeedback() {
  state.error = "";
  state.notice = "";
}

export function announce(message) {
  announcer.textContent = "";
  requestAnimationFrame(() => { announcer.textContent = message; });
}

export function focusMain() {
  requestAnimationFrame(() => root.querySelector("#main-content")?.focus());
}

export function updateConnectionStatus() {
  const summary = root.querySelector("#realtime-summary");
  const footer = root.querySelector("#realtime-footer");
  if (summary) {
    summary.className = `connection ${connectionClass()}`;
    summary.textContent = connectionText();
  }
  if (footer) {
    const indicator = document.createElement("i");
    indicator.className = `status-dot ${connectionClass()}`;
    footer.replaceChildren(indicator, document.createTextNode(connectionText()));
  }
}

export function connectionText() {
  return {
    connected: "实时已连接",
    synced: `已同步 #${state.realtime.cursor || 0}`,
    connecting: "正在连接",
    reconnecting: "正在重连",
    resync: "需要重新同步",
    forbidden: "实时权限已撤销",
    unauthenticated: "实时会话已失效",
    unavailable: "实时服务不可用",
    error: `实时错误 ${state.realtime.code || ""}`,
    disconnected: "Realtime 尚未进入房间",
  }[state.realtime.state] || "Realtime 尚未进入房间";
}

export function connectionClass() {
  if (["connected", "synced"].includes(state.realtime.state)) return "good";
  if (["connecting", "reconnecting", "resync"].includes(state.realtime.state)) return "warn";
  return "bad";
}

export function fatalView() {
  return `<main id="main-content" class="boot-screen" tabindex="-1"><p class="eyeline">雾港调查台</p><h1>无法打开调查档案</h1><p>${escapeHtml(state.error)}</p><button class="button" type="button" data-action="reload">重试</button></main>`;
}
