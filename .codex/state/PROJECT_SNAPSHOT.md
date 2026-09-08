---
document_id: CODEX-PROJECT-SNAPSHOT
schema_version: 1
document_kind: state-summary
authority: state-summary
status: ACTIVE
source_commit: "28ddda38e0a426be314ae8af2298672a61e972f9"
---

# 项目摘要

## M1-B002 在已验收 B011 上的生命周期接续候选

本节为 plan v26 的治理候选投影；精确候选 Commit/tree 由外部验收绑定，避免自引用。未接收前，主线 authority 仍是 `b67298de2861643e61794ace51b71891d6417320`、plan v25、`M1-B002 = BLOCKED`。本候选不证明 runtime 已实现，不代替独立 ACCEPT、owner 接收或 human merge，不授权本会话启动业务角色。

- 实际 native batch/task target：`M1-B002`；本次治理入口是 `PLAN M1/MILESTONE`，role `planner`。没有新分配的治理 task 或 M1-B012。
- plan revision：`25 -> 26`；唯一 batch 状态变化：`M1-B002: BLOCKED -> IMPLEMENTING`。这是原生 `validBatchStateTransition` 允许的接续状态；不修改 frozen contract，不是重新定义目标/需求/scope 的 replan。
- 原冻结合同仍在 `.codex/state/MILESTONE_PLAN.yaml` 的 M1-B002 条目，SHA-256 `5dbcccd9ddda20bbcbdcbff473fb9197b83bb087a10f1ee802ed8ad547d74677`。没有独立 contract revision 字段，没有新冻约或摘要替换。本文是 always-read 状态/基线投影，不是平行执行合同。
- 唯一 Lua runtime 主责任仍为 B002，拥有 `REQ-LUA-001/002` 和 `TEST-LUA-001/002`。`M1-B012` 未分配；`next_batch_sequence=12` 不变；所有其他 batch、dependencies、tombstones 和 parallel fields 均不变；WIP=1。
- 已验收 B011 产品/主线 baseline：commit `b67298de2861643e61794ace51b71891d6417320`，tree `4c4d3e613c48dcd829192da7e2eddcc9fd1b1d11`。验收记录 `M1-B011-PLATFORM-REMOTE-INTEGRATION-INDEPENDENT-ACCEPT` / `PASS_M1_B011_PLATFORM_REMOTE_INTEGRATION_ACCEPTED` 固定该 identity；B011 全部已验收能力继承。下方旧 preintegration snapshot 保留原文，只代表当时阶段，不撤销该后续已接收事实。
- 实现性质：`FIRST_IMPLEMENTATION_ON_ACCEPTED_B011_LINEAGE`。当前树只有 M0 lua-runner shell，没有 internal/luaruntime；未来不是对当前已有实现做两项小修。没有选择整合历史候选，也没有声明其 backend 合格。
- 生效实现 baseline：本候选正式接收/集成后的精确 authority SHA，由接收回执和新的 native reading map 固定；当前为 `NOT_EFFECTIVE_PENDING_ACCEPTANCE`。不得把未接收候选 SHA 当成已生效主线，不允许回到历史 B002 checkout 施工。
- 接收后按本 plan 状态映射得到的业务 mode/role 为 `IMPLEMENT / builder`，对象为 B002 完整原冻结合同；本轮只进行该路由的只读解析验证，不启动 Builder。旧 c1 的 `REPAIR / repair` envelope 不适用于 B011。

### B002 冻结义务和 finding lineage

原 objective、non_goals、requirements、allowed/forbidden scope、machine_contracts、reading_map_sections、acceptance、tests、stop_conditions、depends_on 与 parallel fields 逐项保留，原摘要不变。Machine contracts 为 `REQ-LUA-001/002`、`TEST-LUA-001/002`、`R14-A15/A22`、`R15-A09/A11/A13`。Frozen reading sections 为 `SPEC-V1-ROADMAP-M1`、`SPEC-M1-ALLOWED/FORBIDDEN/EXIT`、`SPEC-LUA-RUNTIME-001`、`SPEC-PACKAGE-001`、`SPEC-SECURITY-001`、`SPEC-QUALITY-001`；它们仍由原生 route materialization 强制覆盖。

