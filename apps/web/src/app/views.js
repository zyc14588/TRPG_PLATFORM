import { decisionPresentation, escapeHtml, eventPresentation, safeJson } from "/src/presentation.js";
import { root, state } from "./context.js";
import { defaultCharacterSheet, lastEventSequence, version, visibilityName } from "./support.js";
import {
  connectionClass,
  connectionText,
  feedbackView,
  navButton,
  tabButton,
  updateConnectionStatus,
} from "./view-support.js";

export function render() {
  root.setAttribute("aria-busy", String(state.busy));
  if (!state.session) {
    root.innerHTML = loginView();
    return;
  }
  const views = {
    campaigns: campaignHubView,
    workspace: workspaceView,
    admin: adminView,
    developer: developerView,
  };
  const view = views[state.screen] || campaignHubView;
  root.innerHTML = shellView(view());
  updateConnectionStatus();
}

function loginView() {
  return `
    <main id="main-content" class="login-layout" tabindex="-1">
      <section class="login-brand" aria-labelledby="brand-title">
        <div class="brand-seal" aria-hidden="true"><span>✦</span></div>
        <p class="brand-name" id="brand-title">雾港调查台</p>
        <h1>从这里进入你的调查档案</h1>
        <p>正式行动只会经由公开 API、工作流与事件日志提交。此浏览器不会把会话 token 写入本地存储。</p>
        <dl class="trust-list">
          <div><dt>权威</dt><dd>Campaign 创建后锁定，只能 fork</dd></div>
          <div><dt>骰子</dt><dd>由服务端生成并记录</dd></div>
          <div><dt>私密</dt><dd>由服务器按 Visibility 过滤</dd></div>
        </dl>
      </section>
      <section class="login-panel" aria-labelledby="login-title">
        <p class="eyeline">COC 7 调查工作区</p>
        <h2 id="login-title">进入调查台</h2>
        <p class="muted">使用部署初始化时创建的账号。</p>
        ${feedbackView()}
        <form data-form="login" class="form-stack" data-testid="login-form">
          <label>登录名<input name="login" autocomplete="username" required autofocus /></label>
          <label>密码<input name="password" type="password" autocomplete="current-password" required /></label>
          <button class="button primary full" type="submit">登录</button>
        </form>
        <p class="form-note">凭据只发送到同源认证端点；页面刷新后需要重新登录。</p>
      </section>
    </main>`;
}

function shellView(content) {
  const active = state.screen;
  return `
    <div class="product-shell">
      <aside class="sidebar" aria-label="主要导航">
        <div class="compact-brand"><span class="brand-glyph" aria-hidden="true">✦</span><b>雾港<br />调查台</b></div>
        <nav>
          ${navButton("campaigns", "战役", "⌂", active)}
          ${navButton("workspace", "场景", "◇", active, !state.campaign)}
          ${navButton("developer", "时间线", "◷", active, !state.campaign)}
        </nav>
        <nav class="utility-nav" aria-label="管理导航">
          ${navButton("admin", "管理", "⚙", active)}
          ${navButton("developer", "开发者", "⌘", active, !state.campaign)}
        </nav>
        <div class="user-block">
          <span class="user-avatar" aria-hidden="true">${escapeHtml(state.session.userId.slice(0, 1).toUpperCase())}</span>
          <span><b>${escapeHtml(state.session.userId)}</b><small>${escapeHtml(state.session.globalRole)}</small></span>
        </div>
      </aside>
      <div class="product-body">
        <header class="topbar">
          <div>
            <span class="topbar-label">${state.campaign ? "战役" : "档案库"}</span>
            <strong>${escapeHtml(state.campaign?.title || "战役档案")}</strong>
          </div>
          <div class="topbar-meta">
            ${state.authority ? `<span>权威 <b>${escapeHtml(state.authority.mode)}</b></span>` : ""}
            ${state.campaign ? `<span id="realtime-summary" class="connection ${connectionClass()}">${escapeHtml(connectionText())}</span>` : ""}
            <button class="button quiet" type="button" data-action="logout">退出</button>
          </div>
        </header>
        ${content}
        <footer class="statusbar">
          <span><i class="status-dot good"></i>公开 API 已连接</span>
          <span id="realtime-footer"><i class="status-dot ${connectionClass()}"></i>${escapeHtml(connectionText())}</span>
          <span class="status-spacer"></span>
          <span>Web ${escapeHtml(state.config.version)}</span>
        </footer>
      </div>
      <nav class="mobile-nav" aria-label="移动端导航">
        ${navButton("campaigns", "战役", "⌂", active)}
        ${navButton("workspace", "场景", "◇", active, !state.campaign)}
        ${navButton("developer", "时间线", "◷", active, !state.campaign)}
        ${navButton("admin", "管理", "⚙", active)}
      </nav>
    </div>`;
}

