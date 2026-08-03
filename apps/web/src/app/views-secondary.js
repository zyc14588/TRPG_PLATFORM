import { escapeHtml, eventPresentation, safeJson } from "/src/presentation.js";
import { state } from "./context.js";
import { lastEventSequence, visibilityName } from "./support.js";
import { feedbackView } from "./view-support.js";

export function timelineView() {
  return `<aside class="timeline" aria-labelledby="timeline-title"><header><div><p class="eyeline">服务器过滤后</p><h2 id="timeline-title">实时事件</h2></div><button class="icon-button" type="button" data-action="refresh-events" aria-label="刷新时间线">↻</button></header>${timelineItems(20)}<div class="reconnect-line"><span>游标 #${escapeHtml(state.realtime.cursor || lastEventSequence())}</span><button class="button quiet" type="button" data-action="reconnect">重新连接</button></div></aside>`;
}

export function timelineItems(limit) {
  const items = state.events.slice(-limit).reverse().map((raw) => {
    const event = eventPresentation(raw);
    return `<article class="event-item"><header><code>#${escapeHtml(event.sequence)}</code><span class="visibility">${escapeHtml(visibilityName(event.visibility))}</span></header><h3>${escapeHtml(event.title)}</h3><p>${escapeHtml(event.summary)}</p><footer>来源 · ${escapeHtml(event.provenance)}</footer></article>`;
  }).join("");
  return `<div class="event-list" data-testid="event-list">${items || `<div class="empty-state"><strong>尚无可见事件</strong><p>连接房间或从公开 API 重放后，服务器许可的事件会出现在这里。</p></div>`}</div>`;
}

export function adminView() {
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

export function developerView() {
  return `<main id="main-content" class="page developer-page" tabindex="-1">
    <section class="page-heading"><div><p class="eyeline">Developer</p><h1>Agent / Tool / Event 证据</h1><p>证据来自当前 Campaign 的公开重放与 WS；敏感 prompt、token 和 chain-of-thought 在渲染前移除。</p></div><button class="button" type="button" data-action="refresh-events">刷新证据</button></section>
    ${feedbackView()}
    <div class="evidence-filters" aria-label="证据类别"><span>Agent 运行</span><span>工具调用</span><span>正式事件</span></div>
    <pre class="developer-output" data-testid="developer-evidence">${escapeHtml(safeJson(state.events.slice(-50)))}</pre>
  </main>`;
}