| 必须继承的原义务 | 实现边界与未来验收 |
| --- | --- |
| pinned、许可兼容的 Lua 5.5 Platform Profile，真实语义一致性；没有合格 candidate 则停止 | profile/backend；TEST-LUA-001。没有冻结指定第三方库或 CGo/纯 Go 方案，历史 `golua/v2@v2.0.5` 不是新选型结论。 |
| 仅 UTF-8 source；拒绝 bytecode、io/os/debug、native loader、C module、动态库、文件/网络/进程/环境/原始凭据；无 production REPL | loader/production profile/runner；TEST-LUA-001 的每项拒绝负例，不能只执行 trivial script。 |
| 同主机独立 Lua process、本地双向有界 IPC、无公网接口/DB credentials/Go pointers、无权威写入 | cmd/lua-runner 与 ipc；进程与消息边界/错误传播测试，正式 Host Callback 属于 B004。 |
| 每 Session 独立长期 VM；globals/modules/coroutines/random/capability handles 不共享；结束销毁不影响其他 Session | vm；TEST-LUA-002 生命周期/销毁/交叉污染与 multi-Session race。 |
| checkpoint 仅 nil、bool、受限数值、UTF-8 string、规范数组/字符串键表；拒绝函数、闭包、coroutine、userdata、句柄、环、metatable 行为和能力 token | checkpoint；TEST-LUA-002 允许值 roundtrip、全部禁止值及不兼容绑定负例。 |
| checkpoint 绑定状态版本、包 hash、exact lock、Lua Profile、runtime version；从 authoritative state+checkpoint 重建；内存压力提前重建须先取得当前一致 checkpoint | checkpoint/vm；TEST-LUA-002 恢复等价、故障和内存压力；不得序列化 opaque VM memory 或只在 globals 保存权威事实。 |
| 执行失败污染 VM，下一命令前重建；包括执行后返回值转换错误 | vm/ipc；继承 ACC-M1-B002-008：部分写 globals 后返回 function 等不允许值，后续执行必须拒绝直到重建。 |
| CPU/指令、wall-clock、memory、递归、输出等 runtime 硬预算与取消/超时隔离 | profile/vm/ipc；TEST-LUA-001/002 的资源耗尽/取消/恢复测试；不能靠只在执行前后检查 context 允许无限脚本继续。 |
| declared ∩ trust ∩ execution-context capability；required 缺失拒绝、optional 有 tested fallback、无动态提权；权威 random/time 来自 Host API；稳定序列化/恢复 | 复用现有 package capability/lock model；runtime 必须拒绝未提供能力，跨 VM/过期/伪造句柄负例；完整事务与事件 replay 由现有下游批次承接。 |
| TEST-LUA-001/002 各有真实重复命令、exit code、machine evidence、exact candidate SHA | runner evidence；继承 ACC-M1-B002-009，拒绝/消除 test-selection overrides，证明必需测试实际执行；零执行绝不是 PASS。 |

Host Callback 次数、patch/DB rows/bytes/events/tasks、MutationWorkspace commit/rollback、AUDIT-0 及更高级审计的完整验收仍由 B004 的 `REQ-LUA-003/004/006` 和 `TEST-LUA-003/004/006` 主拥有；B002 的 runtime 安全、预算、污染与拒绝边界不能因其尚未实现而减免。正式 Session/replay/组合安全仍归 B005/B006/B008。该分工沿用现有合同，不创建 B002→B004 的反向依赖。

历史 FAIL candidate `3a1e45b464ac66f4d3592e0574a27810205a6073`，tree `bce928aa7d3f8ff9e4c67947659f68a335e1684d`；blocking commit `27bc3b7ecf870a27348516ff950526c4fea5f0ed`。原生 finding authority `native-acceptance:sha256:d1dcdb91ab2ba70fd7d3093d1dc9d8d20f9b548dc02468b65b1d5cdb65ac1d75`，finding-set digest `cf4a102beb92e6b63c4b78bc8d39c53cc329289a2e123d1be07764a2d2fbd9cd`。008/009 均继续 `OPEN_HISTORICAL`、owner B002，必须在未来实际 candidate 独立验收中明确 disposition；本状态接续不声称已修复或在 B011 已复现。