function campaignHubView() {
  const rows = state.campaigns.length
    ? state.campaigns.map(campaignRow).join("")
    : `<div class="empty-state"><strong>还没有可见战役</strong><p>若管理员已预置 Authority Contract，可在下方创建；也可以使用邀请加入。</p></div>`;
  return `
    <main id="main-content" class="page campaign-page" tabindex="-1">
      <section class="page-heading">
        <div><p class="eyeline">调查档案</p><h1>选择一场调查</h1><p>列表只包含当前身份有权查看的 Campaign。</p></div>
        <button class="button" type="button" data-action="refresh-campaigns">刷新</button>
      </section>
      ${feedbackView()}
      <section class="campaign-list" aria-label="战役列表" data-testid="campaign-list">
        <div class="table-heading"><span>战役</span><span>状态</span><span>最近事件</span><span>操作</span></div>
        ${rows}
      </section>
      <div class="lifecycle-grid">
        <details class="open-panel" open>
          <summary>创建预授权战役 <span>Authority 创建后不可更改，只能 fork</span></summary>
          <form data-form="create-campaign" class="form-grid four">
            <label>战役 ID<input name="campaignId" required placeholder="campaign_mist_archive" /></label>
            <label>标题<input name="title" required placeholder="灰港档案室" /></label>
            <label>房间 ID<input name="roomId" required placeholder="room_archive" /></label>
            <label>房间名<input name="roomName" required placeholder="Tutorial Room" /></label>
            <label>父战役 ID（可选）<input name="parentCampaignId" placeholder="campaign_mist_archive" /></label>
            <label>源 Session ID（可选）<input name="sourceSessionId" placeholder="session_tutorial" /></label>
            <label class="span-two">Fork 原因（分支时必填）<input name="forkReason" maxlength="512" placeholder="切换为 AI_KP 教学分支" /></label>
            <p class="span-three form-note">提交时会从服务器读取已锁定 Authority 快照；客户端不能创建或修改权威合同。</p>
            <button class="button primary" type="submit">创建 / 物化分支</button>
          </form>
        </details>
        <details class="open-panel">
          <summary>使用邀请加入 <span>token 仅在内存中处理</span></summary>
          <form data-form="accept-invite" class="form-grid three">
            <label>战役 ID<input name="campaignId" required /></label>
            <label>邀请 ID<input name="inviteId" required /></label>
            <label>邀请 token<input name="rawToken" type="password" autocomplete="off" required /></label>
            <button class="button primary align-end" type="submit">加入战役</button>
          </form>
        </details>
      </div>
    </main>`;
}

function campaignRow(campaign) {
  return `<article class="campaign-row">
    <div><strong>${escapeHtml(campaign.title)}</strong><code>${escapeHtml(campaign.campaign_id)}</code></div>
    <span class="state-label"><i class="status-dot good"></i>${escapeHtml(campaign.state)}</span>
    <span>#${escapeHtml(campaign.last_event_sequence)}</span>
    <button class="button" type="button" data-action="open-campaign" data-campaign-id="${escapeHtml(campaign.campaign_id)}">进入战役</button>
  </article>`;
}

function workspaceView() {
  const isSpectator = state.membership?.role === "SPECTATOR";
  const canKeep = ["HUMAN_KEEPER", "CAMPAIGN_OWNER"].includes(state.membership?.role);
  const decision = decisionPresentation(state.events, state.authority);
  return `
    <main id="main-content" class="workspace" tabindex="-1">
      <nav class="workspace-tabs" aria-label="战役工作区">
        ${tabButton("play", "场景与行动")}
        ${tabButton("setup", "成员与角色")}
        ${tabButton("evidence", "事件证据")}
      </nav>
      ${feedbackView()}
      ${state.workspaceTab === "setup" ? setupView(canKeep) : ""}
      ${state.workspaceTab === "evidence" ? evidenceView(canKeep) : ""}
      ${state.workspaceTab === "play" ? playView({ canKeep, isSpectator, decision }) : ""}
    </main>`;
}

