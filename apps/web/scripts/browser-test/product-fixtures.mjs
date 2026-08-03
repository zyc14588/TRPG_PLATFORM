export function requestBody(request) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    request.on("data", (chunk) => chunks.push(chunk));
    request.on("end", () => {
      if (!chunks.length) return resolve({});
      try { resolve(JSON.parse(Buffer.concat(chunks).toString("utf8"))); } catch (error) { reject(error); }
    });
    request.on("error", reject);
  });
}

export function json(response, status, payload) {
  const body = JSON.stringify(payload);
  response.writeHead(status, { "Content-Type": "application/json", "Content-Length": Buffer.byteLength(body), "Cache-Control": "no-store" });
  response.end(body);
}

export function bearer(value = "") {
  return value.startsWith("Bearer ") ? value.slice(7) : "";
}

export function userByToken(token) {
  return Object.values(USERS).find((user) => user.token === token);
}

export function campaign(campaignId) {
  const value = CAMPAIGNS.find((item) => item.campaign_id === campaignId);
  if (!value) throw new Error("CAMPAIGN_NOT_FOUND");
  return value;
}

export function authority(campaignId) {
  const ai = campaignId === "campaign_ai";
  return {
    contract_id: `authority_${campaignId}`,
    campaign_id: campaignId,
    mode: ai ? "AI_KP" : "HUMAN_KP",
    authority_owner: ai ? "ai_keeper_orchestrator" : "keeper_01",
    version: 1,
    created_at_unix_ms: ai ? 1_700_000_000_001 : 1_700_000_000_000,
    locked: true,
    change_policy: "FORK_ONLY",
    snapshot: {
      ruleset_version: "coc7_rules_1",
      house_rules_version: "house_rules_none_1",
      scenario_version: "tutorial_mist_archive_1",
      prompt_version: "keeper_prompt_1",
      agent_pack_version: "keeper_agent_pack_1",
      tool_schema_version: "tool_schema_1",
      safety_profile_version: "safety_profile_1",
      ai_provider_snapshot: "local-mistral-coc7",
      model_route_snapshot: "route_level4_1",
      character_sheet_template_version: "coc7_investigator_1",
    },
  };
}

export function event(sequence, eventType, visibilityLabel, payload, visibilitySubject = null) {
  return {
    sequence,
    cursor: sequence,
    stream_version: sequence,
    event_type: eventType,
    event_schema_version: 1,
    resource_type: "scene",
    resource_id: "scene_browser",
    visibility_label: visibilityLabel,
    visibility_subject: visibilitySubject,
    provenance_kind: eventType.startsWith("Agent") ? "agent_output" : "user_statement",
    provenance_reference: `reference_${sequence}`,
    provenance_recorded_by: "browser_fixture",
    payload,
  };
}

export function realtimeEvent(item, campaignId) {
  return {
    cursor: item.sequence,
    stream_version: item.stream_version,
    event_type: item.event_type,
    event_schema_version: 1,
    campaign_id: campaignId,
    resource_type: item.resource_type,
    resource_id: item.resource_id,
    authority_mode: authority(campaignId).mode.toLowerCase(),
    authority_epoch: 1,
    visibility_label: item.visibility_label,
    visibility_subject: item.visibility_subject,
    provenance_kind: item.provenance_kind,
    provenance_reference: item.provenance_reference,
    provenance_recorded_by: item.provenance_recorded_by,
    correlation_id: `correlation_${item.sequence}`,
    causation_id: `causation_${item.sequence}`,
    trace_id: `trace_${item.sequence}`,
    payload: item.payload,
  };
}

export function parseClientFrame(buffer) {
  if (buffer.length < 2) return null;
  const opcode = buffer[0] & 0x0f;
  const masked = (buffer[1] & 0x80) !== 0;
  let length = buffer[1] & 0x7f;
  let offset = 2;
  if (length === 126) {
    if (buffer.length < 4) return null;
    length = buffer.readUInt16BE(2);
    offset = 4;
  } else if (length === 127) {
    if (buffer.length < 10) return null;
    const large = buffer.readBigUInt64BE(2);
    if (large > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error("frame too large");
    length = Number(large);
    offset = 10;
  }
  const maskBytes = masked ? 4 : 0;
  if (buffer.length < offset + maskBytes + length) return null;
  const payload = Buffer.from(buffer.subarray(offset + maskBytes, offset + maskBytes + length));
  if (masked) {
    const mask = buffer.subarray(offset, offset + 4);
    for (let index = 0; index < payload.length; index += 1) payload[index] ^= mask[index % 4];
  }
  return { opcode, payload, bytes: offset + maskBytes + length };
}

export function serverFrame(text) {
  const payload = Buffer.from(text);
  if (payload.length < 126) return Buffer.concat([Buffer.from([0x81, payload.length]), payload]);
  const header = Buffer.alloc(4);
  header[0] = 0x81;
  header[1] = 126;
  header.writeUInt16BE(payload.length, 2);
  return Buffer.concat([header, payload]);
}

export const CAMPAIGNS = [
  { campaign_id: "campaign_human", owner_user_id: "keeper_01", authority_contract_id: "authority_campaign_human", title: "灰港档案室", state: "ACTIVE", aggregate_version: 1, last_event_sequence: 1 },
  { campaign_id: "campaign_ai", owner_user_id: "ai_owner", authority_contract_id: "authority_campaign_ai", title: "潮汐下的钟声", state: "ACTIVE", aggregate_version: 1, last_event_sequence: 10 },
];

export const USERS = {
  "keeper@example.test": { token: "token_keeper", userId: "keeper_01", globalRole: "USER", roles: { campaign_human: "HUMAN_KEEPER", campaign_ai: "HUMAN_KEEPER" } },
  "player-b@example.test": { token: "token_player_b", userId: "player_b", globalRole: "USER", roles: { campaign_human: "PLAYER" } },
  "spectator@example.test": { token: "token_spectator", userId: "spectator_01", globalRole: "USER", roles: { campaign_human: "SPECTATOR" } },
  "ai-player@example.test": { token: "token_ai_player", userId: "ai_player", globalRole: "USER", roles: { campaign_ai: "PLAYER" } },
};