旧 controller task `M1-B002/c1` 保留其 immutable baseline `a6ce0e1f1f7cddcd191899946093626a81e00111` 和 `REPAIR / repair` envelope。它属于历史谱系，在当前产品计划原本已注明 inactive/not resumed；其 lease=0、RUNNING role=0 的观测不授权修改 ledger，也不把它重新定义为当前 source。此候选既不重绑旧 task，也不要求主机/controller 恢复。若未来另行使用旧 controller，必须先具备其适用的来源/登记 authority，不得把当前 projectctl reading-route PASS 当成旧 controller task 可运行的证明。

### B011 现存接口、路径和未来测试

所有 `PLANNED_NEW` runtime 子目录已包含在原 B002 frozen allowed_scope；目前仍未建立，不能对不存在入口跑测试再记作 runtime FAIL。

| 路径 | 状态 | 真实符号/未来边界 |
| --- | --- | --- |
| `cmd/lua-runner/main.go` | EXISTING | 当前仅 `baselinecli.Run` M0 shell；未来 runner lifecycle/process 入口。 |
| `cmd/lua-runner/evidence.go` | PLANNED_NEW | 未来候选绑定证据，不假称当前已有 evidence subcommand。 |
| `internal/luaruntime/profile/`、`vm/`、`checkpoint/`、`ipc/`、`testdata/` | PLANNED_NEW | 原 frozen scope 内完整 Profile、VM、恢复、IPC 与 fixtures。未授权在这些子目录外任意新增 runtime 文件。 |
| `go.mod`、`go.sum` | EXISTING | 原合同未来允许必要依赖选型；本治理轮不新增或升级依赖。 |
| `internal/package/manifest/manifest.go` | EXISTING | `Parse`/`Document`/`Package`：TOML→normalized model/error，含 Entrypoint/LuaProfile/HostAPIRange/Capabilities/Dependencies/Extensions；`validateRuntime` 要求 runtime 三字段成组、规范相对 .lua 路径，声明不等于执行。 |
| `internal/package/archive/model.go` | EXISTING | `FromFiles`/`Import`/`ImportBytes`→validated immutable Package；`Manifest()`、`ExactLock()`、`ContentHash()`、`ArtifactIdentity()`、`Entry(path)` 和 `Entry.Bytes()` 提供隔离副本及规范 source bytes。 |
| `internal/package/archive/project.go`、`writer.go`、`snapshot.go` | EXISTING | `ImportProject` 使用受限目录读取和相同 canonical model；`Package.Export()`→deterministic Snapshot/error，`Snapshot.Bytes/Hash` 为最终 ZIP identity。 |
| `internal/package/extension/extension.go`、`internal/package/archive/model.go` | EXISTING | `Descriptor/Support/Load`、`Document.ValidateReplacement`、`Package.ReplaceExtension` 保持 generic namespaced JSON、required unsupported 拒绝、optional unknown raw bytes 无损；extension 不能覆盖 core runtime 字段。 |
| `apps/creator-studio/creator/service.go`、`apps/creator-studio/main.go` | EXISTING | `Service.ImportArchive/Inspect/Edit/Export` 与 `runExtensionInspect/runExtensionEdit` 共享 archive+schema 校验；Edit 输入 conflictToken/namespace/JSON→EditResult/error，Export→ExportResult/error；已有 headless edit/validate/export/reimport，不增加 Creator 特权旁路。 |
| `internal/package/capability/capability.go` | EXISTING | `Resolve(declaration,level,policy,execution)`→Resolution/error，复用既有 capability 交集。 |
| `internal/package/install/`、`internal/hostapi/` | PLANNED_NEW | 仅下游 B003/B004 的未来路径，非本轮或 B002 写范围。 |