function playView({ canKeep, isSpectator, decision }) {
  return `<div class="play-layout">
    <section class="scene-column">
      <header class="scene-heading">
        <p class="eyeline">当前场景</p>
        <h1>${escapeHtml(state.recent.sceneName || "等待 KP 开始场景")}</h1>
        <p>${isSpectator ? "旁观席只接收服务器许可的公开与队伍事件。" : "描述调查方式后提交，规则与骰子均由服务器处理。"}</p>
      </header>
      ${isSpectator ? spectatorView() : actionView()}
      ${state.authority?.mode === "AI_KP" ? decisionView(decision) : ""}
      ${gameplayProjectionView()}
      <section class="character-strip" aria-labelledby="character-strip-title">
        <div><p class="eyeline">当前角色</p><h2 id="character-strip-title">${escapeHtml(state.recent.characterName || "尚未选择角色")}</h2></div>
        <dl><div><dt>角色 ID</dt><dd>${escapeHtml(state.recent.characterId || "—")}</dd></div><div><dt>席位</dt><dd>${escapeHtml(state.membership?.role || "—")}</dd></div><div><dt>状态</dt><dd>${escapeHtml(state.recent.characterState || "未载入")}</dd></div></dl>
      </section>
    </section>
    ${canKeep ? keeperRailView() : playerRailView()}
    ${timelineView()}
  </div>`;
}

function actionView() {
  const pending = state.recent.pendingAction;
  const aiGameplayOptions = state.authority?.mode === "AI_KP"
    ? '<option value="NPC_INTERACTION">NPC 互动</option><option value="COMBAT_ROUND">基础战斗轮</option><option value="CHASE_SEGMENT">基础追逐段</option>'
    : "";
  return `<section class="action-surface" aria-labelledby="action-title">
    <div class="section-tabs"><h2 id="action-title">Tutorial 检定</h2><span>服务端正式骰</span></div>
    <form data-form="submit-action" class="form-grid three">
      <label>角色 ID<input name="characterId" value="${escapeHtml(state.recent.characterId || "")}" required /></label>
      <label>Session ID<input name="sessionId" value="${escapeHtml(state.recent.sessionId || "")}" required /></label>
      <label>Scene ID<input name="sceneId" value="${escapeHtml(state.recent.sceneId || "")}" required /></label>
      <label>行动类型<select name="intentKind"><option value="INVESTIGATION">调查 / 线索</option><option value="SANITY_CHECK">理智检定</option>${aiGameplayOptions}</select></label>
      <label>技能名<input name="skillName" value="Library Use" required /></label>
      <label>线索 ID<input name="clueId" value="tutorial_archive_clue" required /></label>
      <label>重要性<select name="clueImportance"><option value="CORE">核心线索</option><option value="OPTIONAL">可选线索</option></select></label>
      <label>调整<select name="adjustment"><option value="NONE">无</option><option value="BONUS">奖励骰</option><option value="PENALTY">惩罚骰</option></select></label>
      <label>成功 SAN 损失<input name="successLoss" type="number" min="0" max="99" value="0" required /></label>
      <label>失败 SAN 损失<input name="failureLoss" type="number" min="0" max="99" value="1" required /></label>
      <label>游戏日键<input name="dayKey" value="tutorial_day_1" maxlength="128" required /></label>
      <label>NPC ID<input name="npcId" value="npc_marta" required /></label>
      <label>战斗动作<select name="combatActionKind"><option value="MELEE">近战</option><option value="FIREARM">枪械</option></select></label>
      <label>目标防御<select name="combatDefense"><option value="DODGE">闪避</option><option value="NONE">不防御</option></select></label>
      <label>追逐初始距离<input name="initialRange" type="number" min="1" max="4" value="2" required /></label>
      <label>障碍 ID<input name="obstacleId" value="collapsing_salt_shelf" /></label>
      <label>障碍成本<input name="obstacleCost" type="number" min="0" max="2" value="1" required /></label>
      <label class="span-two">行动说明<textarea name="description" maxlength="300" placeholder="描述你如何调查、检查或与环境互动。"></textarea></label>
      <button class="button primary span-three" type="submit">提交行动</button>
    </form>
    ${pending ? `<div class="pending-action"><span>行动 ${escapeHtml(pending.actionId)} 等待确认</span><button class="button" data-action="confirm-action" type="button">确认服务端结果</button></div>` : ""}
  </section>`;
}

