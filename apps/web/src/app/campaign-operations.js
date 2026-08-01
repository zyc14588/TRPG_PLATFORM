import { createCommand } from "/src/api.js";
import { api, realtime, state } from "./context.js";
import { refreshEvents, requestAgentJob } from "./governance-operations.js";
import { requiredRecent, setVersion, version } from "./support.js";

export async function login(data) {
  state.session = await api.login(data.login, data.password);
  state.screen = "campaigns";
  await loadCampaigns();
}

export async function logout() {
  realtime.disconnect();
  await api.logout();
  Object.assign(state, {
    session: null,
    screen: "login",
    campaigns: [],
    campaign: null,
    authority: null,
    membership: null,
    events: [],
    invite: null,
    export: null,
    adminEvidence: null,
    adminReady: false,
    adminVersion: null,
  });
}

export async function loadCampaigns() {
  const payload = await api.listCampaigns();
  state.campaigns = Array.isArray(payload.campaigns) ? payload.campaigns : [];
}

export async function openCampaign(campaignId) {
  const [campaign, authority, membership, replay] = await Promise.all([
    api.getCampaign(campaignId),
    api.getAuthority(campaignId),
    api.getMembership(campaignId),
    api.replayEvents(campaignId),
  ]);
  state.campaign = campaign;
  state.authority = authority;
  state.membership = membership;
  state.events = Array.isArray(replay.events) ? replay.events : [];
  state.realtime = { state: "connecting", cursor: Number(replay.scanned_through_sequence) || 0 };
  state.screen = "workspace";
  state.workspaceTab = "play";
  realtime.connect({ token: api.accessToken, campaignId, roomId: campaignId });
}

export async function createCampaign(data) {
  const forkFields = [data.parentCampaignId, data.sourceSessionId, data.forkReason];
  if (forkFields.some(Boolean) && !forkFields.every(Boolean)) {
    throw new Error("分支创建需要父战役、源 Session 与 Fork 原因");
  }
  const authority = await api.getAuthority(data.campaignId);
  const snapshot = authority.snapshot || {};
  await api.createCampaign({
    command: createCommand("campaign_create", 0),
    campaign_id: data.campaignId,
    owner_user_id: state.session.userId,
    title: data.title,
    room_id: data.roomId,
    room_name: data.roomName,
    created_at_unix_ms: Number(authority.created_at_unix_ms),
    authority: {
      contract_id: authority.contract_id,
      authority_mode: authority.mode,
      authority_owner: authority.authority_owner,
      ...snapshot,
    },
  });
  if (data.parentCampaignId) {
    await api.forkCampaign(data.parentCampaignId, {
      command: createCommand("campaign_fork", 0),
      fork_id: `fork_${data.campaignId}`,
      parent_campaign_id: data.parentCampaignId,
      child_campaign_id: data.campaignId,
      source_session_id: data.sourceSessionId,
      reason: data.forkReason,
    });
  }
  await loadCampaigns();
}

export async function acceptInvite(data) {
  await api.acceptInvite(data.campaignId, data.inviteId, {
    command: createCommand("invite_accept", 1),
    campaign_id: data.campaignId,
    invite_id: data.inviteId,
    accepting_user_id: state.session.userId,
    raw_token: data.rawToken,
  });
  await loadCampaigns();
}

export async function issueInvite(data) {
  const inviteId = `invite_${Date.now().toString(36)}`;
  state.invite = await api.issueInvite(state.campaign.campaign_id, {
    command: createCommand("invite_issue", 0),
    campaign_id: state.campaign.campaign_id,
    invite_id: inviteId,
    invited_user_id: data.userId,
    role: data.role,
    expires_at_unix_ms: Date.now() + 86_400_000,
  });
  await refreshEvents();
}

export async function createCharacter(data) {
  JSON.parse(data.sheetJson);
  const response = await api.createCharacter(state.campaign.campaign_id, {
    command: createCommand("character_create", 0),
    campaign_id: state.campaign.campaign_id,
    character_id: data.characterId,
    owner_user_id: state.session.userId,
    display_name: data.displayName,
    sheet_version_id: `sheet_${data.characterId}_1`,
    sheet_json: data.sheetJson,
  });
  state.recent.characterId = data.characterId;
  state.recent.characterName = data.displayName;
  state.recent.characterState = "草稿";
  setVersion(`character:${data.characterId}`, response.aggregate_version || 1);
  await refreshEvents();
}

export async function submitCharacter() {
  const characterId = requiredRecent("characterId", "请先保存角色");
  const response = await api.submitCharacter(state.campaign.campaign_id, characterId, {
    command: createCommand("character_submit", version(`character:${characterId}`, 1)),
    campaign_id: state.campaign.campaign_id,
    character_id: characterId,
  });
  setVersion(`character:${characterId}`, response.aggregate_version);
  state.recent.characterState = "待审核";
  await refreshEvents();
}

