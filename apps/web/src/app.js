import { ProductApi, ProductApiError, createCommand } from "/src/api.js";
import { decisionPresentation, escapeHtml, eventPresentation, safeJson } from "/src/presentation.js";
import { RealtimeClient } from "/src/realtime.js";

const root = document.querySelector("#app");
const announcer = document.querySelector("#announcer");

const state = {
  config: null,
  screen: "login",
  workspaceTab: "play",
  session: null,
  campaigns: [],
  campaign: null,
  authority: null,
  membership: null,
  events: [],
  realtime: { state: "disconnected", cursor: 0 },
  versions: new Map(),
  recent: {},
  invite: null,
  export: null,
  adminEvidence: null,
  adminReady: false,
  adminVersion: null,
  error: "",
  notice: "",
  busy: false,
};

let api;
let realtime;

start().catch((error) => {
  state.error = messageFor(error);
  root.innerHTML = fatalView();
  root.setAttribute("aria-busy", "false");
});

root.addEventListener("click", (event) => {
  const target = event.target.closest("[data-action]");
  if (!target || state.busy) return;
  void handleAction(target.dataset.action, target.dataset);
});

root.addEventListener("submit", (event) => {
  const form = event.target.closest("form[data-form]");
  if (!form) return;
  event.preventDefault();
  if (!state.busy) void handleForm(form);
});

async function start() {
  const response = await fetch("/config.json", { cache: "no-store" });
  if (!response.ok) throw new Error(`CONFIGURATION_${response.status}`);
  state.config = await response.json();
  api = new ProductApi(state.config);
  realtime = new RealtimeClient({
    baseUrl: state.config.realtimeBase,
    onStatus: (status) => {
      state.realtime = { ...state.realtime, ...status };
      if (["forbidden", "unauthenticated", "resync", "error"].includes(status.state)) {
        render();
      } else {
        updateConnectionStatus();
      }
    },
    onEvent: (event) => {
      appendEvent(event);
      render();
    },
  });
  render();
}