function spectatorView() {
  return `<section class="action-surface spectator-copy"><p class="eyeline">旁观模式</p><h2>行动控件不可用</h2><p>这是界面状态，不是授权边界；服务器仍会拒绝旁观者提交命令，并在投影前过滤私密事件。</p></section>`;
}

function keeperRailView() {
  const mode = state.authority?.mode || "—";
  const isAi = mode === "AI_KP";
  return `<aside class="keeper-rail" aria-labelledby="keeper-tools-title">
    <header><p class="eyeline">当前 · ${escapeHtml(mode)}</p><h2 id="keeper-tools-title">KP 工具</h2></header>
    <details open><summary>场景与安全</summary>
      <form data-form="switch-scene" class="form-stack compact">
        <label>Session ID<input name="sessionId" value="${escapeHtml(state.recent.sessionId || "")}" required /></label>
        <label>下一 Scene ID<input name="sceneId" required placeholder="scene_archive_02" /></label>
        <label>Scene 名<input name="sceneName" required placeholder="封存书库" /></label>
        <button class="button" type="submit">推进场景</button>
      </form>
      <button class="tool-button danger" type="button" data-action="safety-pause"><b>安全暂停</b><small>暂停 Session，不直接改变游戏结果</small></button>
      <form data-form="end-session" class="form-stack compact">
        <label>结束的 Session ID<input name="sessionId" value="${escapeHtml(state.recent.sessionId || "")}" required /></label>
        <button class="button" type="submit">记录 Tutorial 结局并结束</button>
      </form>
    </details>
    ${isAi ? "" : `<details open><summary>NPC / 战斗 / 追逐</summary>
      <form data-form="public-gameplay" class="form-stack compact">
        <label>玩法类型<select name="gameplayKind"><option value="NPC_INTERACTION">NPC 互动</option><option value="COMBAT_ROUND">基础战斗轮</option><option value="CHASE_SEGMENT">基础追逐段</option></select></label>
        <label>Session ID<input name="sessionId" value="${escapeHtml(state.recent.sessionId || "")}" required /></label>
        <label>调查员 ID<input name="characterId" value="${escapeHtml(state.recent.characterId || "")}" required /></label>
        <label>NPC ID<input name="npcId" value="npc_marta" required /></label>
        <label>互动方式<input name="approach" value="询问昨夜的访客记录" maxlength="500" required /></label>
        <label>NPC 公开回应<textarea name="publicResponse" maxlength="2000" required>玛塔避开视线，声称昨夜没有访客。</textarea></label>
        <label>战斗动作<select name="combatActionKind"><option value="MELEE">近战</option><option value="FIREARM">枪械</option></select></label>
        <label>目标防御<select name="combatDefense"><option value="DODGE">闪避</option><option value="NONE">不防御</option></select></label>
        <label>追逐初始距离<input name="initialRange" type="number" min="1" max="4" value="2" required /></label>
        <label>障碍 ID<input name="obstacleId" value="collapsing_salt_shelf" /></label>
        <label>障碍成本<input name="obstacleCost" type="number" min="0" max="2" value="1" required /></label>
        <button class="button approval" type="submit">由服务端结算并记录</button>
      </form>
    </details>`}
    <details open><summary>私密协作</summary>
      <form data-form="agent-job" class="form-stack compact">
        <label>Job ID<input name="jobId" required value="job_${Date.now().toString(36)}" /></label>
        <label>RAG 快照 ID<input name="ragSnapshotId" required value="tutorial_rag_1" /></label>
        <label>私密上下文<textarea name="privateNote" maxlength="512" placeholder="仅进入服务器许可的 Agent 上下文"></textarea></label>
        <button class="tool-button secret" type="submit"><b>${isAi ? "请求 AI Keeper 决策" : "请求私密检定"}</b><small>${isAi ? "正式决策经 Agent Gateway、事件日志与 WS 返回" : "HUMAN_KP 结果仅对 KP 可见"}</small></button>
      </form>
      ${isAi ? `<p class="form-note">AI_KP 正式决策不经过浏览器批准；重考虑仍走服务器工作流。</p>
      <form data-form="reconsider" class="form-stack compact">
        <label>原事件序号<input name="eventSequence" type="number" min="1" value="${escapeHtml(lastEventSequence())}" required /></label>
        <label>重考虑原因<textarea name="reason" required maxlength="500"></textarea></label>
        <button class="button" type="submit">请求重考虑</button>
      </form>` : `<form data-form="approve-agent" class="form-stack compact">
        <label>待批准 Job ID<input name="jobId" value="${escapeHtml(state.recent.agentJobId || "")}" required /></label>
        <label>Job 版本<input name="expectedVersion" type="number" min="1" value="${escapeHtml(state.recent.agentJobVersion || 1)}" required /></label>
        <button class="button" type="submit">批准 AI 草案</button>
      </form><form data-form="confirm-player-action" class="form-stack compact">
        <label>待裁定 Action ID<input name="actionId" value="${escapeHtml(state.recent.pendingAction?.actionId || "")}" required /></label>
        <label>Action 版本<input name="expectedVersion" type="number" min="1" value="1" required /></label>
        <button class="button approval" type="submit">确认服务端行动结果</button>
      </form>`}
    </details>
    <details><summary>分队管理</summary>
      <form data-form="group" class="form-stack compact">
        <label>分队 ID<input name="groupId" required placeholder="team_red" /></label>
        <label>成员用户 ID<input name="userId" required placeholder="investigator_02" /></label>
        <button class="button" type="submit">创建并分配</button>
      </form>
    </details>
  </aside>`;
}

