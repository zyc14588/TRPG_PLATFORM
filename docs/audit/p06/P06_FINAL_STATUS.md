BATCH_STATUS: COMPLETE
BATCH_ID: P06

# P06 最终验收状态

记录日期：2026-07-26（Australia/Brisbane）

```text
BASE_HEAD = b2793988c5e2e021d556635d19e8d110a99ece8a
BRANCH = master
CURRENT_WORKTREE = UNCOMMITTED_INTENDED_P06_CHANGE
AUD_007 = CLOSED_PASS
AUD_041 = CLOSED_PASS
P02_P04_P05_PREREQUISITES = SATISFIED
P06_REQUIRED_COMMANDS = PASS
FULL_WORKSPACE_ALL_FEATURES = PASS
P06_SCHEMA_ASSERTION = PASS
THIRD_PARTY_PATCH_SCAN = PASS_0_NEW_FINDINGS
DEPENDENCY_ADVISORY_SCAN = FAIL_3_DISCLOSED_PREEXISTING_TRANSITIVE
EXTERNAL_CODERABBIT = NOT_RUN_NO_SOURCE_UPLOAD_AUTHORIZATION
HOSTED_CI_CURRENT_PATCH = NOT_RUN
COMMIT_BOUND_RELEASE_EVIDENCE = NOT_GENERATED
PRODUCT_RELEASE_ATTESTATION = NOT_CLAIMED
P07_P06_PREREQUISITE = SATISFIED
P07_IMPLEMENTATION = NOT_STARTED
```

P06 已交付 Campaign、Room、Session、Scene、Scenario、Character、
CharacterSheetVersion、Fork 和 Reconsideration 核心聚合，九张规定业务表、
SQLx Repository、COC7 角色/Scenario 校验、持久 Session/Scene 状态机，以及生产
`api-server` crate 中的 Campaign/Invite/Character command port。P06 没有创建
P07 的 HTTP action route、骰子、线索、SAN 或 Realtime 纵向链。

## 本轮反伪修复

审查时真实复现了旧实现的绕过：`trpg_api_service` 能复用一个合法 Character
事件序号，为另一个 row id 插入伪造角色。旧证据把原测试的绿色结果误写成了投影
完整性通过。当前修复：

- canonical Event integrity v3 的 HMAC 同时绑定允许写入的
  `(relation, row_id, capability_hash)` projection target；受信 Repository 从 commit id
  与 Event integrity secret 派生 commit-scoped projection capability，并在 projection transaction
  内以 transaction-local setting 提交。数据库 trigger 同时要求实际 relation/row 精确匹配，
  且 capability 的 SHA-256 与认证 target 相符。
- `trpg_api_service` 对九张 P06 业务投影只保留 `SELECT/INSERT/UPDATE`，显式撤销
  `DELETE`；真实 `SET LOCAL ROLE trpg_api_service` 的不同 row 复用事件伪造、已知精确
  target 但缺少 secret capability 的伪造内容写入，以及删除探针均被拒绝。
- `AuthorizedCoreApiContext` 字段私有且不实现 `Deserialize`，只能由 Identity 签发的
  requester/workflow context、锁定且 `FORK_ONLY` 的 Authority Contract 和精确 policy
  audit 构造；Authority binding 现在也绑定 mode。
- Invite token 由服务端 secret、稳定命令输入和 idempotency key 确定性派生，原始 token
  不持久化且 `Debug` 输出脱敏；Invite issue/accept、Character submit/approve、
  Session/Scene transition 和 Reconsideration 的精确重试均返回原结果而不追加事件。
- `api-server::core_domain::RepositoryCampaignCharacterPort` 是生产源码中的真实
  PostgreSQL/COC7 适配器，并由生产适配器集成测试直接实例化；不再用仅存在于测试文件的
  adapter 冒充生产接线。

## 事务语义（纠正旧证据）

本批次不再宣称 Event Store 与全部业务 Projection 位于同一个 SQL transaction：

- canonical transaction 原子写入 Event、Formal Commit、tamper-evident Audit 与 Outbox；
- P06 read-model projection 在随后的独立 transaction 中写入，并由已认证 event target、
  commit-scoped secret projection capability、workflow/audit、Visibility 与 Fact Provenance
  约束；
- 注入 projection failure 时，canonical event/outbox 保持唯一，Campaign/Room/Authority/
  Membership 不出现部分业务行，API 返回失败；移除故障后用相同 idempotency key 重试，
  不追加第二个 event，并完成 projection。