拟议 package-to-runtime adapter 放在原允许的 vm/profile/ipc 边界，只消费 `Package.Manifest/ExactLock/ContentHash/Entry`，按 `Entrypoint` 取得校验 source，绑定包 identity/profile/lock，进入独立 runner。以真实 v1/v2 B011 包 fixtures 验证加载/执行、缺入口/错 profile 拒绝、core override 禁止和原 extension 无损。包、Creator 的 EXISTING 文件只读；若必须修改其产品契约或 scope，触发原停止条件。

错误类别须区分装载/输入拒绝、profile/capability 拒绝、脚本失败、取消/预算、IPC/runner 故障、checkpoint 值/绑定失败、VM 污染/已销毁；具体新类型/码值尚未实现，不能把历史符号冒充当前 API。通过实际 typed error/有限响应验证传播、不泄露秘密和不继续执行污染 VM。

原测试命令完整保留：`go test ./internal/luaruntime/profile/... ./cmd/lua-runner/...`（TEST-LUA-001）；`go test ./internal/luaruntime/vm/... ./internal/luaruntime/checkpoint/...`（TEST-LUA-002）；`go test -race ./internal/luaruntime/...`；clean tracked tree 下 `just check`、`just test`、`go vet ./...`、`just license-check`。未来 IPC/包到 runtime/取消/008/009 regressions 在这些原 test targets 内落实。新增业务测试均 `PLANNED_NOT_RUN`。

B011 不回归的真实现存入口：`go test ./internal/package/... ./cmd/creator-cli/... ./apps/creator-studio/...`、Creator `TestHeadlessInspectEditValidateExportReimport`、以及当前三平台 `just ci` 分流。本轮均 `NOT_RUN_NO_BUSINESS_CANDIDATE`，不重用 B011 PASS 充当 Lua 验收。

平台责任保持 `SPEC-RELEASE-GATE-MATRIX`：Core/platformd/workerd/lua-runner 仅 Linux amd64 原生；Compose 为 Linux amd64，Windows 走 WSL2/Docker Desktop；Player 为 Windows/Linux/macOS Chromium；Creator Studio/CLI 为 Windows/Linux。`platformCIPlanForGOOS` 的 Linux 执行全部 Go build/test/vet+native Studio/Compose，Windows 执行 portable package/Creator 的 native build/test/vet，macOS 执行 projectctl 和 frontend gates。三平台 workflow 都调用 Just，不等于三个 OS 都原生执行 Lua；cross-build 不能当 native execution。原全 Go tests/vet 义务在正式 Linux Core 平台执行，不通过禁用必需能力令构建变绿。

### 下游与停止边界

依赖不变：B002←B001；B003←B001/B002；B004←B001/B002/B003；B005←B003/B004；B006←B002/B005；B007←B003/B006；B008←B003/B004/B005/B006/B007；B009←B007/B008；B010/B011←B001。无环，无第二个 Lua 主责任。B003 安装需要已验收 Lua profile/runner；B004 正式 Host API 需要 runtime lifecycle/IPC/errors/isolation 及 B003 安装/存储前置。此顺序来自现行 graph，不强制增加反向交叉依赖。

原四项 stop conditions 不变：不得削弱 Lua 5.5/source-only/process/checkpoint/production-debug 标准；不得接受未定/不兼容许可或未 pin 关键依赖；不得用 opaque VM memory/functions/coroutines/userdata 或缺失权威事实恢复；不得扩大到 Host API/Session/AI/Room/official game/Creator。原非目标及全部禁止目录保持原文。未来实施交付必须满足完整原冻约和 finding 回归，而非最小壳。

本轮 `BUSINESS_CODE_CHANGED=NO`、`LUA_BUILDER_STARTED=NO`、`LUA_IMPLEMENTATION_TESTS=NOT_RUN`、`BUSINESS_FULL_SUITE=NOT_RUN_NO_BUSINESS_CANDIDATE`、`REMOTE_WRITES=NONE`。候选接收前 `IMPLEMENTATION_AUTHORIZED=NO`；唯一下一 gate 为此治理候选的独立 ACCEPT 与原生要求的 owner/human 接收，不释放实现启动提示词。

