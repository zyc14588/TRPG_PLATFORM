BATCH_STATUS: COMPLETE
BATCH_ID: P05

# P05 当前最终验收状态

记录日期：2026-07-26（Australia/Brisbane）

```text
REVALIDATION_BASE_HEAD = 63e708afe3560a419fe66afbb6f55dc99e79e175
WORKTREE = UNCOMMITTED_INTENDED_REPAIR
LOCAL_REQUIRED_GATES = PASS
REAL_DEPENDENCY_WORKSPACE = PASS
PRODUCTION_TLS_MTLS_SECRET_ROTATION = PASS
AUD_011_023_024_027_037_056_061_062_064 = CLOSED_PASS
P02_P03_P04_PREREQUISITES = SATISFIED
P06_ENTRY = ALLOWED
P06_IMPLEMENTATION = NOT_STARTED
HOSTED_CI_CURRENT_PATCH = NOT_RUN
COMMIT_BOUND_RELEASE_EVIDENCE = NOT_GENERATED
PRODUCT_RELEASE_ATTESTATION = NOT_CLAIMED
```

本状态以当前工作树的真实复验为准，取代 2026-07-25 文档中
`PRODUCTION_COMPOSE_RUNTIME = NOT_RUN`、`P04_STRICT_PREREQUISITE = NOT_PROVEN_COMPLETE` 和
`P06_ENTRY = DENIED` 的旧结论。生产运行态、P04 精确目标及 P05 全部验收面现在已经实际通过。

未提交工作树、Hosted CI 和 commit-bound 发布证据仍如实标为未完成；这些是发布/合并门禁，
不是 P05 提示词或 P06 强制前置所列的批次完成条件，因而没有被用来伪造或否定本地批次验收。

## 修复结果

- 修复 PostgreSQL TLS/SCRAM、CA 路径标准化和服务最小权限：API/Worker 不再以 owner 身份运行
  migration/recovery，migration runner 独占 schema 变更权。
- 修复容器密钥边界：只复制版本化普通 secret 文件到 non-root 私有目录，权限收敛为 `0400`；
  Realtime 对十六进制 secret 做严格 32-byte 转换。
- 修复 NATS TLS-first、相对 include、最低 TLS 版本和服务端兼容配置；客户端不再向诊断路径泄露
  URL credential。
- 修复 integration bootstrap、数据库版本签名、TLS hostname、OpenFGA 命令和生产 smoke 的
  Buildx/tag 竞争，使 PostgreSQL 16/18、NATS、Redis、MinIO、OpenFGA、OPA 及完整生产服务图都
  可重复执行。
- 删除工作流继续由 normalized owner `trpg-security-governance` 承担，覆盖 PostgreSQL、RAG、
  Object、Cache、Queue、Export、Backup 规则及完成后不可检索验证；没有重新引入非规范
  `trpg-privacy` crate。
- 保持统一 Visibility、Fact Provenance、Secret、Cloud Egress 和错误脱敏边界；全 workspace
  回归证明 Replay、RAG、Summary、Export、Tool Result 未退化。

## 变更范围审计

| 范围 | 文件类别 | P05 必要性 |
| --- | --- | --- |
| 服务启动边界 | API、Agent Worker、Realtime | 移除 owner migration/recovery，严格解析 secret；是 TLS/Secret/最小权限的接口适配 |
| 数据/身份边界 | data-eventing、identity、security-governance | NATS TLS-first、verify-full PostgreSQL、删除 surface readiness |
| 生产安全配置 | entrypoint、NATS、PostgreSQL HBA、CI Compose | Secret 私有 staging、TLS/SCRAM/mTLS 与隔离 runtime probe |
| 验证工具 | schema assertion、integration/bootstrap、production smoke、Compose verifier、S09 | 让 PG16/18 和完整生产图真实可执行并拒绝假绿 |
| 证据 | P04/P05 audit 与三份 generated manifest | 清除旧阻断状态并绑定当前 staged 内容 |

这些修改均服务于 P05 的 Secret、删除、字段/传输安全和前置回归，没有新增其他产品功能。
`git diff --cached --name-only` 不含 Campaign、Session、Scenario、Character、P06 migration 或
P06 API；跨预期目录的修改仅为上述编译、启动、权限和真实测试适配。

## P05 验收

| 验收项 | 当前证据 | 状态 |
| --- | --- | --- |
| 全 Visibility 矩阵 | `derived_visibility_matrix`：`4/4`；全 workspace 关联矩阵与泄漏回归 | PASS |
| AgentProposal 不可晋升 Confirmed | `fact_provenance`：必需范围 `6/6`；非正式来源和无正式事件均拒绝 | PASS |
| 玩家导出无 Keeper/System 内容 | Visibility leakage/export/agent-context 回归进入全 workspace | PASS |
| 删除任务有完成验证 | normalized `data_deletion_e2e`：`8/8`，真实多 surface | PASS |
| 跨云边界明确同意与审计 | cloud egress policy/E2E、OpenFGA/OPA 与审计链回归 | PASS |

九个主责 AUD 的代码位置、负向证据和逐项状态见 `P05_FINDINGS_TRACEABILITY.md`；运行命令、真实
失败和反伪通过处置见 `P05_TEST_RESULTS.md`。

## 权威命名冲突处置

外部 P05 提示词列出的字面命令
`cargo test -p trpg-privacy --test data_deletion_e2e` 对当前 workspace 不存在。仓库根
`AGENTS.md` 要求先应用 normalized ownership map，该 map 将 privacy/deletion 明确归入
`trpg-security-governance`，并禁止旧/非规范 output 重新成为工程命名。因此没有创建兼容空壳、
别名 crate 或 marker test。该字面命令已实际执行并以 package 不存在退出 `101`，随后执行真实
等价目标：

```text
cargo test --locked -p trpg-security-governance \
  --test data_deletion_e2e --all-features -- --test-threads=1
```

结果为 `8 passed / 0 failed / 0 ignored`。字面旧命令没有被写成 PASS。

## P06 准入结论

P02、P04、P05 的完成证据均已复核；P05 自身所需 P03 回归也通过。故 P06 的进入前置已经满足。
本轮严格停在 P05，没有创建或修改任何 P06 Campaign/Session/Scenario/Character 聚合、表、API
或状态机。

风险仍是修复尚未形成不可变提交，回滚应通过撤销本轮普通代码/配置变更并保留 Event Store 与
前向 migration 历史完成；不得编辑 SQLx ledger、删除正史或放宽 TLS/Visibility/policy gate。
