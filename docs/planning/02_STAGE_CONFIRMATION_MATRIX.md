# 02 — 施工阶段确认矩阵

## 1. 阶段确认规则

每阶段开始前，Codex 必须在输出中明确确认：

```text
阶段编号：SXX
已读取：AGENTS.md、codex-persistent-context、codex-prompt-boundary、阶段 START_PROMPT、相关 module prompt、相关 execution batch、相关 per-file prompts
计划修改：crate / migration / schema / API / tests / docs
不越界声明：primary/supplemental/documentation 语义保持一致
```

每阶段结束时，Codex 必须输出：

```text
阶段编号：SXX
变更文件清单
关联 Prompt ID / batch
测试命令与结果
schema / migration / event / API 变更
已知风险与 TODO
是否满足阶段验收门禁：YES/NO
```

## 2. 阶段矩阵

| 阶段 | 名称 | 主要输入分类 / Batch | 主要输出 | 阶段验收焦点 |
|---|---|---|---|---|
| S00 | 治理落位与 Codex 施工入口 | `00-index`, `90-traceability`, `99-appendix`<br>BATCH-001-00-index, BATCH-002-00-index, BATCH-046-90-traceability, BATCH-047-90-traceability, BATCH-048-90-traceability, BATCH-049-90-traceability, BATCH-050-90-traceability, BATCH-051-99-appendix, BATCH-052-99-appendix | `trpg-docs-governance` | 仓库根 AGENTS.md 存在并声明不可突破原则；docs/codex/00-index/codex-persistent-context.md 与 prompt-boundary 可被 Codex 定位 |
| S01 | Rust Workspace 与 shared kernel 基座 | `01-foundation`<br>BATCH-003-01-foundation, BATCH-004-01-foundation, BATCH-005-01-foundation, BATCH-006-01-foundation | `trpg-shared-kernel` | 所有公开类型使用领域专名，不出现 ModuleService/ModuleCommand 模板残留；CommandEnvelope 携带 idempotency_key、expected_version、actor、correlation_id、causation_id |
| S02 | Domain Core：Authority、Campaign、Decision 与事件模型 | `02-domain-core`<br>BATCH-007-02-domain-core, BATCH-008-02-domain-core, BATCH-009-02-domain-core, BATCH-010-02-domain-core, BATCH-011-02-domain-core | `trpg-domain-core` | Campaign 生命周期内 authority_mode 和 authority_owner 不可更改；正式状态只能由 Decision/Event 路径生成 |
| S03 | Data/Eventing：PostgreSQL、Event Store、Outbox、Projection、RAG Snapshot | `06-data-eventing`<br>BATCH-024-06-data-eventing, BATCH-025-06-data-eventing, BATCH-026-06-data-eventing, BATCH-027-06-data-eventing, BATCH-028-06-data-eventing | `trpg-data-eventing` | Event Store 是唯一正史；Projection/Cache/RAG 均可重建；所有正式写入在 transaction 内完成 event append 与 outbox |
| S04 | Security Governance：OpenFGA/OPA、权限、隐私、版权与审计 | `09-security-governance`<br>BATCH-035-09-security-governance, BATCH-036-09-security-governance, BATCH-037-09-security-governance | `trpg-security-governance` | Policy Gate 不能被 Agent、插件、handler、provider 绕过；安全暂停不直接改变游戏结果 |
| S05 | COC7 Ruleset：角色、骰子、检定、SAN、战斗、追逐、场景结构 | `05-ruleset-coc7`<br>BATCH-021-05-ruleset-coc7, BATCH-022-05-ruleset-coc7, BATCH-023-05-ruleset-coc7 | `trpg-ruleset-coc7` | 所有正式骰子由服务端生成；核心线索不因一次失败永久丢失 |
| S06 | Runtime Orchestration：Session、Workflow、Pending Decision、Decision Commit Pipeline | `03-runtime-orchestration`<br>BATCH-012-03-runtime-orchestration, BATCH-013-03-runtime-orchestration, BATCH-014-03-runtime-orchestration, BATCH-015-03-runtime-orchestration, BATCH-016-03-runtime-orchestration | `trpg-runtime` | 任何正式状态变更必须经过 Decision Commit Pipeline；HUMAN_KP 模式 AI 输出 requires_human_confirmation=true |
| S07 | Agent Runtime：Gateway、Tool Permission Gate、Provider、本地模型认证、Memory/RAG | `04-ai-agent-system`<br>BATCH-017-04-ai-agent-system, BATCH-018-04-ai-agent-system, BATCH-019-04-ai-agent-system, BATCH-020-04-ai-agent-system | `trpg-agent-runtime` | 所有 AI 能力经 Agent Gateway -> Orchestrator/Runtime -> Provider Adapter；表达 Agent 不能新增事实或调用状态变更工具 |
| S08 | API / Realtime：REST、OpenAPI、WebSocket、NATS Contract、服务二进制 | `07-api-realtime-contracts`<br>BATCH-029-07-api-realtime-contracts, BATCH-030-07-api-realtime-contracts | `trpg-api` | 前端不能直接调用模型服务；handler 必须传递 actor、visibility、provenance、correlation_id |
| S09 | Platform Infrastructure：Docker Compose、Object Storage、Observability、Admin Health | `08-platform-infrastructure`<br>BATCH-031-08-platform-infrastructure, BATCH-032-08-platform-infrastructure, BATCH-033-08-platform-infrastructure, BATCH-034-08-platform-infrastructure | `trpg-platform` | docker compose up -d 能启动 Web/API/Realtime/Agent/Postgres/pgvector/Redis/Object Storage/Reverse Proxy/Admin；生产环境拒绝占位 key 或暴露未鉴权本地模型 |
| S10 | Ops / Migration：备份、恢复、升级、回滚、Projection Rebuild | `11-ops-migration`<br>BATCH-042-11-ops-migration, BATCH-043-11-ops-migration | `trpg-ops` | 恢复后 Event Store hash 与备份前一致；Projection rebuild 不产生新正史事件 |
| S11 | Testing Quality：Tutorial/Golden Scenario、Contract、Leakage、Model Certification CI | `10-testing-quality`<br>BATCH-038-10-testing-quality, BATCH-039-10-testing-quality, BATCH-040-10-testing-quality, BATCH-041-10-testing-quality | `trpg-testing` | Golden Scenario Tests 全部通过；失败不能通过删除测试、弱化 policy gate 或关闭 visibility 检查解决 |
| S12 | Extension SDK 与分层 UI 边界 | `12-extension-sdk`<br>BATCH-044-12-extension-sdk, BATCH-045-12-extension-sdk | `trpg-extension-sdk` | 插件/扩展不能绕过 Tool Grant、Policy Gate、Event Store；V1 UI 覆盖 Player/KP/Admin/Developer 最小界面 |
| S13 | V1 Release Hardening 与总验收 | `ALL`<br>ALL | `workspace` | 17 条 V1 完成标准逐项有证据；P0/P1 defects=0 |