## 历史 B011 preintegration 摘要（原文保留）

以下文字保留 b67298d 所承载的历史阶段及 evidence identity；其中“当前”“下一 gate”“未集成”均为该历史快照语境，不覆盖上文已经读取的实际 B011 remote-integration acceptance 或本候选状态。


产品：AI 原生在线桌面游戏平台。

V1：官方隐藏信息桌游 + 规则异常 TRPG + Creator Studio + 托管/自托管。

技术：单节点 Go、Lua 5.5 游戏包、React Web、Wails Studio、PostgreSQL。

M0 已独立验收 `PASS`、合并并关闭。当前边界为 `M1`，计划版本为 `25`，`next_batch_sequence = 12`。

`M1-B001` 已完成；`M1-B002` 为 `BLOCKED / GOVERNANCE_STATE_REPROJECTION_ONLY`；`M1-B003`—`M1-B009` 为 `PLANNED`；`M1-B010` 为 `COMPLETED / HISTORICAL_COMPLETION_PROJECTION`；`M1-B011` 为 `COMPLETED / LINEAGE_CLEAN_CONTENT_RECONSTRUCTION`。当前 preintegration governance rebind candidate 的 active verification target 为 `M1-B011-PLATFORM-PREINTEGRATION-GOVERNANCE-REBIND-INDEPENDENT-ACCEPT`。

`M1-B002` 的 blocked historical authority 是 `27bc3b7ecf870a27348516ff950526c4fea5f0ed`，冻结契约仍为 `5dbcccd9ddda20bbcbdcbff473fb9197b83bb087a10f1ee802ed8ad547d74677`。B002 product code is NOT integrated into this lineage；其 `BLOCKED` 状态是 independently established historical evidence 的治理投影；product code imported: `false`。

`M1-B010` 的冻结契约 `452846abd429474fb57aaab1a4247308df5b78ea819601e113bd2a33d2368583` 保持精确历史等价。历史 lifecycle authority 为 `cde53640e143b7adb9e36d7d8bc4fedb6f0ed401`；accepted code/tree 为 `1339c490bd8cae1e2a6e6607fc0f1db019faab64` / `25d5645caf883a73f165db0179accbca48ddf526`；platform ACCEPT 为 `PASS`；`R2-INT-010` 为 `RESOLVED_ADAPTER`。这些都是历史完成事实投影：B010 product code integrated into clean lineage 为 `false`，当前 tree 不具有 B010 capability，旧 main integration 已由 `M1-B011` clean reconstruction 取代。

旧 plan authority `ea211cfaa0f9e6008da68688b076db281c8a45df` 与旧 B011 contract `b2a2ca0d410b2dec4977f4dd09529e4213eceb4d613d0a87a7b1b5044b3616fd` 保持 immutable historical evidence，分别分类为 `REJECTED_NON_INTEGRABLE_PLAN_AUTHORITY` 与 `REJECTED_HISTORICAL_PLAN_CONTRACT`，modified: `false`。新的 clean B011 contract 为 `fa7d260c4f77f164b6ae7f1582eb87b20a1f2b4cf59b37a799027e17bfe876a3 = ACTIVE_FROZEN_CONTRACT`，仅依赖 `M1-B001`，没有额外 pre-implementation governance prerequisite。Lineage-clean product identity 继续固定为 `b82d52571287d9d0ebcfa589fd575125ac578476`，tree 为 `b9be959f22a8b15080a1035fba0fb46c256edf62`；product accepted 为 `true`，本 transition 的 product changes 为 `NONE`。

