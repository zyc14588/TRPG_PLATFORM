import { api, realtime, root, state } from "./context.js";
import {
  acceptInvite, createCampaign, createCharacter, endSession, issueInvite, loadCampaigns,
  login, logout, openCampaign, reviewCharacter, safetyPause, startSession, submitAction,
  submitCharacter, switchScene,
} from "./campaign-operations.js";
import {
  adminCreateUser, adminForkAuthority, adminLogin, approveAgentJob, confirmAction,
  manageGroup, reconsider, refreshEvents, requestAgentJob, requestExport,
} from "./governance-operations.js";
import { messageFor, successFor } from "./support.js";
import { render } from "./views.js";
import { announce, clearFeedback, focusMain, setBusy } from "./view-support.js";

export async function handleAction(action, dataset) {
  if (action === "reload") {
    globalThis.location.reload();
    return;
  }
  if (action === "logout") return run("已退出调查台", logout);
  if (["campaigns", "workspace", "admin", "developer"].includes(action)) {
    if ((action === "workspace" || action === "developer") && !state.campaign) return;
    state.screen = action;
    clearFeedback();
    render();
    focusMain();
    return;
  }
  if (["play", "setup", "evidence"].includes(action)) {
    state.workspaceTab = action;
    clearFeedback();
    render();
    focusMain();
    return;
  }
  if (action === "open-campaign") return run("战役已打开", () => openCampaign(dataset.campaignId));
  if (action === "refresh-campaigns") return run("战役列表已刷新", loadCampaigns);
  if (action === "refresh-events") return run("可见事件已刷新", refreshEvents);
  if (action === "reconnect") {
    realtime.reconnect();
    state.notice = "正在重新连接实时房间";
    render();
    return;
  }
  if (action === "confirm-action") return run("行动结果已确认", confirmAction);
  if (action === "submit-character") return run("角色已提交审核", submitCharacter);
  if (action === "review-character") return run("角色已批准", reviewCharacter);
  if (action === "safety-pause") return run("安全暂停已提交", safetyPause);
  if (action === "focus-reconsider") {
    state.workspaceTab = "play";
    state.notice = "请在调查员工具中填写原事件序号与重考虑原因";
    render();
    root.querySelector('input[name="eventSequence"]')?.focus();
    return;
  }
  if (action === "admin-diagnostics") return run("诊断证据已载入", async () => { state.adminEvidence = await api.adminEvidence("diagnostics"); });
  if (action === "admin-audit") return run("审计证据已载入", async () => { state.adminEvidence = await api.adminEvidence("audit"); });
}

export async function handleForm(form) {
  const data = Object.fromEntries(new FormData(form));
  const handlers = {
    login: () => login(data),
    "create-campaign": () => createCampaign(data),
    "accept-invite": () => acceptInvite(data),
    "issue-invite": () => issueInvite(data),
    "create-character": () => createCharacter(data),
    "review-character": () => reviewCharacter(data),
    "start-session": () => startSession(data),
    "switch-scene": () => switchScene(data),
    "end-session": () => endSession(data),
    "submit-action": () => submitAction(data),
    "agent-job": () => requestAgentJob(data),
    "approve-agent": () => approveAgentJob(data),
    "confirm-player-action": () => confirmAction(data),
    group: () => manageGroup(data),
    reconsider: () => reconsider(data),
    export: () => requestExport(data),
    "admin-login": () => adminLogin(data),
    "admin-create-user": () => adminCreateUser(data),
    "admin-fork-authority": () => adminForkAuthority(data),
  };
  const handler = handlers[form.dataset.form];
  if (handler) await run(successFor(form.dataset.form), handler);
}

async function run(successMessage, operation) {
  setBusy(true);
  clearFeedback();
  try {
    await operation();
    state.notice = successMessage;
    announce(successMessage);
  } catch (error) {
    state.error = messageFor(error);
    announce(state.error);
  } finally {
    setBusy(false);
    render();
  }
}