function render() {
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
  return `<section class="action-surface" aria-labelledby="action-title">
    <div class="section-tabs"><h2 id="action-title">Tutorial 检定</h2><span>服务端正式骰</span></div>
    <form data-form="submit-action" class="form-grid three">
      <label>角色 ID<input name="characterId" value="${escapeHtml(state.recent.characterId || "")}" required /></label>
      <label>Session ID<input name="sessionId" value="${escapeHtml(state.recent.sessionId || "")}" required /></label>
      <label>Scene ID<input name="sceneId" value="${escapeHtml(state.recent.sceneId || "")}" required /></label>
      <label>检定类型<select name="intentKind"><option value="INVESTIGATION">调查 / 线索</option><option value="SANITY_CHECK">理智检定</option></select></label>
      <label>技能名<input name="skillName" value="Library Use" required /></label>
      <label>线索 ID<input name="clueId" value="tutorial_archive_clue" required /></label>
      <label>重要性<select name="clueImportance"><option value="CORE">核心线索</option><option value="OPTIONAL">可选线索</option></select></label>
      <label>调整<select name="adjustment"><option value="NONE">无</option><option value="BONUS">奖励骰</option><option value="PENALTY">惩罚骰</option></select></label>
      <label>成功 SAN 损失<input name="successLoss" type="number" min="0" max="99" value="0" required /></label>
      <label>失败 SAN 损失<input name="failureLoss" type="number" min="0" max="99" value="1" required /></label>
      <label>游戏日键<input name="dayKey" value="tutorial_day_1" maxlength="128" required /></label>
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

async function handleAction(action, dataset) {
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

async function handleForm(form) {
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

async function login(data) {
  state.session = await api.login(data.login, data.password);
  state.screen = "campaigns";
  await loadCampaigns();
}

async function logout() {
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

async function loadCampaigns() {
  const payload = await api.listCampaigns();
  state.campaigns = Array.isArray(payload.campaigns) ? payload.campaigns : [];
}

async function openCampaign(campaignId) {
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

async function createCampaign(data) {
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

async function acceptInvite(data) {
  await api.acceptInvite(data.campaignId, data.inviteId, {
    command: createCommand("invite_accept", 1),
    campaign_id: data.campaignId,
    invite_id: data.inviteId,
    accepting_user_id: state.session.userId,
    raw_token: data.rawToken,
  });
  await loadCampaigns();
}

async function issueInvite(data) {
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

async function createCharacter(data) {
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

async function submitCharacter() {
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

async function reviewCharacter(data = {}) {
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

async function startSession(data) {
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

async function switchScene(data) {
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

async function safetyPause() {
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

async function endSession(data) {
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

async function submitAction(data) {
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

async function confirmAction(data = {}) {
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

async function requestAgentJob(data) {
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

async function approveAgentJob(data) {
  await api.approveAgentJob(state.campaign.campaign_id, data.jobId, {
    command: createCommand("agent_job_approve", Number(data.expectedVersion)),
    campaign_id: state.campaign.campaign_id,
    job_id: data.jobId,
  });
  await refreshEvents();
}

async function manageGroup(data) {
  await api.createGroup(state.campaign.campaign_id, data.groupId);
  await api.assignGroup(state.campaign.campaign_id, data.groupId, data.userId);
}

async function reconsider(data) {
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

async function requestExport(data) {
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

async function adminLogin(data) {
  await api.adminLogin(data.login, data.password);
  state.adminReady = true;
  const status = await api.adminStatus();
  state.adminVersion = Number(status.state_version);
  state.adminEvidence = await api.adminEvidence("diagnostics");
}

async function adminCreateUser(data) {
  const response = await api.adminCreateUser({
    user_id: data.userId,
    login: data.login,
    password: data.password,
  }, requiredAdminVersion());
  state.adminVersion = Number(response.state_version);
  state.adminEvidence = response;
}

async function adminForkAuthority(data) {
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

function requiredAdminVersion() {
  if (!Number.isInteger(state.adminVersion) || state.adminVersion < 0) {
    throw new Error("请先建立 Admin session");
  }
  return state.adminVersion;
}

async function refreshEvents() {
  if (!state.campaign) return;
  const replay = await api.replayEvents(state.campaign.campaign_id, 0);
  state.events = Array.isArray(replay.events) ? replay.events : [];
  state.realtime.cursor = Math.max(
    Number(state.realtime.cursor) || 0,
    Number(replay.scanned_through_sequence) || 0,
  );
}

function appendEvent(event) {
  const key = `${event.cursor || event.sequence}:${event.event_type}`;
  const seen = new Set(state.events.map((item) => `${item.cursor || item.sequence}:${item.event_type}`));
  if (!seen.has(key)) state.events = [...state.events, event].slice(-200);
  state.realtime.cursor = Math.max(Number(state.realtime.cursor) || 0, Number(event.cursor) || 0);
}

function feedbackView() {
  if (state.error) return `<div class="feedback error" role="alert">${escapeHtml(state.error)}</div>`;
  if (state.notice) return `<div class="feedback notice" role="status">${escapeHtml(state.notice)}</div>`;
  return "";
}

function navButton(action, label, icon, active, disabled = false) {
  return `<button type="button" data-action="${action}" class="nav-button ${active === action ? "active" : ""}" ${disabled ? "disabled" : ""}><span aria-hidden="true">${icon}</span>${label}</button>`;
}

function tabButton(tab, label) {
  return `<button type="button" data-action="${tab}" aria-current="${state.workspaceTab === tab ? "page" : "false"}" class="${state.workspaceTab === tab ? "active" : ""}">${label}</button>`;
}

function setBusy(value) {
  state.busy = value;
  root.setAttribute("aria-busy", String(value));
  root.querySelectorAll("button, input, select, textarea").forEach((element) => {
    if (value) element.setAttribute("data-was-disabled", String(element.disabled));
    element.disabled = value || element.dataset.wasDisabled === "true";
    if (!value) delete element.dataset.wasDisabled;
  });
}

function clearFeedback() {
  state.error = "";
  state.notice = "";
}

function announce(message) {
  announcer.textContent = "";
  requestAnimationFrame(() => { announcer.textContent = message; });
}

function focusMain() {
  requestAnimationFrame(() => root.querySelector("#main-content")?.focus());
}

function updateConnectionStatus() {
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

function connectionText() {
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

function connectionClass() {
  if (["connected", "synced"].includes(state.realtime.state)) return "good";
  if (["connecting", "reconnecting", "resync"].includes(state.realtime.state)) return "warn";
  return "bad";
}

function setVersion(key, value) {
  const normalized = Number(value);
  if (Number.isFinite(normalized) && normalized > 0) state.versions.set(key, normalized);
}

function version(key, fallback) {
  return state.versions.get(key) || fallback;
}

function requiredRecent(key, message) {
  const value = state.recent[key];
  if (!value) throw new Error(message);
  return value;
}

function lastEventSequence() {
  return state.events.reduce(
    (highest, event) => Math.max(highest, Number(event.sequence ?? event.cursor ?? 0)),
    0,
  ) || 1;
}

function visibilityName(value) {
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

function successFor(formName) {
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

function messageFor(error) {
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

function defaultCharacterSheet() {
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
    skills: { "Library Use": 75, "Spot Hidden": 60 },
    backstory_anchors: ["保护消息来源", "不会抛下同伴"],
  }, null, 2);
}

function fatalView() {
  return `<main id="main-content" class="boot-screen" tabindex="-1"><p class="eyeline">雾港调查台</p><h1>无法打开调查档案</h1><p>${escapeHtml(state.error)}</p><button class="button" type="button" data-action="reload">重试</button></main>`;
}
