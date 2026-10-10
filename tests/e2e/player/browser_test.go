//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package player_test

import (
	"encoding/json"
	"net/http"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	aicontext "github.com/zyc14588/TRPG_PLATFORM/internal/ai/context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

func TestChromePrivateRoomMixedPlaySafetyRecoveryAndCurrentAuthorization(t *testing.T) {
	var calls atomic.Int32
	f := newActualAI(t, func(w http.ResponseWriter, r *http.Request) {
		number := calls.Add(1)
		request := readAI(t, r)
		if strings.Contains(request.Messages[1].Content, PrivateValue) {
			t.Error("AI received private host state")
		}
		var payload aicontext.PayloadData
		if json.Unmarshal([]byte(request.Messages[1].Content), &payload) != nil {
			t.Error("real AI context absent")
			w.WriteHeader(500)
			return
		}
		if number%2 == 1 {
			writeAI(w, request.Model, proposalText(payload.Version))
		} else {
			writeAI(w, request.Model, "行动已执行。")
		}
	})
	server := serveBrowser(t, f.playerFixture)
	host := newChrome(t, f.server)
	host.screen(t, "01-player-home")
	host.login(t, f.login)
	host.input(t, "input[name=workspace]", f.w)
	host.button(t, "查看可用游戏")
	host.wait(t, "document.querySelectorAll('.game-card').length===3")
	host.evaluate(t, "Array.from(document.querySelectorAll('.game-card')).find(b=>b.textContent.includes('minimal')).click();true")
	host.waitText(t, "本局完整包清单")
	host.input(t, "input[name=room-name]", "浏览器创建的私人房间")
	host.button(t, "创建私人房间")
	host.waitText(t, "浏览器创建的私人房间")
	if f.sql(t, "SELECT count(*) FROM platform_room.rooms WHERE workspace_id='"+f.w+"'") != "2" {
		t.Fatal("browser did not create real private room")
	}
	host.open(t, f.w, f.room)
	host.waitText(t, "开始游戏")
	host.button(t, "开始游戏")
	host.waitText(t, "准备检查未通过")
	if f.sql(t, "SELECT count(*) FROM platform_launch.sessions WHERE workspace_id='"+f.w+"'") != "0" {
		t.Fatal("readiness denial nevertheless created a session")
	}
	t.Log("browser readiness denial preserved lobby and native session absence")
	// Re-select the server-authorized human and AI seats through the UI.
	host.input(t, "select[aria-label='席位 gm']", "human:"+f.hostPart)
	host.input(t, "select[aria-label='席位 player']", "human:"+f.playerPart)
	host.input(t, "select[aria-label='席位 ai']", "ai:selected")
	host.button(t, "保存游戏与席位")
	host.waitText(t, "保存我的同意与准备")
	guest := newChrome(t, f.server)
	guest.login(t, f.participantLogin)
	guest.open(t, f.w, f.room)
	host.consent(t)
	guest.consent(t)
	host.button(t, "刷新房间与准备")
	host.wait(t, "!document.querySelector('fieldset:disabled')")
	if host.truth(t, "document.body.innerText.includes('所有必要准备已通过')") {
		t.Fatal("old model preparation binding survived a new room revision")
	}
	// This explicit server administrator operation revalidates the existing
	// credential, certificate and budget against the changed preparation. It is
	// not a browser grant or fake ready flag; normal player selection uses only
	// the resulting authorized server selection.
	owner := f.owner.StorageValue()
	_, e := f.models.(*model.Service).Configure(f.ctx, auth.RoomSecret(model.CallerData{Credential: owner.Cookie, CSRF: owner.CSRF, IdempotencyKey: "browser-current-model-" + token(t), Network: owner.Network}), model.NewConfigureRequest(model.ConfigureRequestData{Scope: f.scope, SeatID: "ai", Selection: "selected", ExpectedVersion: 1, CredentialID: "byok", Budget: aiLimits()}))
	need(t, e)
	t.Log("server administrator explicitly rebound synthetic model to current room preparation; no browser credential grant")
	// Preparation setup, a negative launch, account login and administrator
	// reconfiguration deliberately precede the live-play journey. Let the real
	// unchanged production minute expire; do not replace/reset its limiter.
	t.Log("waiting one actual production admission window after preparation and administrator setup")
	select {
	case <-time.After(time.Minute + time.Second):
	case <-f.ctx.Done():
		t.Fatal("production admission window interrupted")
	}
	host.button(t, "刷新房间与准备")
	host.waitText(t, "所有必要准备已通过")
	host.button(t, "开始游戏")
	host.waitText(t, "游戏桌")
	guest.button(t, "刷新房间与准备")
	guest.waitText(t, "游戏桌")
	host.button(t, "连接 / 重新连接")
	guest.button(t, "连接 / 重新连接")
	continueTogether(t, host, guest)
	guest.counter(t, "1")
	guest.assertNoPrivate(t)
	// Drop only the transport response after the real SQL/Actor commit. The UI
	// must retain the original key/id, lock mutation, and query that exact result.
	server.drop.Store(true)
	guest.action(t)
	guest.waitText(t, "查询原操作结果")
	before := f.sql(t, "SELECT version FROM host_command.sessions WHERE workspace='"+f.w+"'")
	if before != "2" {
		t.Fatal("real human command did not commit once")
	}
	// Pause in the unknown-result window, before any query of the committed
	// command. Losing the local connection must not remove this safety control.
	if !guest.truth(t, "Array.from(document.querySelectorAll('button')).some(b=>b.textContent==='连接 / 重新连接'&&b.disabled) && !document.querySelector('[data-testid=game-view]')") {
		t.Fatal("unknown response did not remove the private connection projection")
	}
	guest.screen(t, "04-player-unknown-command-before-pause")
	guest.button(t, "立即安全暂停")
	guest.waitText(t, "游戏暂停：参与者请求安全暂停")
	host.waitText(t, "游戏暂停：参与者请求安全暂停")
	if server.commands.Load() != 1 || !guest.truth(t, "Array.from(document.querySelectorAll('button')).some(b=>b.textContent==='查询原操作结果'&&!b.disabled)") {
		t.Fatal("immediate pause queried or lost the original pending command")
	}
	if f.sql(t, "SELECT paused FROM platform_player.control WHERE workspace_id='"+f.w+"'") != "t" || f.sql(t, "SELECT count(*) FROM host_command.tasks WHERE workspace='"+f.w+"' AND kind='ai'") != "1" || f.sql(t, "SELECT count(*) FROM host_command.outbox WHERE workspace='"+f.w+"' AND kind='dispatch-ai'") != "1" {
		t.Fatal("immediate unknown-result pause did not persist with pending real AI work")
	}
	runtime := f.runtime(t)
	if _, e := runtime.RunOnce(f.ctx); e != task.ErrNotFound {
		t.Fatal("unknown-result pause dispatched queued AI")
	}
	if f.sql(t, "SELECT paused FROM platform_player.control WHERE workspace_id='"+f.w+"'") != "t" || f.sql(t, "SELECT count(*) FROM host_command.tasks WHERE workspace='"+f.w+"' AND kind='ai'") != "1" {
		t.Fatal("paused runtime lost the persistent control or original AI intent")
	}
	if calls.Load() != 0 || f.sql(t, "SELECT count(*) FROM platform_budget.reservations WHERE workspace_id='"+f.w+"'") != "0" || f.sql(t, "SELECT count(*) FROM platform_budget.counters WHERE workspace_id='"+f.w+"' AND ((convert_from(used,'UTF8')::json->>'Calls')::bigint<>0 OR (convert_from(used,'UTF8')::json->>'CostMicros')::bigint<>0)") != "0" {
		t.Fatal("unknown-result pause dispatched or charged AI")
	}
	guest.screen(t, "05-player-unknown-command-paused")
	// Each human explicitly confirms. The unknown original attempt stays
	// pending; only a current server control permits its idempotent replay.
	continueTogether(t, host, guest)
	if server.commands.Load() != 1 || !guest.truth(t, "document.body.innerText.includes('查询原操作结果')") {
		t.Fatal("safety confirmations replaced or automatically retried the pending command")
	}
	t.Log("actual unknown-command window paused immediately; SQL control persisted, queued AI stayed pending, zero provider calls/reservations/charges; both humans explicitly confirmed before original replay")
	guest.button(t, "查询原操作结果")
	guest.waitText(t, "已找回原行动结果")
	if server.commands.Load() != 2 || f.sql(t, "SELECT version FROM host_command.sessions WHERE workspace='"+f.w+"'") != before {
		t.Fatal("unknown-result retry repeated native mutation")
	}
	guest.button(t, "连接 / 重新连接")
	guest.counter(t, "2")
	// Participant's immediate pause gates pending real AI work, including billing.
	guest.button(t, "立即安全暂停")
	guest.waitText(t, "游戏暂停：参与者请求安全暂停")
	host.waitText(t, "游戏暂停：参与者请求安全暂停")
	if _, e := runtime.RunOnce(f.ctx); e != task.ErrNotFound {
		t.Fatal("paused AI task dispatched")
	}
	if calls.Load() != 0 {
		t.Fatal("paused AI called provider")
	}
	continueTogether(t, host, guest)
	first, e := runtime.RunOnce(f.ctx)
	need(t, e)
	second, e := runtime.RunOnce(f.ctx)
	need(t, e)
	if first.Executed != 1 || first.Applied != 1 || second.Executed != 1 || second.Applied != 1 || calls.Load() != 2 {
		t.Fatal("real AI proposal/narrative Actor roundtrip incomplete")
	}
	guest.counter(t, "3")
	guest.assertNoPrivate(t)
	t.Log("actual synthetic local AI proposal committed before narrative; paid model qualification NOT_RUN")
	host.button(t, "建立恢复点")
	host.waitText(t, "恢复点版本")
	guest.input(t, "select[name=kind]", "personal")
	guest.input(t, "input[name=export-limit]", "1")
	guest.button(t, "生成权限过滤导出")
	guest.wait(t, "document.querySelector('[data-testid=export]')!==null")
	guest.assertNoPrivate(t)
	guest.waitText(t, "还有后续页面")
	pages := 1
	for guest.truth(t, "document.querySelector('[data-testid=export]')?.dataset.hasMore==='true'") {
		if pages >= 8 {
			t.Fatal("bounded fixture export did not finish pagination")
		}
		var cursor string
		_ = json.Unmarshal(guest.evaluate(t, "document.querySelector('[data-testid=export]').dataset.nextCursor"), &cursor)
		guest.button(t, "读取下一页")
		guest.wait(t, "document.querySelector('main').getAttribute('aria-busy')==='false'&&document.querySelector('[data-testid=export]')?.dataset.nextCursor!=="+js(cursor))
		guest.assertNoPrivate(t)
		pages++
	}
	guest.waitText(t, "已到末尾")
	guest.download(t)
	t.Logf("actual personal export completed %d authoritative cursor pages and verified downloaded filtered JSON", pages)
	guest.assertNoPrivate(t)
	// No fake local state: disconnect removes the private view, both humans must
	// explicitly reconfirm after a real new server lease.
	guest.button(t, "离开游戏连接")
	guest.waitText(t, "重新连接后请确认继续")
	guest.wait(t, "document.querySelector('[data-testid=game-view]')===null")
	host.waitText(t, "游戏暂停：必要参与者连接中断")
	guest.button(t, "连接 / 重新连接")
	guest.counter(t, "3")
	continueTogether(t, host, guest)
	if !guest.truth(t, "document.querySelector('[data-testid=game-view]').textContent.includes('choose-player')") {
		t.Fatal("pending seat action did not recover")
	}
	guest.screen(t, "02-player-filtered-game")
	// Revocation is observed by current-cookie polling, clearing view and export.
	_ = f.sql(t, "UPDATE platform_auth.sessions SET revoked=true WHERE account_id='"+f.participant.StorageValue().ID+"'")
	guest.waitText(t, "登录已失效")
	guest.wait(t, "document.querySelector('[data-testid=game-view]')===null && document.querySelector('[data-testid=export]')===null")
	guest.assertNoPrivate(t)
	t.Logf("real concurrent resume RATE_LIMITED responses observed and recovered by explicit confirmation: %d", server.resumeLimited.Load())
	t.Log("Chrome account creation-room selection, readiness denial, mixed seats, actual human/AI play, unknown-result dedup, pause, disconnect/reconnect, filtered export, recovery and revocation passed")
}

func TestChromeGuestInvitationApprovalExchangeAndClaim(t *testing.T) {
	f := newActualAI(t, func(w http.ResponseWriter, r *http.Request) {
		t.Error("admission flow dispatched model")
		w.WriteHeader(500)
	})
	serveBrowser(t, f.playerFixture)
	host := newChrome(t, f.server)
	host.login(t, f.login)
	host.open(t, f.w, f.room)
	host.button(t, "创建邀请")
	host.wait(t, "document.querySelector('[data-testid=room-code]')!==null")
	var code string
	_ = json.Unmarshal(host.evaluate(t, "document.querySelector('[data-testid=room-code]').textContent"), &code)
	visitor := newChrome(t, f.server)
	visitor.input(t, "input[name=invite]", code)
	visitor.input(t, "input[name=nickname]", "受邀的朋友")
	visitor.button(t, "申请入场")
	visitor.waitText(t, "等待房主批准")
	host.button(t, "刷新房间与准备")
	host.waitText(t, "等待入场")
	host.button(t, "批准入场")
	visitor.button(t, "查看申请结果")
	visitor.waitText(t, "受邀访客")
	visitor.waitText(t, "本局完整包清单")
	visitor.assertNoPrivate(t)
	participantID := f.sql(t, "SELECT id FROM platform_room.participants WHERE workspace_id='"+f.w+"' AND room_id='"+f.room+"' AND guest_id IS NOT NULL")
	if participantID == "" {
		t.Fatal("guest participant identity missing")
	}
	if visitor.truth(t, "document.body.innerText.includes('创建私人房间') || document.body.innerText.includes('游戏与席位安排')") {
		t.Fatal("guest received management controls")
	}
	visitor.evaluate(t, "Array.from(document.querySelectorAll('summary')).find(e=>e.textContent==='认领访客身份').click();true")
	visitor.input(t, "input[name=login]", "claimed_"+prefix)
	visitor.input(t, "input[name=name]", "认领后的朋友")
	visitor.input(t, "input[name=password]", "synthetic-claim-password")
	visitor.button(t, "认领本局身份")
	visitor.waitText(t, "认领后的朋友")
	if f.sql(t, "SELECT count(*) FROM platform_core.memberships WHERE workspace_id='"+f.w+"'") != "1" {
		t.Fatal("claim granted workspace membership")
	}
	if f.sql(t, "SELECT count(*) FROM platform_room.participants WHERE workspace_id='"+f.w+"' AND room_id='"+f.room+"'") != "3" {
		t.Fatal("claim changed participant identity cardinality")
	}
	if f.sql(t, "SELECT id FROM platform_room.participants WHERE workspace_id='"+f.w+"' AND room_id='"+f.room+"' AND id='"+participantID+"' AND account_id IS NOT NULL AND active=true") != participantID {
		t.Fatal("claim replaced the admitted participant identity")
	}
	visitor.assertNoPrivate(t)
	visitor.wait(t, "!document.querySelector('fieldset:disabled')")
	// A successful identity claim is incomplete if the same admitted person can
	// no longer inspect the selected packages and confirm participation. Probe
	// the current-room endpoint, without granting workspace membership
	// or treating denial of the whole-workspace catalog as the normal journey.
	var presentationStatus int
	_ = json.Unmarshal(visitor.evaluate(t, "fetch("+js("/api/v1/workspaces/"+f.w+"/rooms/"+f.room+"/presentation")+",{credentials:'same-origin',cache:'no-store',referrerPolicy:'no-referrer'}).then(r=>r.status)"), &presentationStatus)
	if presentationStatus != http.StatusOK {
		t.Fatalf("claimed participant cannot read its current room presentation: status=%d", presentationStatus)
	}
	visitor.waitText(t, "本局完整包清单")
	visitor.consent(t)
	visitor.call(t, "Emulation.setDeviceMetricsOverride", map[string]any{"width": 390, "height": 844, "deviceScaleFactor": 1, "mobile": true}, nil)
	visitor.wait(t, "document.documentElement.scrollWidth<=innerWidth")
	visitor.screen(t, "03-player-claimed-room-mobile")
	if f.sql(t, "SELECT count(*) FROM platform_launch.acknowledgments WHERE workspace_id='"+f.w+"' AND room_id='"+f.room+"' AND participant_id='"+participantID+"' AND (convert_from(body,'UTF8')::json->>'Consent')::boolean AND (convert_from(body,'UTF8')::json->>'Ready')::boolean AND (convert_from(body,'UTF8')::json->>'SafetyConfirmed')::boolean") != "1" {
		t.Fatal("claimed participant did not continue personal consent in the same room")
	}
	t.Log("actual guest invitation approval, exchange and account claim preserved participation without management grant")
}

// Concurrent participants may receive the server's one-slot urgent-operation
// backpressure or a changed control revision. The UI keeps that safe refusal
// visible, refreshes the authoritative revision, and needs a new explicit vote.
func continueTogether(t *testing.T, host, participant *chrome) {
	t.Helper()
	const label = "我已确认边界，继续游戏"
	for _, c := range []*chrome{host, participant} {
		c.button(t, label)
	}
	for _, c := range []*chrome{host, participant} {
		c.wait(t, "document.querySelector('main').getAttribute('aria-busy')==='false'")
	}
	for turn := 0; turn < 3; turn++ {
		for _, c := range []*chrome{host, participant} {
			if !c.truth(t, "Array.from(document.querySelectorAll('button')).some(b=>b.textContent==="+js(label)+")") {
				continue
			}
			if !c.truth(t, "['操作过于频繁，请稍后再试。','房间或游戏已发生变化，请刷新后重新确认。'].includes(document.querySelector('.feedback [role=alert]')?.textContent)") {
				t.Fatal("resume did not finish and had no recoverable authoritative refusal")
			}
			if c == participant {
				c.assertNoPrivate(t)
			}
			t.Log("browser observed safe concurrent resume refusal; participant explicitly confirms refreshed current revision")
			c.button(t, label)
			c.wait(t, "document.querySelector('main').getAttribute('aria-busy')==='false'")
		}
	}
	host.waitText(t, "游戏进行中")
	participant.waitText(t, "游戏进行中")
}
