---
document_id: SPEC-M1-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# M1 包规范与最小一致性游戏范围及出口门禁

<a id="SPEC-M1-PURPOSE"></a>
## 1. 目的

M1 冻结 package identity、manifest、dependency lock 与 Host API 的首个可执行契约，选定并验证符合 Platform Lua 5.5 Profile 的 VM implementation candidate，实现最小权威 Session、command/event、package load 与 recovery/replay fixture。`fixture-minimal` 作为长期兼容、迁移和部署 smoke fixture 保留。

本里程碑只落实 `SPEC-V1-ROADMAP-M1` 以及当前标记为 `M1` 的 requirement、test 和 traceability 行，不增加产品范围。

<a id="SPEC-M1-B010-OWNER-EXCEPTION"></a>
### 1.1 M1-B010 项目所有者窄例外

`OWNER-DECISION-PLATFORM-CHANGE-R2-INT-010-REM-001` 仅为 `M1-B010` 增加以下 M1 范围：通用 Game Package namespaced JSON extension、Host canonical preservation、`SCHEMA_VALIDATED_GENERIC_JSON_EDITOR` 的 import / inspect / edit / validate / export / re-import 最小往返，以及第一方和第三方走同一路径的跨仓库 probe。该例外不移动或完成任何 M6 Creator requirement，不授权完整 Creator Studio、游戏专用编辑器、正式装备语义或 runtime，也不改变 `M1-B002` 的冻结合同。

`M1-B010` 的 Phase K 本地集成目标固定为从 `02dd02447ed07fe6248108365847dccf4023ca00` 建立的 `m1/b010-game-package-extensions`；不得把候选合入或改写 `m1/b002-lua-runtime`。只允许在完整独立验收后 `git merge --ff-only`，默认不 push、不创建 PR。

<a id="SPEC-M1-ALLOWED"></a>
## 2. 允许

- package 类型、不可变 `package_id`、语义版本、内容哈希、构建来源、manifest、capability、精确 dependency lock、单 Session 单版本与循环依赖拒绝的首个可执行契约（`REQ-PACKAGE-001`—`005`）；
- 隔离校验后的原子 package install，以及精确 package lock、事件兼容和安全边界迁移的最小合同与 fixture（`REQ-PACKAGE-005`、`008`）；
- 首版 Host API/Callback：标准入口、版本、最小 capability、事务工作区、受控数据接口、原子 commit/rollback、资源预算和分级审计（`REQ-LUA-003`、`004`、`006`）；
- Lua 5.5 source-only Platform Profile、危险库与生产 Debug 禁令、每 Session 独立长期 VM，以及从权威状态和显式 checkpoint 重建所需的 implementation candidate 选择与验证（`REQ-LUA-001`、`002`、`006`）；
- 最小 `SessionActor`、有界 Mailbox、command envelope、幂等 command、event/outbox 原子提交及 commit 后广播路径（`REQ-SESSION-002`、`003`）；
- 事件权威、snapshot/projection 可重建、固定 replay 与 package/schema/callback/migration 边界 fuzz（`REQ-DATA-001`、`REQ-QUALITY-003`）；
- package、Session、Lua、Host Callback 不得突破的安全硬边界（`REQ-SEC-001`）；
- 可重复加载并恢复/重放的 `fixture-minimal`，长期用于兼容、迁移和部署 smoke 验证。
- 仅限 `M1-B010` 的通用 namespaced JSON extension、Host 无损保存和最小 schema-validated Creator 往返，精确合同见 `SPEC-PACKAGE-EXTENSIONS-M1-B010`。

<a id="SPEC-M1-FORBIDDEN"></a>
## 3. 禁止

- 完整账户系统、正式 Room/Lobby/邀请与玩家房间体验；
- 完整 AI gateway、模型路由、AI 席位产品能力或 browser-local model；
- 官方隐藏信息桌游、官方 TRPG 与 Campaign 产品能力；
- 除 `SPEC-M1-B010-OWNER-EXCEPTION` 固定的通用 JSON 最小编辑面外，Creator Studio 的实际编辑能力；
- public marketplace、正式 package 信任/撤销运营面；
- 正式备份系统、生产部署与后续里程碑的运维能力；
- 任何不属于当前 M1 requirement/test/traceability 行的 V1 功能。

<a id="SPEC-M1-EXIT"></a>
## 4. 出口门禁

M1 只有在下列现有机器门禁全部通过并提供“可重复命令及退出码、机器报告或 CI Artifact、精确 Commit SHA”后才能关闭：

- `TEST-PACKAGE-001`、`002`、`003`、`004`、`005`、`008`：package 分层、身份与发布物、最小 capability、精确依赖、原子安装及升级兼容均符合对应 requirement；
- `TEST-SESSION-002`、`003`：每个活跃 Session 的单写入 Actor/有界 Mailbox 与先提交后广播、command 幂等路径成立；
- `TEST-LUA-001`、`002`、`003`、`004`、`006`：Lua 5.5 Profile、Session VM、事务回滚、数据库能力分级、资源预算和审计成立；
- `TEST-DATA-001`：事件保持权威，snapshot/projection 与 VM 可从权威状态和 checkpoint 重建，replay 不重新调用外部模型；
- `TEST-SEC-001`：身份、席位、隐藏信息、租户、sandbox、secret 与 audit 硬边界不能被 package 绕过；
- `TEST-QUALITY-003`：`fixture-minimal` 提供版本化固定 replay 场景，并覆盖协议、package、schema、callback 与 migration 边界 fuzz/smoke；
- 退出证据只覆盖上述 M1 rows；不得以完成后续里程碑功能替代任何 M1 门禁。
