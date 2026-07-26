# P06 主责问题修复追溯

记录日期：2026-07-26（Australia/Brisbane）
基线 HEAD：`b2793988c5e2e021d556635d19e8d110a99ece8a`

```text
PRIMARY_AUD_COUNT = 2
CLOSED_PASS = 2
BLOCKED = 0
```

| AUD | 实现证据 | 正向与负向证据 | 状态 |
| --- | --- | --- | --- |
| `AUD-007` | `domain_entities_value_objects.rs` 定义稳定 ID、聚合和版本事件；P06 migration 建立九张业务表、约束、索引和最小权限；`CoreDomainRepository` 覆盖 Campaign、Invite、Character、Scenario、Fork、Reconsideration；Event integrity v3 绑定 projection relation/row id/capability hash | `core_entities` `4/4`；`core_domain_schema_integration` 真实数据库 `1/1`；空库/升级 migration 和 schema assertion 通过；复用合法 event 写不同 Character row、已知精确 target 但缺少 secret capability 写伪造 Campaign 内容、API role DELETE 均被拒绝；projection 故障无部分业务行且 exact retry 不追加事件 | `CLOSED_PASS` |
| `AUD-041` | `SessionSceneStateMachine` 实现 Scheduled/Active/Paused/Ended 与 Scene Ready/Active/Closed；Repository 使用 expected version、idempotency、唯一 live-session/active-scene 索引和 event replay | `session_scene_state_machine` 真实 PostgreSQL `1/1`；并发 start 只有一个成功；非法转换不追加 event；进程重连恢复 active session/scene；pause/resume/end/switch 精确重试不重复事件 | `CLOSED_PASS` |

## 需求到实现映射

| P06 需求 | 当前实现与边界 |
| --- | --- |
| 核心聚合与稳定 ID | Campaign、Room、Session、Scene、Scenario、Character、CharacterSheetVersion、Fork、Reconsideration typed IDs 和 lifecycle |
| Schema/约束 | 九张规定业务表、唯一/外键/非空/版本约束、append-only/immutability trigger、PG16/PG18 catalog fingerprint |
| Campaign + Authority | canonical `CampaignCreated` 先进入 Event/FormalCommit/Audit/Outbox；随后同一 projection transaction 写锁定 `FORK_ONLY` Authority、owner Membership、Campaign 和 Room；失败返回错误并由 exact retry 重建，不虚构为与 Event Store 同一 SQL transaction |
| Invite / Membership | 服务端确定性 secret 派生 token、只存摘要、绑定 subject/role/expiry；有效 accept 写真实 P02 Membership；issue/accept 可幂等重试 |
| Character 版本 | Draft→Submitted→Approved；COC7 schema 校验；owner/keeper 权限；初始 sheet version 物理锁定；完整重试与伪造事件负例 |
| Scenario parser | 真实 YAML/JSON parser、结构验证、稳定 canonical JSON/SHA-256 和 `.scenario.yaml` fixture |
| Session/Scene | PostgreSQL 持久状态机、唯一 live session/active scene、非法转换拒绝和 event replay |
| Fork/Reconsideration | snapshot hash、父子 lineage、append-only request/review chain 和版本约束 |
| API | 私有、不可反序列化的 typed authorization context；真实 OpenFGA/OPA/audit API integration；生产 `api-server` repository port |

## 架构不变量

- Authority Contract 是 Campaign 级锁定契约，mode 被绑定，只能按 `FORK_ONLY` 规则改变。
- Event Store 是正史；Projection 是可重建读模型。正式 canonical transaction 原子包含
  Event、Formal Commit、Audit 和 Outbox，绝不把独立 projection transaction 伪称为同一事务。
- projection 写入必须匹配 HMAC v3 认证的 relation/row/capability-hash target、
  transaction-local commit-scoped secret capability、workflow permit、Visibility、
  Fact Provenance、event type 与 last event sequence。
- API 数据库角色无 P06 业务表 DELETE；无 event、复用 event 写其他 row，或知道精确
  target 但缺少 secret capability 写伪造内容均 fail closed。
- 正式写 actor 必须是 Identity 签发的 `workflow_engine` workload；客户端不能提交或
  反序列化 policy decision、role、Authority binding。
- User 继续引用 P02 `public.users`，没有复制 Identity 正史。
- P06 没有实现战斗、追逐、AI Provider、Realtime、Export 或 P07 玩家行动闭环。
