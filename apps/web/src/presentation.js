const SENSITIVE_KEYS = new Set([
  "access_token",
  "authorization",
  "chain_of_thought",
  "hidden_prompt",
  "private_context",
  "private_prompt",
  "prompt",
  "raw_prompt",
  "raw_token",
  "reasoning",
  "resume_token",
  "secret",
  "token",
]);

export function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

export function redactEvidence(value) {
  if (Array.isArray(value)) return value.map(redactEvidence);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value)
      .filter(([key]) => !SENSITIVE_KEYS.has(key.toLowerCase()))
      .map(([key, child]) => [key, redactEvidence(child)]),
  );
}

export function eventPresentation(event) {
  const type = String(event?.event_type || "EventRecorded");
  const payload = redactEvidence(event?.payload || {});
  const detail = eventPayload(event);
  const visibleDetail = detail.decision && typeof detail.decision === "object"
    && !Array.isArray(detail.decision)
    ? detail.decision
    : detail;
  const summary = firstText(
    visibleDetail.user_visible_summary,
    visibleDetail.player_visible_text,
    visibleDetail.player_visible_explanation,
    visibleDetail.summary,
    visibleDetail.scene_name,
    visibleDetail.review_summary,
    visibleDetail.resolution,
    visibleDetail.state,
    event?.resource_id,
  );
  return {
    sequence: Number(event?.sequence ?? event?.cursor ?? 0),
    type,
    title: EVENT_TITLES[type] || splitWords(type),
    summary: summary || "正式事件已记录",
    visibility: String(event?.visibility_label || "server_filtered"),
    provenance: String(event?.provenance_kind || "recorded_event"),
    payload,
  };
}

export function decisionPresentation(events, authority) {
  const decisionEvent = [...events]
    .reverse()
    .find((event) => /Agent|Decision|ToolExecution/.test(String(event.event_type)));
  const decisionPayload = eventPayload(decisionEvent);
  const decision = decisionPayload.decision && typeof decisionPayload.decision === "object"
    ? decisionPayload.decision
    : decisionPayload;
  const routeEvent = [...events]
    .reverse()
    .find((event) => firstText(eventPayload(event).model_id));
  const route = eventPayload(routeEvent);
  return {
    summary: firstText(
      decision.user_visible_summary,
      decision.player_visible_text,
      decision.player_visible_explanation,
      decision.summary,
      decision.narration,
      "尚无可向玩家公开的 AI 决策摘要。",
    ),
    basis: asList(
      decision.basis_categories
      || decision.basis
      || decision.linked_records
      || ["规则", "场景事实"],
    ),
    model: firstText(
      decision.model_id,
      route.model_id,
      authority?.snapshot?.ai_provider_snapshot,
      "未报告",
    ),
    certification: firstText(
      decision.certification_status,
      decision.certification_level,
      route.certification_status,
      route.certification_level,
      route.route_authorization_event_id && "服务器路由已授权",
      authority?.snapshot?.model_route_snapshot && "Authority 已锁定模型路由",
      "服务端未报告",
    ),
    eventSequence: Number(decisionEvent?.sequence ?? decisionEvent?.cursor ?? 0),
  };
}

export function safeJson(value) {
  return JSON.stringify(redactEvidence(value), null, 2);
}

function eventPayload(event) {
  const payload = redactEvidence(event?.payload || {});
  const eventType = String(event?.event_type || "");
  const typed = payload?.[eventType];
  return typed && typeof typed === "object" && !Array.isArray(typed) ? typed : payload;
}

function firstText(...values) {
  return values.find((value) => typeof value === "string" && value.trim()) || "";
}

function asList(value) {
  if (Array.isArray(value)) return value.map(String).slice(0, 4);
  if (typeof value === "string") return value.split(/[,/]/).map((item) => item.trim()).filter(Boolean).slice(0, 4);
  return [];
}

function splitWords(value) {
  return value.replace(/([a-z])([A-Z])/g, "$1 $2");
}

const EVENT_TITLES = {
  CampaignCreated: "战役已创建",
  CharacterCreated: "角色草稿已建立",
  CharacterSubmitted: "角色已提交审核",
  CharacterReviewed: "角色审核已完成",
  SessionStarted: "Session 已开始",
  SessionStateChanged: "Session 状态已更新",
  SceneSwitched: "场景已切换",
  PlayerActionSubmitted: "调查行动已提交",
  PlayerActionResolved: "检定结果已记录",
  AgentJobRequested: "Agent 工作已请求",
  AgentDraftApproved: "AI 草案已批准",
  AgentDecisionProduced: "AI 决策已生成",
  DecisionCommitted: "AI 决策已正式记录",
  ToolExecutionSucceeded: "工具执行已形成正式事件",
  CampaignForkRecorded: "Campaign 分支已记录",
  ReconsiderationRequested: "重考虑请求已记录",
  CampaignExportRequested: "战报导出已请求",
};