Platform independent acceptance 为 `PASS`。历史 verdict `PASS_M1_B011_PLATFORM_ACCEPTED` 的 evidence root `112dfdd0d93ba936fba31108006c166ae21ea86a2e090b9dbac611fcca93e9a9` 仅为 `HISTORICAL_ONLY`：original payload unavailable、not reverified，不能作为 current authority。当前 durable authority 是 `M1-B011-PLATFORM-ACCEPTANCE-EVIDENCE-REISSUE-001` / `PASS_M1_B011_PLATFORM_ACCEPTANCE_EVIDENCE_REISSUED`，verified root `865a244e234dadb0f919fd42645b44ee74129c1e01b81d4736615bbe0659ea40`，archive SHA-256 `21ecfa011cee94197670e12f13c94201276585362e66fbed1c96a7c485f5d5dd`，Platform ACCEPT binding `aee21ae041faad20ba96379d7d2c9ecfae0cef372d40e3265cd08f03e7a1f081`。

Fresh/current Creator binary identity 为 `5b54b2d5c9db302baf965da0e5061b427652e2a7c58a2f647bf5d8a98e36bb55`；historical identity 为 `6158fe53646fc3d53f87f48ef675d0dc727806e24a0ee195d0992dd28681af87`；binary reproducibility 为 `DIFFERENT`，但 deterministic product output 继续为未变化的 `35de290d80f307b19548595c6e26b160b34cda012e1dd0343aa88c522a0810cc`。

历史 cross-repository closure 为 `PASS / SUPERSEDED_FOR_CURRENT_INTEGRATION`。历史 authority 是 `M1-B011-CROSS-REPOSITORY-EXACT-PAIR-ACCEPT-001` / `PASS_M1_B011_CROSS_REPOSITORY_EXACT_PAIR_ACCEPTED`，root `b154784d312babb7dc83b76f03f88c7ed879c5895d0dee8ea15b58f5573ea88d`，archive SHA-256 `3365c6adc84ebb490beb1a5fe9a56ecf1487436bc7f8ac6d2acad82f44dc0743`，Rules cross-ACCEPT binding `09ced89ea54e068d62aee2e2da288d1c5a8d22fb49d7330dd36f97080e522edf`。其 exact tuple 绑定 Rules adapter `455cd5d66c683565a4aad7ef8d6523421d37db71` / `d74aae0baf791566a770ef1aaed2da7562f681d5`、Rules rebind `00e015270b99d819ffefcb0e36442f0b4e7874ef` / `c63a63b051f6cecc6eec24b786d2b2a0f2b216d6`、Platform product `b82d52571287d9d0ebcfa589fd575125ac578476` / `b9be959f22a8b15080a1035fba0fb46c256edf62` 与 Platform VERIFYING predecessor `412111c05c087ed066a297304bed728870f03fc8` / `a77015feee62564ec8ec9744391a3b5de83e4217`。该历史 PASS 不接受新的 Rules / Platform governance tuple。

当前 preintegration Rules binding 为 `6caf57cdc1127e84546459766949e0da664bb2f9` / tree `ec6cb5e7fdc0c93f09862fcf032dbb61393293a1`，candidate acceptance task 为 `M1-B011-RULES-REBIND-INDEPENDENT-ACCEPT`，root `27d8cf103dd0ce6740330df74c813c9872133a484243d06c264ac5903f1348d5`，archive SHA-256 `b4c55e9886651945a693239799f64f1474cf13a15b8a745c7acb6baddb4b45df`。Preintegration prerequisite `M1-B011-RULES-REMOTE-INTEGRATION-INDEPENDENT-ACCEPT` 的外部证据 root 为 `5ca14d526359972dec8b345e3c537e5d9d5b751695277302cc4d7bd935e0d9d3`，archive SHA-256 为 `a0ed5564e60ea072ed0210c87da0a1fc0545ab158292d157ea2e2e06b83568da`；远端写入结果只记录于该外部审计包。

Recovery PLAN-003（root `2fc1d3ba75e457a329f46a4ebf9292801424598b6480dd81048fa460bdf8f995`；独立验收 root `008abc6ae98d9d0820c4bd5eb94f21fbcf3f4056994d85f294f762d7e297f47f`）要求先验收本次治理绑定，再执行 `M1-B011-NEW-CROSS-PAIR-ACCEPT` 和 `M1-B011-COMPLETION-REACCEPT`。新的 cross-pair 和 completion reaccept 均为 `REQUIRED_NOT_RUN`；不复用历史配对 PASS。

