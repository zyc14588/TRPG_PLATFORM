---
document_id: CODEX-PROJECT-SNAPSHOT
schema_version: 1
document_kind: state-summary
authority: state-summary
status: ACTIVE
source_commit: "28ddda38e0a426be314ae8af2298672a61e972f9"
---

# 项目摘要

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
