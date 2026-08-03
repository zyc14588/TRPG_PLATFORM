import { ProductApiError, createCommand } from "/src/api.js";
import { api, state } from "./context.js";
import { requiredRecent, setVersion, version } from "./support.js";

export async function confirmAction(data = {}) {
  const pending = data.actionId ? { actionId: data.actionId } : state.recent.pendingAction;
  if (!pending) throw new Error("没有等待确认的行动");
  const expectedVersion = Number(data.expectedVersion || version(`action:${pending.actionId}`, 1));
  await api.confirmAction(state.campaign.campaign_id, pending.actionId, {
    command: createCommand("player_action_confirm", expectedVersion),
    campaign_id: state.campaign.campaign_id,
    action_id: pending.actionId,
    resolved_at_unix_ms: Date.now(),
  });
  if (state.recent.pendingAction?.actionId === pending.actionId) state.recent.pendingAction = null;
  await refreshEvents();
}

export async function requestAgentJob(data) {
  const response = await api.requestAgentJob(state.campaign.campaign_id, {
    command: createCommand("agent_job", 0),
    campaign_id: state.campaign.campaign_id,
    job_id: data.jobId,
    rag_snapshot_id: data.ragSnapshotId,
    input: data.input || {
      kind: "npc_skill_check",
      private_note: data.privateNote || undefined,
    },
    deadline_unix_ms: Date.now() + 240_000,
  });
  state.recent.agentJobId = data.jobId;
  state.recent.agentJobVersion = 1;
  state.recent.agentJobState = response.state;
  await refreshEvents();
}

export async function approveAgentJob(data) {
  await api.approveAgentJob(state.campaign.campaign_id, data.jobId, {
    command: createCommand("agent_job_approve", Number(data.expectedVersion)),
    campaign_id: state.campaign.campaign_id,
    job_id: data.jobId,
  });
  await refreshEvents();
}

export async function manageGroup(data) {
  await api.createGroup(state.campaign.campaign_id, data.groupId);
  await api.assignGroup(state.campaign.campaign_id, data.groupId, data.userId);
}

export async function reconsider(data) {
  const reconsiderationId = `reconsider_${Date.now().toString(36)}`;
  await api.requestReconsideration(state.campaign.campaign_id, {
    command: createCommand("reconsider", 0),
    reconsideration_id: reconsiderationId,
    campaign_id: state.campaign.campaign_id,
    original_event_sequence: Number(data.eventSequence),
    requested_by: state.session.userId,
    reason: data.reason,
  });
  await refreshEvents();
}

export async function requestExport(data) {
  await api.requestExport(state.campaign.campaign_id, {
    command: createCommand("campaign_export", 0),
    export_id: data.exportId,
    campaign_id: state.campaign.campaign_id,
    requested_by: state.session.userId,
    audience: data.audience,
    requested_at_unix_ms: Date.now(),
  });
  let status;
  for (let attempt = 0; attempt < 120; attempt += 1) {
    status = await api.getExport(state.campaign.campaign_id, data.exportId);
    if (status.state === "READY") break;
    if (status.state === "FAILED" && Number(status.attempt_count) >= Number(status.max_attempts)) {
      throw new ProductApiError(409, `CAMPAIGN_EXPORT_FAILED_${status.failure_code || "UNKNOWN"}`);
    }
    if (["EXPIRED", "DELETED"].includes(status.state)) {
      throw new ProductApiError(409, `CAMPAIGN_EXPORT_${status.state}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  if (status?.state !== "READY") {
    throw new ProductApiError(503, "CAMPAIGN_EXPORT_TIMEOUT");
  }
  const authorization = await api.issueExportDownload(
    state.campaign.campaign_id,
    data.exportId,
  );
  const artifact = await api.downloadExport(
    state.campaign.campaign_id,
    data.exportId,
    authorization.token,
  );
  state.export = { artifact, status };
  await refreshEvents();
}

export async function adminLogin(data) {
  await api.adminLogin(data.login, data.password);
  state.adminReady = true;
  const status = await api.adminStatus();
  state.adminVersion = Number(status.state_version);
  state.adminEvidence = await api.adminEvidence("diagnostics");
}

export async function adminCreateUser(data) {
  const response = await api.adminCreateUser({
    user_id: data.userId,
    login: data.login,
    password: data.password,
  }, requiredAdminVersion());
  state.adminVersion = Number(response.state_version);
  state.adminEvidence = response;
}

export async function adminForkAuthority(data) {
  const response = await api.adminForkAuthority({
    parent_campaign_id: data.parentCampaignId,
    child_campaign_id: data.childCampaignId,
    authority_mode: data.authorityMode,
    authority_owner: data.authorityOwner,
    campaign_manager_user_id: data.campaignManagerUserId,
  }, requiredAdminVersion());
  state.adminVersion = Number(response.state_version);
  state.adminEvidence = response;
}

export function requiredAdminVersion() {
  if (!Number.isInteger(state.adminVersion) || state.adminVersion < 0) {
    throw new Error("请先建立 Admin session");
  }
  return state.adminVersion;
}

export async function refreshEvents() {
  if (!state.campaign) return;
  const replay = await api.replayEvents(state.campaign.campaign_id, 0);
  state.events = Array.isArray(replay.events) ? replay.events : [];
  state.realtime.cursor = Math.max(
    Number(state.realtime.cursor) || 0,
    Number(replay.scanned_through_sequence) || 0,
  );
}

export function appendEvent(event) {
  const key = `${event.cursor || event.sequence}:${event.event_type}`;
  const seen = new Set(state.events.map((item) => `${item.cursor || item.sequence}:${item.event_type}`));
  if (!seen.has(key)) state.events = [...state.events, event].slice(-200);
  state.realtime.cursor = Math.max(Number(state.realtime.cursor) || 0, Number(event.cursor) || 0);
}
