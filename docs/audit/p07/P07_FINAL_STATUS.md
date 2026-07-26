BATCH_STATUS: COMPLETE
BATCH_ID: P07

# P07 最终验收状态

记录日期：2026-07-27（Australia/Brisbane）

```text
BASE_HEAD = b2793988c5e2e021d556635d19e8d110a99ece8a
P07_IMPLEMENTATION_COMMIT = 6657d90a47110e3df4ce4f0e53f1e78e2b661a4c
P07_REVIEW_REMEDIATION_COMMIT = eb02b24d8d2a422dc70d0d2e5052b3f0267431c1
PUBLICATION_BRANCH = agent/p06-p07-core-player-action
PUBLICATION_PR = 8
DOCUMENT_SCOPE = LOCAL_ACCEPTANCE_CI_REPAIR_AND_REVIEW_REMEDIATION
P04_P05_P06_PREREQUISITES = SATISFIED
AUD_006 = CLOSED_PASS
AUD_008 = CLOSED_PASS
AUD_013 = CLOSED_PASS
AUD_030 = CLOSED_PASS
AUD_042 = CLOSED_PASS
P07_REQUIRED_COMMANDS = PASS
REAL_HTTP_POSTGRES_POLICY_JETSTREAM = PASS
P07_SCHEMA_ASSERTION = PASS
THIRD_PARTY_LOCAL_DIFF_SCAN = PASS_0_FINDINGS
CODERABBIT_EXTERNAL_REVIEW = NOT_RUN_SIGNED_OUT_NO_SOURCE_UPLOAD
DEPENDENCY_ADVISORY_SCAN = FAIL_3_DISCLOSED_PREEXISTING
HOSTED_CI_INITIAL_IMPLEMENTATION_COMMIT = FAIL_PG_DUMP_16_SERVER_18
CI_REPAIR_LOCAL_BACKUP_RESTORE = PASS_POSTGRESQL_18_4
CI_REPAIR_THIRD_PARTY_SCAN = PASS_0_FINDINGS
HOSTED_CI_CI_REPAIR_COMMIT = PASS_ALL_REQUIRED_CHECKS
GITHUB_CODEX_REVIEW = 5_P1_FIXED_REPLIED_RESOLVED
FINAL_FULL_WORKSPACE_TEST = PASS
FINAL_THIRD_PARTY_SCAN = PASS_0_NEW_FINDINGS_4_PREEXISTING_INFO
HOSTED_CI_FINAL_HEAD_POLICY = MUST_PASS_BEFORE_MERGE
PRODUCT_RELEASE_ATTESTATION = NOT_CLAIMED
P08_P07_PREREQUISITE = SATISFIED
P08_IMPLEMENTATION = NOT_STARTED
```

P07 已完成最小 HUMAN_KP 玩家行动真实纵向链：TCP HTTP 请求经过 bearer authentication、
Campaign membership、锁定 Authority Contract、OpenFGA/OPA 与 tamper-evident audit，
进入 Runtime workflow；只有 Authority owner 确认后才执行服务器 CSPRNG 和 COC7
Rules；Decision、DiceRoll、Clue/SAN、Event、Audit、Outbox 与受保护 Projection 在同一
PostgreSQL transaction 中提交；API 返回的 Realtime Delta 标识随后由生产 Outbox
publisher 在真实 NATS JetStream 上投递并取得 ACK。

## 验收矩阵

| 验收项 | 当前证明 | 状态 |
| --- | --- | --- |
| 真实 HUMAN_KP HTTP 纵向链 | TCP HTTP + Identity + Authority + OpenFGA/OPA + Runtime + COC7 + primary/witness PostgreSQL + JetStream | PASS |
| 客户端/模型不可提供正式骰点 | DTO `deny_unknown_fields`；带 `roll` 请求 HTTP `400` 且 Event/Dice 不增加；正式 Dice 为 `SERVER_OS_CSPRNG` | PASS |
| Tool 真实执行后才提交 | Agent/Runtime 默认拒绝 executor；成功顺序为 authorize → canonical retry lookup/version preflight → execute/validate（仅未命中）→ ToolExecutionSucceeded → DecisionCommitted | PASS |
| 调查与核心线索 | 成功/失败由规则计算；核心线索失败 `REVEALED_WITH_COST`，不会因坏骰阻断模组 | PASS |
| SAN 固定日初阈值 | 持久 `day_start_sanity`；分次/合并损失属性不变量与真实 DB transaction 通过 | PASS |
| HUMAN_KP 权威边界 | AI/非 owner 只能保留 Draft 或被拒；无确认无 RNG、Dice、Decision 或正式状态 | PASS |
| 原子性与幂等 | fault injection 全回滚；精确 retry 返回原序号，不重掷、不追加重复状态 | PASS |
| Visibility/Provenance/Trace | API、Event、Projection、Outbox、NATS envelope 保持标签、subject、来源及 correlation/trace | PASS |
| Migration 与最小权限 | 五张 P07 表、guarded projection functions/triggers；canonical/API role 无直接表 DML 绕过；schema assertion 通过 | PASS |
| 邀请可信时间与原子 membership | 客户端时钟字段拒绝；服务端时钟校验；CampaignInviteAccepted 与 membership 冲突在同一 canonical transaction 回滚 | PASS |
| 第三方检查 | GitHub Codex 五个 P1 均修复并 resolved；最终 Semgrep 1.171.0 扫描 21/21 目标、13 条适用规则、0 新 finding；4 个既存 INFO 完整披露 | PASS |

