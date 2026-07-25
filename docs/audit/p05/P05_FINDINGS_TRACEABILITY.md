# P05 主责问题修复追溯

记录日期：2026-07-26（Australia/Brisbane）

```text
REVALIDATION_BASE_HEAD = 63e708afe3560a419fe66afbb6f55dc99e79e175
PRIMARY_AUD_COUNT = 9
CLOSED_PASS = 9
BLOCKED = 0
P06_ENTRY = ALLOWED
```

本表只关闭外部 P05 提示词明确列出的九个 AUD，不从已丢失的历史扫描临时文件猜测额外实例。
每一行同时列出实现、正向/负向测试与当前状态。

| AUD | 实现证据 | 测试与负向证据 | 状态 |
| --- | --- | --- | --- |
| `AUD-011` | `trpg-shared-kernel/src/shared_kernel.rs` 的 `VisibilityLabel`、`Visibility`、`PrincipalScope`；支持 player/group/spectator/system 组合 | `derived_visibility_matrix` 全处理者×来源×目标受众矩阵；shared-kernel 与 visibility leakage 回归证明严格度不降级 | `CLOSED_PASS` |
| `AUD-023` | 私密标签携带非空 `EntityId`；`Visibility::try_from_parts` 对缺失 subject fail closed，反序列化走相同校验 | shared-kernel 构造/serde 负例拒绝无目标 `PrivateToPlayer`、`PrivateToGroup`、`InvestigatorPrivate` | `CLOSED_PASS` |
| `AUD-024` | `trpg-security-governance/src/derived_visibility.rs` 统一按来源与目标受众求交；`agent_runtime.rs`、RAG/context assembler 使用同一决策 | `derived_visibility_matrix` `4/4`；domain visibility leakage、agent context、RAG、replay/export 回归随全 workspace 通过 | `CLOSED_PASS` |
| `AUD-027` | `trpg-shared-kernel/src/error_model.rs` 分离公开 wire error 与内部 cause，保留 operation/resource/correlation/trace；Debug/响应脱敏 | `error_model_contract_tests`、`wire_error_contract_tests`、`error_code_contract`；负例证明内部根因不进入公开响应 | `CLOSED_PASS` |
| `AUD-037` | `trpg-domain-core/src/visibility_fact_provenance.rs` 和 `decision_record_model.rs` 验证 source/provenance/正式事件/commit 一致性 | `fact_provenance` 必需范围 `6/6`；AgentProposal、未提交事件、不存在事件与矛盾 provenance 均拒绝 | `CLOSED_PASS` |
| `AUD-056` | `trpg-security-governance/src/secret.rs` 的 `SecretReference`/zeroizing secret；provider 只持引用；容器 versioned secret 私有 staging | `secret_boundary`、`provider_secret_reference`、production secret v1/v2 rotation；Debug 只含 `[redacted]` | `CLOSED_PASS` |
| `AUD-061` | `trpg-security-governance/src/security_privacy.rs` 与删除 migrations 实现租约状态机、证据绑定、重试终态和多 surface verifier | normalized `data_deletion_e2e` `8/8`；覆盖 DB/RAG/Object/Cache/Queue/Export/Backup、缺失 surface、错误保留、不可检索与恢复耗尽 | `CLOSED_PASS` |
| `AUD-062` | `trpg-data-eventing/src/event_store_sqlx_outbox_projection.rs` 的 AEAD `PayloadCipher`；encrypted columns/checks；PostgreSQL TLS/SCRAM 与角色最小权限 | `field_encryption`、TLS PostgreSQL integration、PG16/18 schema assertion、production TLS smoke；明文/错误 CA/非 owner 权限负例均拒绝 | `CLOSED_PASS` |
| `AUD-064` | `trpg-security-governance/src/cloud_egress.rs` 绑定 persisted consent、notice、route snapshot、secret refs、最小上下文与审计记录 | `cloud_egress_policy`、`cloud_egress_e2e`、OpenFGA/OPA 回归；缺同意、同意漂移、受限 visibility、route 变化与 provider unavailable 均 fail closed | `CLOSED_PASS` |

## 全链路回归映射

| 链路 | 当前证明 |
| --- | --- |
| Replay / Projection | P04 精确 Projection test 与完整 workspace |
| RAG / Summary | agent-runtime 与 data-eventing RAG tests 随完整 workspace 通过 |
| Export / 玩家可见性 | domain/testing visibility leakage 与 platform privacy tests 随完整 workspace 通过 |
| Tool Result / Agent Context | extension SDK tool contract、agent-runtime context tests 随完整 workspace 通过 |
| Secret / Log / Error | secret boundary、provider reference、wire error tests 与 production runtime |
| Cloud Egress / Policy | cloud policy/E2E、真实 OpenFGA/OPA、OPA 16/16 |

## Normalized owner 说明

当前权威 map 将 privacy/deletion 指定给 `trpg-security-governance`，payload encryption 指定给
`trpg-data-eventing`。因此外部提示词中的旧 `trpg-privacy` package 名只作为输入 provenance，
不创建为当前 crate/output；其真实验收目标是
`trpg-security-governance/tests/data_deletion_e2e.rs`。该替换遵守仓库根权威顺序，不是跳过测试。

历史 `P05-D015`、`P05-D025` 的原始标题/PoC 工件仍不可恢复；它们不属于 P05 提示词列出的九个
AUD，也未被本文猜测为关闭。此 provenance 缺失与当前九项控制验收相互独立，不构成 P06
批次前置。