## 3. 阶段不可跳过门禁

| 门禁 | 必须发生在 | 说明 |
|---|---|---|
| AGENTS / persistent context 落位 | S00 | 后续所有 Codex 任务依赖。 |
| shared-kernel 编译通过 | S01 | 后续 crate 不得复制基础类型。 |
| Authority Contract 与 Event 模型稳定 | S02 | 后续 data/runtime/API/agent 必须复用。 |
| Event Store / migration / outbox 稳定 | S03 | 任何正式状态必须有正史链路。 |
| Policy Gate / Visibility 泄露负例通过 | S04 | Agent/API/UI 之前必须完成。 |
| COC7 规则核心通过 | S05 | Tutorial/Golden 与 AI KP 裁定依赖。 |
| Decision Commit Pipeline 通过 | S06 | Agent 正式裁定落库依赖。 |
| Agent Gateway / Tool Gate / Provider 通过 | S07 | 禁止直接 LLM 的实现证据在此形成。 |
| OpenAPI/WebSocket 契约通过 | S08 | UI、E2E、deployment 依赖。 |
| Docker Compose smoke 通过 | S09 | V1 acceptance 的部署基础。 |
| 备份恢复与 projection rebuild 通过 | S10 | 运维发布基础。 |
| Golden/Tutorial 全量测试通过 | S11 | 不通过不得进入 release hardening。 |
| UI/SDK 边界通过 | S12 | V1 可玩界面与扩展安全边界。 |
| 17 条 V1 acceptance 有证据 | S13 | 发布唯一完成标准。 |

## 4. 阶段确认输出模板

见 `prompts/persistent/03_STAGE_START_TEMPLATE.md` 与 `prompts/persistent/04_STAGE_ACCEPTANCE_TEMPLATE.md`。