五个主责 finding 的逐项代码与负向证据见
`docs/audit/p07/P07_FINDINGS_TRACEABILITY.md`；命令、失败基线和退出结果见
`docs/audit/p07/P07_TEST_RESULTS.md`；第三方工具边界见
`docs/audit/p07/P07_THIRD_PARTY_REVIEW.md`。

## 变更范围审计

`git diff --name-only`、`git status --porcelain=v1` 与 `git diff --check` 已实际执行。
P07 开始前即存在且必须保留的 P06 patch 与 P07 实现随后共同发布为
`6657d90a47110e3df4ce4f0e53f1e78e2b661a4c`；它们不是 P08 功能，也未被撤销或覆盖。
P07 新增/修改面限定为：

- Player Action API、生产 adapter/route、Runtime/Agent Tool 执行语义；
- COC7 服务端骰、调查核心线索和 SAN 日初阈值；
- canonical transaction、P07 state projection、migration 与最小权限；
- 真实 HTTP/PostgreSQL/OpenFGA/OPA/JetStream 测试及必要 CI 环境接线；
- 托管 runner 在 PostgreSQL 客户端与服务端主版本不同时使用同一摘要固定的 18.x
  容器客户端，且只挂载本次作业专用临时目录；
- formal authorization 与 canonical retry lookup 在任何非幂等 executor 前完成；
- 邀请时间改用可信服务端 clock，邀请接受 Event 与 membership 成为单一原子事务；
- legacy 零 projection request hash 保持升级后 exact retry 兼容；
- P07 audit evidence、Cargo lock/manifests 和受影响旧测试的语义修正。

没有新增 Combat/Chase/Fork/Reconsideration/Ending/Growth 的 P08 实现。

## 回滚与运行边界

`TRPG_PLAYER_ACTION_WRITES_ENABLED=0` 可停用新玩家行动写入口：API 仍启动，但 submit/confirm
fail-closed 为 workflow unavailable；不删除或改写任何 Event Store 正史。未设置该开关
时保持当前启用行为；非法值会使服务启动失败，避免配置拼写导致静默状态漂移。已发布事件
继续保留 schema version，后续格式变化必须以前向 migration/upcaster 兼容。

实现提交首次 Hosted CI 揭示 runner 自带 `pg_dump 16` 与固定 PostgreSQL 18 服务不匹配；
该失败没有被改写为 PASS。后续 CI 修复已在本地用 PostgreSQL 18.4 完成真实 dump、
`pg_restore --list`、P02 独立恢复及 P04 Event Store 灾备重建，并经 Semgrep 精确差异
复扫，且该 CI repair commit 的五项托管检查全部通过。随后 GitHub Codex 的五个 P1 已
由 `eb02b24d8d2a422dc70d0d2e5052b3f0267431c1` 修复、逐条回复并 resolved；合并策略仍
要求包含最终证据与 manifests 的 PR head 托管 CI 全绿。RustSec 的三个既有 advisory
保持显式失败/披露；它们不被 Semgrep 的 0 个新增 finding 覆盖。回滚不得删除测试、弱化
policy/Visibility gate、编辑 SQLx ledger、删除正史或使用破坏性 down migration。

P07 migration SHA-384：

- `20260727000100_create_player_action_decision_state.sql`：
  `590d03cd0df1df8ff07a3fc5bd5c28b30027d526f662c0a31ec7c7eae0056c1b283c59855a482cb28bf6cdd7c25a3427`
- `20260727000200_harden_campaign_invite_acceptance.sql`：
  `8f796c71a4ee6c567ec54f479cd852b3095bb072ee16dd2947c0928355718787ad11c2207abdbf8450626b4414567312`

P07 进入条件已经满足。本批次严格停止于 P07；未实现、运行或验收 P08。