这符合 Event Store 为正史、Projection 可重建的顶层设计；旧文档中的“全部同一事务”断言
已撤回。

## 验收矩阵

| 验收项 | 当前证明 | 状态 |
| --- | --- | --- |
| 空库 Migration、表、约束和索引 | `migration_upgrade` 覆盖空库、历史升级、重复执行和 checksum drift；`assert-schema.sql` 输出 `P06_SCHEMA_ASSERTION_OK` | PASS |
| Campaign 与唯一锁定 Authority | 成功响应前完成锁定 Authority、owner Membership、Campaign/Room projection；故障注入证明无部分 projection，精确重试恢复 | PASS |
| Invite / Membership | 过期、错误 subject/token 被拒；原始 token 不持久化；issue/accept 精确重试不追加事件 | PASS |
| Character 生命周期 | COC7 schema、owner/keeper 权限、Draft→Submitted→Approved、初始版本物理锁定和精确重试在真实 API/Repository/PostgreSQL 链通过 | PASS |
| Session/Scene 状态机 | 并发 start 仅一个成功；非法 resume/switch 不产生事件；重连后从正史恢复；转换精确重试通过 | PASS |
| Tutorial Scenario | 真实 YAML parser、核心线索路径校验、稳定 canonical JSON/hash round-trip | PASS |
| Fork / Reconsideration | snapshot hash、不可变 lineage、版本化复议链和重试约束 | PASS |
| Authority/Membership/Visibility/Event 回归 | Identity 签发 context、真实 OpenFGA/OPA、Visibility matrix、canonical commit；不同 row 复用事件、精确 target 缺 capability 的伪造内容、DELETE 均被真实 API 数据库角色拒绝；完整 workspace 通过 | PASS |
| 生产接线边界 | 生产 port 的 Campaign→Invite→Membership→Character 真实数据库测试；未宣称 HTTP E2E | PASS |
| 第三方补丁检查 | Semgrep 1.171.0，HEAD differential，28 个 P06 文件、90 条适用规则、0 个新增 finding | PASS |

## 变更范围审计

| 文件面 | P06 必要性 |
| --- | --- |
| domain-core、ruleset-coc7、runtime | 规定的核心聚合、COC7 角色/Scenario 校验和 Session/Scene 状态机 |
| data-eventing、migration | SQLx Repository、九张业务表、canonical event target/capability、投影恢复与真实数据库回归 |
| trpg-api、api-server | typed 授权入口、基础 Campaign/Invite/Character command port 和生产 PostgreSQL adapter |
| shared-kernel、Cargo manifests/lock | Authority mode binding、必要的 crate 依赖与测试依赖；未引入 P07 产品功能 |
| CI/schema/test-support、安全删除测试适配 | 将新增 migration 纳入固定摘要服务、PG16/PG18 schema/升级和完整回归；未弱化既有 P05 断言 |
| Scenario fixture、audit、generated manifests | 结构化 Tutorial 输入、可追溯证据和完整 patch snapshot |

变更中不存在 P07 的 player action、DiceRoll、Clue、SAN、Realtime 或 HTTP action route，
也不存在 P08 及后续批次的预建实现。

P06 migration SHA-384：
`47774b27008ca0ed39188582d97edc21beb3f31ad739da688e0702854e25504e26169ab864624e584c61777f6691f424`。

## P07 准入与剩余边界

P04、P05、P06 的批次完成证据存在，P06 的代码、真实依赖测试和第三方补丁检查满足
P07 的开发准入前置；P07 仍须在开始时独立复核其全部门禁。`cargo-audit` 对既有
`rust-s3 -> aws-creds -> quick-xml 0.38.4` 报告两个 High advisory，并对当前 Linux
依赖图不可达、仅存在于 lockfile 的 `rsa 0.9.7` 报告一个 Medium advisory。它们没有被
写成 PASS，也没有在 P06 范围内通过替换对象存储客户端“顺手修复”；在发布准入前仍须由
P05/依赖治理批次升级、替换或正式接受风险。

当前 patch 未提交，Hosted CI、不可变候选上的外部 CodeRabbit 和 commit-bound release
evidence 均未执行，因此不能宣称已合并、已发布或发布安全门禁全绿。回滚只能撤销普通代码
变更或用前向 migration 修正；不得编辑 SQLx ledger、删除 Event Store 正史或放宽
Authority/Visibility/policy gate。
