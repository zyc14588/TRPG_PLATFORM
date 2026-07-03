# AGENTS.md — COC AI TRPG Codex 持久化施工指令

Codex 在本仓库执行任何编码、测试、重构、修复、评审或文档治理任务前，必须读取本文件、`docs/codex/00-index/codex-persistent-context.md`、`docs/codex/00-index/codex-prompt-boundary.md`、当前阶段 `START_PROMPT.md` 与相关 per-file prompt。

## 不可突破原则

- Authority Contract 不可变；变更只能 fork 或创建新的 authority_contract_version。
- HUMAN_KP / AI_KP 在 Campaign 级互斥；同一 campaign 不得同时接受两种最终裁定权。
- AI 不能直接写正式状态；AI 只能提出 Proposal / ToolCall / DraftDecision。
- 正式状态必须经过 Command -> Workflow -> Decision -> Event Store -> Projection。
- Event Store 是正史；Projection、Cache、RAG Index 都是可重建读模型。
- Visibility Label 与 Fact Provenance 必须跨 API、Event、Agent、RAG、Export、Replay、Log、Metric 传播。
- Tool Grant、Policy Gate、OpenFGA、OPA、Audit Log 不得被 Agent、插件、handler 或 provider 绕过。
- 所有写命令必须携带 idempotency_key、expected_version、actor、correlation_id、causation_id。
- 业务层、KP 服务、规则引擎、前端不得直接调用 LLM；所有模型调用必须经 Agent Gateway -> Agent Runtime -> Model Provider Adapter。
- 本地模型是一等 Provider，但不享有特权路径；未认证 Level 4 的本地模型不能担任 AI Keeper Orchestrator；不得静默 fallback 到云端。

## Codex 工作方式

1. 先定位阶段与 batch，再读取模块级 `codex-module-code-prompt.md`、`codex-module-test-prompt.md` 与 per-file prompt。
2. 修改代码前输出最小执行计划，列出 crate、migration、schema、API/Event、测试文件。
3. 只允许 primary-implementation prompt 创建 concrete Rust src/test 输出。
4. supplemental-requirement 只写入 `docs/codex/90-traceability/supplemental-requirements/<Prompt ID>.md` 并说明归并到哪个 primary。
5. documentation-or-traceability 只维护 Markdown、索引、矩阵、报告或验证清单。
6. 不得生成 ModuleService、ModuleCommand、ModuleError 或“核心接口占位”等模板残留。
7. 不得把 serde_json::Value 当作最终领域模型；只能用于受控 schema boundary。
8. 新增写命令必须同步新增正向、幂等冲突、版本冲突、权限拒绝、visibility 泄露负例测试。
9. 新增 API/Event/WebSocket/NATS 契约必须同步更新 schema、contract test、trace map。
10. 完成后运行阶段测试；无法运行时说明环境限制和替代验证。

## 输出格式

每次任务结束必须输出：变更文件清单、测试命令与结果、schema/migration/event/API 变更、风险与 TODO、关联 Prompt ID。没有测试或没有验收证据的实现不得合并。