function playerRailView() {
  return `<aside class="keeper-rail player-rail" aria-labelledby="player-tools-title">
    <header><p class="eyeline">调查员工具</p><h2 id="player-tools-title">复议与导出</h2></header>
    <form data-form="reconsider" class="form-stack compact">
      <label>原事件序号<input name="eventSequence" type="number" min="1" value="${escapeHtml(lastEventSequence())}" required /></label>
      <label>重考虑原因<textarea name="reason" required maxlength="500"></textarea></label>
      <button class="button" type="submit">请求重考虑</button>
    </form>
    <p class="form-note">调查员可生成仅含自身许可范围事件的玩家版战报。</p>
  </aside>`;
}

function decisionView(decision) {
  return `<section class="decision-panel" aria-labelledby="decision-title" data-testid="ai-decision">
    <header><div><p class="eyeline">AI_KP · 用户可见</p><h2 id="decision-title">决策说明</h2></div><span>事件 #${escapeHtml(decision.eventSequence || "—")}</span></header>
    <div class="decision-grid"><div><h3>用户可见摘要</h3><p>${escapeHtml(decision.summary)}</p></div><div><h3>依据类别</h3><p>${decision.basis.map(escapeHtml).join(" / ") || "未报告"}</p></div><div><h3>模型 / 认证</h3><p>${escapeHtml(decision.model)} · ${escapeHtml(decision.certification)}</p></div></div>
    <button class="button" type="button" data-action="focus-reconsider">请求重考虑</button>
    <p class="form-note">界面不会展示 KP 私密 prompt 或 chain-of-thought。</p>
  </section>`;
}

function gameplayProjectionView() {
  const event = [...state.events].reverse().find((candidate) => {
    if (["coc7.npc_decision_recorded", "CombatStateUpdated", "ChaseSegmentResolved"].includes(candidate?.event_type)) return true;
    const result = candidate?.payload?.ToolExecutionSucceeded?.result;
    return ["NPC_INTERACTION", "COMBAT_ROUND", "CHASE_SEGMENT"].includes(result?.kind);
  });
  const result = event?.payload?.ToolExecutionSucceeded?.result || event?.payload || state.recent.gameplayResult;
  if (!result || !["NPC_INTERACTION", "COMBAT_ROUND", "CHASE_SEGMENT"].includes(result.kind)) return "";
  const labels = {
    NPC_INTERACTION: "NPC 互动",
    COMBAT_ROUND: "基础战斗轮",
    CHASE_SEGMENT: "基础追逐段",
  };
  return `<section class="decision-panel" aria-labelledby="gameplay-result-title" data-testid="gameplay-result">
    <header><div><p class="eyeline">服务端规则结果</p><h2 id="gameplay-result-title">${escapeHtml(labels[result.kind])}</h2></div><span>事件 #${escapeHtml(event?.sequence || event?.cursor || "—")}</span></header>
    <p>${escapeHtml(result.summary || "正式玩法结果已记录")}</p>
    <p class="form-note">规则、随机数与权限均在服务端执行；此处仅投影公开结果。</p>
  </section>`;
}