export async function reviewCharacter(data = {}) {
  const characterId = data.characterId || requiredRecent("characterId", "请先保存并提交角色");
  const expectedVersion = Number(data.expectedVersion || version(`character:${characterId}`, 2));
  const response = await api.reviewCharacter(state.campaign.campaign_id, characterId, {
    command: createCommand("character_review", expectedVersion),
    campaign_id: state.campaign.campaign_id,
    character_id: characterId,
  });
  setVersion(`character:${characterId}`, response.aggregate_version);
  state.recent.characterState = "已批准";
  await refreshEvents();
}

export async function startSession(data) {
  const response = await api.startSession(state.campaign.campaign_id, {
    command: createCommand("session_start", 0),
    campaign_id: state.campaign.campaign_id,
    session_id: data.sessionId,
    room_id: data.roomId,
    scenario_id: data.scenarioId,
    scene_id: data.sceneId,
    scene_key: data.sceneKey,
    scene_name: data.sceneName,
    started_at_unix_ms: Date.now(),
  });
  Object.assign(state.recent, {
    sessionId: data.sessionId,
    roomId: data.roomId,
    sceneId: data.sceneId,
    sceneName: data.sceneName,
  });
  setVersion(`session:${data.sessionId}`, response.aggregate_version || 1);
  state.workspaceTab = "play";
  await refreshEvents();
}

export async function switchScene(data) {
  const response = await api.switchScene(state.campaign.campaign_id, data.sessionId, {
    command: createCommand("scene_switch", version(`session:${data.sessionId}`, 1)),
    campaign_id: state.campaign.campaign_id,
    session_id: data.sessionId,
    next_scene_id: data.sceneId,
    next_scene_key: data.sceneId,
    next_scene_name: data.sceneName,
    switched_at_unix_ms: Date.now(),
  });
  Object.assign(state.recent, { sessionId: data.sessionId, sceneId: data.sceneId, sceneName: data.sceneName });
  setVersion(`session:${data.sessionId}`, response.aggregate_version);
  await refreshEvents();
}

export async function safetyPause() {
  const sessionId = requiredRecent("sessionId", "请先开始 Session");
  const response = await api.changeSession(state.campaign.campaign_id, sessionId, {
    command: createCommand("safety_pause", version(`session:${sessionId}`, 1)),
    campaign_id: state.campaign.campaign_id,
    session_id: sessionId,
    state: "PAUSED",
    changed_at_unix_ms: Date.now(),
  });
  setVersion(`session:${sessionId}`, response.aggregate_version);
  await refreshEvents();
}

export async function endSession(data) {
  const sessionId = data.sessionId;
  const response = await api.changeSession(state.campaign.campaign_id, sessionId, {
    command: createCommand("tutorial_end", version(`session:${sessionId}`, 1)),
    campaign_id: state.campaign.campaign_id,
    session_id: sessionId,
    state: "ENDED",
    changed_at_unix_ms: Date.now(),
  });
  setVersion(`session:${sessionId}`, response.aggregate_version);
  state.recent.sessionState = "ENDED";
  await refreshEvents();
}

export async function submitAction(data) {
  const actionId = `action_${Date.now().toString(36)}`;
  const intent = data.intentKind === "SANITY_CHECK"
    ? {
        kind: "SANITY_CHECK",
        success_loss: Number(data.successLoss),
        failure_loss: Number(data.failureLoss),
        day_key: data.dayKey,
      }
    : {
        kind: "INVESTIGATION",
        skill_name: data.skillName,
        clue_id: data.clueId,
        clue_importance: data.clueImportance,
        adjustment: data.adjustment,
      };
  if (state.authority?.mode === "AI_KP") {
    await requestAgentJob({
      jobId: `job_${actionId}`,
      ragSnapshotId: "tutorial_rag_ai",
      input: {
        kind: "player_action",
        action_id: actionId,
        character_id: data.characterId,
        session_id: data.sessionId,
        scene_id: data.sceneId,
        intent,
        description: data.description || "",
      },
    });
    Object.assign(state.recent, {
      characterId: data.characterId,
      sessionId: data.sessionId,
      sceneId: data.sceneId,
      pendingAction: null,
    });
    return;
  }
  const response = await api.submitAction(state.campaign.campaign_id, {
    command: createCommand("player_action", 0),
    campaign_id: state.campaign.campaign_id,
    action_id: actionId,
    character_id: data.characterId,
    scene_id: data.sceneId,
    submitted_at_unix_ms: Date.now(),
    intent,
  });
  Object.assign(state.recent, { characterId: data.characterId, sessionId: data.sessionId, sceneId: data.sceneId, pendingAction: { actionId } });
  setVersion(`action:${actionId}`, response.aggregate_version || 1);
  await refreshEvents();
}
