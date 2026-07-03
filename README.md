# COC AI TRPG — Codex v2.21 Strict Self-contained Construction Package

## 0. 项目定位

本包是面向 Codex 的完整工程施工资料包，用于把 **COC 7 首发的 AI / 真人 KP 在线跑团平台** 从设计阶段推进到可编码、可测试、可验收、可准备发布的工程实现阶段。

项目本身不是普通聊天机器人，而是一个由规则包、角色卡、场景、线索、NPC、状态机、骰子、事件日志、可见性系统、Agent 裁定协议、多人实时同步、模型 Provider 和一键部署能力组成的游戏运行时。V1 的目标是完成一个可信、可部署、可审计、可完整游玩的 COC 7 闭环。

## 1. 本包面向的读者

| 读者 | 主要入口 | 目的 |
|---|---|---|
| Codex 施工会话 | `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`stages/**/START_PROMPT.md` | 分阶段执行编码、测试、评审和修复任务。 |
| 人类工程负责人 | `CODEX_MASTER_EXECUTION_GUIDE.md`、`CODEX_STRICT_OPERATION_CHECKLIST.md` | 控制施工顺序、验收节奏、变更范围和 evidence 归档。 |
| 审计人员 | `DOCUMENT_ORGANIZATION_AND_AUDIT_BOUNDARY.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md`、`manifests/**`、`inventory/**` | 检查文档来源、覆盖关系、包完整性和验收证据链。 |
| 发布负责人 | `CODEX_RELEASE_PREPARATION_GUIDE.md`、`codex-operator-guides/06_RELEASE_PREPARATION_PLAYBOOK.md` | 准备 release candidate、发布门禁、回滚与审计包。 |

## 2. 项目施工主线

Codex 必须按 S00 → S13 顺序施工，不得跳过阶段门禁：

```text
S00 Governance onboarding
S01 Foundation shared kernel
S02 Domain core: Authority Contract / Event model
S03 Data, eventing, persistence
S04 Security, governance, visibility, provenance
S05 COC7 ruleset engine
S06 Runtime orchestration and decision pipeline
S07 Agent runtime, provider, memory, RAG
S08 API / realtime / contracts
S09 Platform infrastructure and deployment
S10 Ops, migration, runbooks
S11 Testing, quality, Golden Scenario, CI
S12 Extension SDK and UI boundary
S13 V1 release hardening
```

## 3. 当前 Codex 读取顺序

Codex 新会话必须先读取以下文件，之后才能进入任何 batch 或阶段 prompt：

1. `AGENTS.md`
2. `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`
3. `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`
4. `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`
5. `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
6. `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
7. `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
8. `DOCUMENT_ORGANIZATION_AND_AUDIT_BOUNDARY.md`
9. `CODEX_MASTER_EXECUTION_GUIDE.md`
10. `CODEX_START_ACCEPT_TEST_RELEASE_GUIDE.md`
11. `CODEX_STRICT_OPERATION_CHECKLIST.md`
12. `V1_ACCEPTANCE_EVIDENCE_MATRIX.md`
13. `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`
14. `stages/s00-governance-onboarding/START_PROMPT.md`

## 4. 目录职责

| 路径 | 职责 | 当前执行权限 |
|---|---|---|
| `AGENTS.md` | 根级 Codex 持久化约束。 | 当前施工必读。 |
| `prompts/persistent/**` | 可长期放入仓库的 Codex 持久化辅助提示词。 | 当前施工可用。 |
| `stages/**` | 每阶段启动、验收、测试、测试数据和修复 prompt。 | 当前施工可用。 |
| `codex-operator-guides/**` | 给人类操作员和 Codex 会话使用的专项执行手册。 | 当前施工可用。 |
| `docs/top-level-design/**` | 顶层产品和架构设计基线。 | 当前设计权威。 |
| `docs/codex/**` | 筛选后的 V6 Codex 稳定施工材料、batch、per-file prompt 和 traceability。 | 当前参考/施工材料；执行前必须应用 normalized maps。 |
| `codex-active-normalized/**` | 当前 prompt 执行映射和安全 module/output 映射。 | 当前施工必读。 |
| `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md` | 唯一 canonical token rewrite 表。 | 当前施工必读；没有其他 active alias。 |
| `fixtures/**` | Tutorial、Golden、COC7、Visibility、Provider、API、Event Store、RAG、Export 等测试 fixture。 | 当前测试输入。 |
| `ci-cd/workflows-extractable/**` | 唯一 canonical CI/CD workflow Markdown 提取源。 | 当前 CI/CD 落库源。 |
| `inventory/**` | 输入筛选、Prompt 覆盖、batch 映射、清理审计。 | 审计和追踪用；不是施工 prompt。 |
| `manifests/**` | 当前包 manifest、hash、strict validation 报告。 | 包验收用。 |
| `source-archive/**` | 原始输入、旧报告、旧路径、旧 CI/CD 和 token alias provenance。 | 只读审计；不得作为当前施工入口。 |
| `source-archive/reviews/**` | 用户提供的历史验收报告与复核结论 provenance。 | 只读审计；不得作为当前施工入口。 |

## 5. CI/CD canonical source

当前唯一 CI/CD 提取源是：

```text
ci-cd/workflows-extractable/target-ci.yml.md
ci-cd/workflows-extractable/target-contracts.yml.md
ci-cd/workflows-extractable/target-golden-scenarios.yml.md
ci-cd/workflows-extractable/target-docker-compose-smoke.yml.md
ci-cd/workflows-extractable/target-release.yml.md
```

历史 `github-actions-*.yml.md` 文件已经转入 `source-archive/provenance/**`，只能用于来源追溯，不能作为 workflow 提取入口。

## 6. 验收入口

严格验收从以下文件开始：

- `STRICT_SELF_CONTAINED_ACCEPTANCE_REPORT.md`
- `STRICT_V221_ACCEPTANCE_REPORT.md`
- `V221_FULL_PACKAGE_MARKDOWN_CLEANUP_REPORT.md`
- `STRICT_LINK_AND_REFERENCE_VALIDATION.md`
- `manifests/V221_STRICT_VALIDATION_REPORT.md`
- `manifests/CURRENT_PACKAGE_MANIFEST.md`
- `inventory/V221_FULL_FILE_CLEANUP_AUDIT.md`

通过标准是：manifest 闭合、cleanup audit 覆盖全包、batch → prompt 引用闭合、guide heading 可渲染、README 声明目录真实存在、active token rewrite 入口唯一、CI/CD canonical source 唯一、active 区域无旧版本当前语义、actionable module/output 无旧版本命名。

## 7. 施工红线摘要

- V1 只允许 P0/P1 进入首发，P2/P3 默认进入 backlog。
- HUMAN_KP / AI_KP 是 Campaign 级互斥权威模式，Authority Contract 锁定后只能 fork。
- 所有正式裁定必须经过工具、规则引擎、状态服务和事件日志。
- AI 只能通过 Agent Gateway / Orchestrator / Runtime / Provider Adapter 工作。
- Agent 不能直接写数据库、伪造骰子、绕过规则引擎、泄露 keeper/private 内容或修改 Authority Contract。
- Visibility Label 与 Fact Provenance 必须贯穿 API、Event、Agent、RAG、Export、Replay、Log 和 Metric。
- 本地模型是一等 Provider，但未认证 Level 4 的模型不能担任 AI Keeper Orchestrator。
- 默认不得从本地模型静默 fallback 到云端。





## 8. 当前清理状态

本包为 v2.21 清理版。active/current 区域只保留当前施工所需信息；历史失败报告、旧 CI/CD、旧 token alias、旧路径材料均保留在 `source-archive/**`，只作 provenance，不得作为当前施工入口。



## v2.21 路径引用清理说明

当前每批次人工启动提示词位于 `batch-prompts/start/B###.md`，每批次人工验收提示词位于 `batch-prompts/accept/B###.md`。旧 batch prompt 路径只保留在 `inventory/PATH_REWRITE_MAP.md` 的 `old_path` 字段中作为 provenance。