function setupView(canKeep) {
  return `<div class="setup-layout">
    <section class="setup-main">
      <ol class="steps" aria-label="教学战役准备步骤"><li class="done">邀请成员</li><li class="active">创建角色</li><li>提交审核</li><li>开始 Session</li></ol>
      <section class="setup-section"><div class="section-heading"><div><p class="eyeline">角色卡</p><h2>创建、提交与审核</h2></div><span>规则验证由服务端执行</span></div>
        <form data-form="create-character" class="form-grid two">
          <label>角色名<input name="displayName" required value="林若岚" /></label>
          <label>角色 ID<input name="characterId" required value="investigator_${Date.now().toString(36)}" /></label>
          <label class="span-two">角色表 JSON<textarea class="code-input" name="sheetJson" required>${escapeHtml(defaultCharacterSheet())}</textarea></label>
          <button class="button" type="submit">保存角色</button>
          <button class="button primary" type="button" data-action="submit-character">提交审核</button>
        </form>
        ${canKeep ? `<form data-form="review-character" class="form-grid three review-form"><label>待审核角色 ID<input name="characterId" required /></label><label>角色版本<input name="expectedVersion" type="number" min="1" value="2" required /></label><button class="button approval align-end" type="submit">批准角色</button></form>` : ""}
      </section>
      <section class="setup-section"><div class="section-heading"><div><p class="eyeline">Session / Scene</p><h2>开始教学场景</h2></div></div>
        <form data-form="start-session" class="form-grid three">
          <label>Session ID<input name="sessionId" required value="session_${Date.now().toString(36)}" /></label>
          <label>房间 ID<input name="roomId" required value="tutorial_room" /></label>
          <label>Scenario ID<input name="scenarioId" required value="tutorial_mist_archive" /></label>
          <label>Scene ID<input name="sceneId" required value="scene_archive_01" /></label>
          <label>Scene Key<input name="sceneKey" required value="archive_night" /></label>
          <label>Scene 名<input name="sceneName" required value="档案室 · 雾夜" /></label>
          <button class="button primary span-three" type="submit">开始 Session</button>
        </form>
      </section>
    </section>
    <aside class="setup-aside">
      <section><p class="eyeline">邀请成员</p><h2>PLAYER / SPECTATOR</h2>
        <form data-form="issue-invite" class="form-stack compact">
          <label>成员用户 ID<input name="userId" required /></label>
          <label>角色<select name="role"><option>PLAYER</option><option>SPECTATOR</option></select></label>
          <button class="button" type="submit">签发邀请</button>
        </form>
        ${state.invite ? `<div class="secret-output"><b>只显示一次</b><code>${escapeHtml(state.invite.invite_id)}</code><code>${escapeHtml(state.invite.raw_token)}</code></div>` : ""}
      </section>
      <section><p class="eyeline">权威合同</p><h2>${escapeHtml(state.authority?.mode || "—")}</h2><dl class="metadata-list"><div><dt>合同</dt><dd>${escapeHtml(state.authority?.contract_id || "—")}</dd></div><div><dt>版本</dt><dd>${escapeHtml(state.authority?.version || "—")}</dd></div><div><dt>策略</dt><dd>FORK_ONLY</dd></div><div><dt>安全档案</dt><dd>${escapeHtml(state.authority?.snapshot?.safety_profile_version || "—")}</dd></div></dl></section>
    </aside>
  </div>`;
}

function evidenceView(canKeep) {
  const audienceOptions = canKeep
    ? '<option value="KEEPER_PRIVATE">KP 私密版</option><option value="AUDIT">审计版</option>'
    : '<option value="PLAYER">玩家版</option>';
  return `<div class="evidence-layout">
    <section><div class="section-heading"><div><p class="eyeline">正式事件</p><h1>时间线与证据</h1></div><button class="button" type="button" data-action="refresh-events">从公开 API 重放</button></div>${timelineItems(100)}</section>
    <aside><p class="eyeline">导出战报</p><h2>可验证制品</h2><form data-form="export" class="form-stack compact"><label>导出 ID<input name="exportId" value="export_${Date.now().toString(36)}" required /></label><label>视图<select name="audience">${audienceOptions}</select></label><button class="button primary" type="submit">生成并下载</button></form>${state.export ? `<pre data-testid="campaign-export">${escapeHtml(safeJson(state.export))}</pre>` : ""}</aside>
  </div>`;
}