M1-B011 lifecycle state: `COMPLETED`。

Identity 分层保持明确：

- Product identity: `b82d52571287d9d0ebcfa589fd575125ac578476` / `b9be959f22a8b15080a1035fba0fb46c256edf62`。
- VERIFYING governance identity: `412111c05c087ed066a297304bed728870f03fc8` / `a77015feee62564ec8ec9744391a3b5de83e4217`。
- Completion transition identity: commit `e99f31ad842ebfda4c51808f50c46be3f2a97cfc` / tree `a15ded87265b2ae30da20baa9810cca056e30798`；parent `412111c05c087ed066a297304bed728870f03fc8`。
- Accepted post-transition identity projection repair: `967aa4f628a521265e82eeb746bad629f23d5358` / `cac9af0744e0acf8a08cc937990a242ceb12b96c`。
- Current preintegration governance rebind candidate: product and lifecycle identities remain unchanged；this new candidate's exact commit/tree are bound externally by its signed Git object and the next independent acceptance, avoiding a self-reference。

历史 completion transition independent acceptance `M1-B011-COMPLETION-INDEPENDENT-ACCEPT-001` 的结果为 `FAIL_M1_B011_COMPLETION_TRANSITION`，failure 为 `FAIL_COMPLETION_TARGET_BINDING`，当时 disposition 为 `REPAIR_REQUIRED`。随后 `M1-B011-COMPLETION-INDEPENDENT-ACCEPT-RETRY-001` 在 governance head `967aa4f628a521265e82eeb746bad629f23d5358` 通过，结果为 `PASS_M1_B011_COMPLETION_TRANSITION_ACCEPTED_AFTER_PROJECTION_REPAIR`，root 为 `9091af0acf5a48177ca7fea8708aa87a7c07a77ac0f861ee46d25f30ad865b46`。这是旧配对的已接受历史事实；新的 Rules binding 仍要求 completion reaccept。

读取图缺失的规范章节以 exact accepted semantic authority `cde53640e143b7adb9e36d7d8bc4fedb6f0ed401` 补齐：`PACKAGE_MODEL.md` 为 mode/blob `100644 3322861b08979c328d068b1c6040494238cd8186`，`M1_SCOPE_AND_EXIT_GATE.md` 为 `100644 dee317bded7541582f4b168a52bd85213a462e3b`。Creator boundary governance repair authority `54f5dea3438d32a04a14f8888347aa3dfc222ee9` 保持 accepted，manifest-v2 Creator boundary 保持 `GENERIC JSON`。Clean lineage base 为 `main@36e0009e779564ad44799f54f9ccc1c74ac412a8`；B002 ancestry 为 `ABSENT`；old rejected PLAN ancestry 为 `ABSENT`；old B010 product ancestry 为 `ABSENT`。

Rules 状态仍为 `M2-B001 = ACCEPTED`、effective `R2-INT-010 = RESOLVED_ADAPTER`、`M2-B002 = NOT_STARTED`、`M2-B002_READY = NO`；不宣称 Rules aggregate M2 PASS、formal M2-B002 PASS 或 formal M2-B009 PASS。

原 completion transition `e99f31ad842ebfda4c51808f50c46be3f2a97cfc` 继续保持 `M1-B011 = COMPLETED`；本 preintegration governance rebind 只更新当前绑定和验收依赖，不产生第二次 lifecycle transition。治理修复范围由 `GOV-M1-B011-PREINTEGRATION-BINDING` 限定。新的治理候选尚待独立验收；Platform `integration_authorized = false`、`push_authorized = false`、`merge_authorized = false`。下一 gate 是 `M1-B011-PLATFORM-PREINTEGRATION-GOVERNANCE-REBIND-INDEPENDENT-ACCEPT`，之后依次为新 cross-pair、completion reaccept、transport PLAN 及其独立验收、staging flow。

状态摘要不覆盖权威规范。
