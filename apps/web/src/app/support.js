import { ProductApiError } from "/src/api.js";
import { state } from "./context.js";

export function setVersion(key, value) {
  const normalized = Number(value);
  if (Number.isFinite(normalized) && normalized > 0) state.versions.set(key, normalized);
}

export function version(key, fallback) {
  return state.versions.get(key) || fallback;
}

export function requiredRecent(key, message) {
  const value = state.recent[key];
  if (!value) throw new Error(message);
  return value;
}

export function lastEventSequence() {
  return state.events.reduce(
    (highest, event) => Math.max(highest, Number(event.sequence ?? event.cursor ?? 0)),
    0,
  ) || 1;
}

export function visibilityName(value) {
  return {
    public: "公开",
    party_visible: "队伍可见",
    keeper_only: "仅 KP",
    private_to_player: "私密玩家",
    private_to_group: "私密分队",
    server_admin_only: "仅管理员",
    server_filtered: "服务器已过滤",
  }[String(value).toLowerCase()] || value;
}

export function successFor(formName) {
  return {
    login: "登录成功",
    "create-campaign": "战役已创建",
    "accept-invite": "已加入战役",
    "issue-invite": "邀请已签发",
    "create-character": "角色草稿已保存",
    "review-character": "角色已批准",
    "start-session": "Session 已开始",
    "switch-scene": "场景推进已提交",
    "end-session": "Tutorial 结局已记录，Session 已结束",
    "submit-action": "行动已提交",
    "public-gameplay": "公开玩法结果已记录",
    "agent-job": "Agent 工作已请求",
    "approve-agent": "AI 草案批准已提交",
    "confirm-player-action": "行动结果已确认",
    group: "分队成员已更新",
    reconsider: "重考虑请求已提交",
    export: "战报导出已就绪",
    "admin-login": "Admin session 已建立",
    "admin-create-user": "普通用户已创建",
    "admin-fork-authority": "Authority 子分支已派生",
  }[formName] || "操作已完成";
}

export function messageFor(error) {
  const code = error instanceof ProductApiError ? error.code : error?.message || "UNKNOWN_ERROR";
  const known = {
    INVALID_CREDENTIALS: "凭据无效，请重新检查。",
    LOGIN_RATE_LIMITED: "登录尝试过多，请稍后再试。",
    SESSION_REQUIRED: "会话已失效，请重新登录。",
    SESSION_EXPIRED: "会话已过期，请重新登录。",
    NETWORK_UNAVAILABLE: "无法连接服务，请检查部署状态后重试。",
    CAMPAIGN_MEMBERSHIP_DENIED: "服务器拒绝此操作：当前席位权限不足。",
    MEMBERSHIP_REQUIRED: "当前身份不是该战役成员。",
    CORE_API_FORBIDDEN: "服务器拒绝此操作：当前席位权限不足。",
    REALTIME_SUBSCRIPTION_DENIED: "服务器拒绝实时房间订阅。",
    AGENT_JOB_AUTHORITY_FORBIDDEN: "当前 Authority 模式不允许此 Agent 操作。",
    AGENT_JOB_GATEWAY_UNAVAILABLE: "Agent Gateway 当前不可用。",
  };
  return known[code] || `操作未完成：${code}`;
}

export function defaultCharacterSheet() {
  return JSON.stringify({
    name: "林若岚",
    age: 29,
    occupation: "调查记者",
    era: "1920s",
    birthplace: "Brisbane",
    characteristics: {
      strength: 45,
      dexterity: 55,
      power: 65,
      constitution: 50,
      size: 50,
      appearance: 60,
      intelligence: 70,
      education: 75,
      luck: 60,
    },
    skills: {
      "Library Use": 75,
      "Spot Hidden": 60,
      "Fighting (Brawl)": 45,
      "Firearms (Handgun)": 35,
      Dodge: 40,
      "First Aid": 30,
      Medicine: 10,
    },
    combat_profile: {
      dexterity: 55,
      skill_targets: { melee: 45, firearm: 35, dodge: 40, first_aid: 30, medicine: 10 },
      skill_target_sources: {
        melee: "Fighting (Brawl)",
        firearm: "Firearms (Handgun)",
        dodge: "Dodge",
        first_aid: "First Aid",
        medicine: "Medicine",
      },
      weapon_loadout: {
        melee: {
          weapon_id: "selected_melee_weapon",
          damage_formula: { dice_count: 1, die_sides: 6, flat_bonus: 1 },
        },
        firearm: {
          weapon_id: "selected_firearm",
          damage_formula: { dice_count: 1, die_sides: 6, flat_bonus: 5 },
        },
      },
      current_hp: 10,
      max_hp: 10,
      armor: 1,
      condition: "ABLE",
    },
    chase_profile: { role: "QUARRY", movement_rate: 8 },
    backstory_anchors: ["保护消息来源", "不会抛下同伴"],
  }, null, 2);
}