function timelineView() {
  return `<aside class="timeline" aria-labelledby="timeline-title"><header><div><p class="eyeline">服务器过滤后</p><h2 id="timeline-title">实时事件</h2></div><button class="icon-button" type="button" data-action="refresh-events" aria-label="刷新时间线">↻</button></header>${timelineItems(20)}<div class="reconnect-line"><span>游标 #${escapeHtml(state.realtime.cursor || lastEventSequence())}</span><button class="button quiet" type="button" data-action="reconnect">重新连接</button></div></aside>`;
}

function timelineItems(limit) {
  const items = state.events.slice(-limit).reverse().map((raw) => {
    const event = eventPresentation(raw);
    return `<article class="event-item"><header><code>#${escapeHtml(event.sequence)}</code><span class="visibility">${escapeHtml(visibilityName(event.visibility))}</span></header><h3>${escapeHtml(event.title)}</h3><p>${escapeHtml(event.summary)}</p><footer>来源 · ${escapeHtml(event.provenance)}</footer></article>`;
  }).join("");
  return `<div class="event-list" data-testid="event-list">${items || `<div class="empty-state"><strong>尚无可见事件</strong><p>连接房间或从公开 API 重放后，服务器许可的事件会出现在这里。</p></div>`}</div>`;
}

function adminView() {
  return `<main id="main-content" class="page admin-page" tabindex="-1">
    <section class="page-heading"><div><p class="eyeline">Server Owner</p><h1>管理与运维证据</h1><p>独立 Admin session 只保存在当前页面内存中。</p></div></section>
    ${feedbackView()}
    <div class="admin-grid">
      <section><h2>Admin 登录</h2><form data-form="admin-login" class="form-stack"><label>管理员登录名<input name="login" autocomplete="username" required /></label><label>密码<input name="password" type="password" autocomplete="current-password" required /></label><button class="button primary" type="submit">建立 Admin session</button></form></section>
      <section><h2>AR10 诊断入口</h2><div class="button-row"><button class="button" data-action="admin-diagnostics" type="button" ${state.adminReady ? "" : "disabled"}>诊断</button><button class="button" data-action="admin-audit" type="button" ${state.adminReady ? "" : "disabled"}>审计</button></div><p class="form-note">备份与恢复保留在经审计的 Admin API；此最小界面不预填破坏性参数。</p></section>
      <section><h2>创建普通用户</h2><form data-form="admin-create-user" class="form-stack"><label>用户 ID<input name="userId" required /></label><label>登录名<input name="login" type="email" autocomplete="off" required /></label><label>初始密码<input name="password" type="password" minlength="16" autocomplete="new-password" required /></label><button class="button primary" type="submit" ${state.adminReady ? "" : "disabled"}>创建 USER</button></form></section>
      <section><h2>派生 Authority 分支</h2><form data-form="admin-fork-authority" class="form-stack"><label>父战役 ID<input name="parentCampaignId" required /></label><label>子战役 ID<input name="childCampaignId" required /></label><label>子模式<select name="authorityMode"><option value="AI_KP">AI_KP</option><option value="HUMAN_KP">HUMAN_KP</option></select></label><label>Authority Owner ID<input name="authorityOwner" required value="ai_keeper_tutorial" /></label><label>Campaign Manager 用户 ID<input name="campaignManagerUserId" required /></label><button class="button primary" type="submit" ${state.adminReady ? "" : "disabled"}>从锁定合同派生</button></form><p class="form-note">当前 Admin state #${escapeHtml(state.adminVersion ?? "—")}；原合同保持不变。</p></section>
      <section class="admin-output"><h2>脱敏证据</h2>${state.adminEvidence ? `<pre>${escapeHtml(safeJson(state.adminEvidence))}</pre>` : `<p class="muted">登录后选择诊断或审计。</p>`}</section>
    </div>
  </main>`;
}

function developerView() {
  return `<main id="main-content" class="page developer-page" tabindex="-1">
    <section class="page-heading"><div><p class="eyeline">Developer</p><h1>Agent / Tool / Event 证据</h1><p>证据来自当前 Campaign 的公开重放与 WS；敏感 prompt、token 和 chain-of-thought 在渲染前移除。</p></div><button class="button" type="button" data-action="refresh-events">刷新证据</button></section>
    ${feedbackView()}
    <div class="evidence-filters" aria-label="证据类别"><span>Agent 运行</span><span>工具调用</span><span>正式事件</span></div>
    <pre class="developer-output" data-testid="developer-evidence">${escapeHtml(safeJson(state.events.slice(-50)))}</pre>
  </main>`;
}
