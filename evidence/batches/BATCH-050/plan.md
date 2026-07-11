# BATCH-050 工作计划

Batch: `BATCH-050-90-traceability — Strict Governance Final`  
Stage: `S00 — 治理落位与 Codex 施工入口`  
Prompt 数量: 10  
Primary / Supplemental: 0 / 0  
Documentation-or-traceability: 10  
Current-safe 唯一目标: 8

## 范围

本批全部条目均为 `traceability-maintenance` / `documentation-or-traceability`。
只允许维护下表 current-safe Markdown 目标与 `evidence/batches/BATCH-050/`。
不得创建或修改业务 Rust `src/`、产品测试、migration、API/Event/WS/NATS
契约、metric、workflow、provider adapter 或正式状态写入路径。

历史版本标签、hash 和旧源路径仅可保留在 provenance 字段；当前 module 与
output 以两份 current-safe 映射为准。尤其 P0107 必须使用
`traceability::input_traversal_audit_previous_provenance` 与
`read_every_file_audit_previous.md`，不得采用 batch/index 中较低优先级的旧建议名。

## Prompt 映射与测试责任

| Prompt ID | Prompt 文件 | Current-safe module | Current-safe 目标 | 允许改动范围 | 测试责任 |
|---|---|---|---|---|---|
| `CODEX-1065-90-TRACEABILITY-eda89799e3` | `codex-prompts/90-traceability/P0100.md` | `traceability::chatgpt_followup_research_prompts_impl` | `docs/codex/90-traceability/chatgpt_followup_research_prompts_impl.md` | 追加 Markdown provenance/traceability，保留旧 batch 记录 | 目标、Prompt ID、prompt/source/SHA、module/output、映射一致性、docs-only 边界 |
| `CODEX-1066-90-TRACEABILITY-0503a7b78f` | `codex-prompts/90-traceability/P0098.md` | `traceability::backlog_open_questions_impl` | `docs/codex/90-traceability/backlog_open_questions_impl.md` | 追加 Markdown provenance/traceability，保留旧 batch 记录 | 同上 |
| `CODEX-1067-90-TRACEABILITY-9bb3354c34` | `codex-prompts/90-traceability/P0099.md` | `traceability::implementation_plan_impl` | `docs/codex/90-traceability/implementation_plan_impl.md` | 追加 Markdown provenance/traceability，保留旧 batch 记录 | 同上 |
| `CODEX-1068-90-TRACEABILITY-c50cc50eaf` | `codex-prompts/90-traceability/P0101.md` | `traceability::readme` | `docs/codex/90-traceability/readme.md` | 合并到单一 B050 README 标记段，不覆盖旧段 | 同上；并验证 3 条共享目标记录 |
| `CODEX-1069-90-TRACEABILITY-e8e261e885` | `codex-prompts/90-traceability/P0102.md` | `traceability::readme` | `docs/codex/90-traceability/readme.md` | 合并到单一 B050 README 标记段，不覆盖旧段 | 同上；并验证 3 条共享目标记录 |
| `CODEX-1070-90-TRACEABILITY-90042976ac` | `codex-prompts/90-traceability/P0103.md` | `traceability::manifest` | `docs/codex/90-traceability/manifest.md` | 追加 Markdown manifest trace，不扩大为包清单 | 同上 |
| `CODEX-1071-90-TRACEABILITY-289dbea468` | `codex-prompts/90-traceability/P0104.md` | `traceability::readme` | `docs/codex/90-traceability/readme.md` | 合并到单一 B050 README 标记段，不覆盖旧段 | 同上；并验证 3 条共享目标记录 |
| `CODEX-1096-90-TRACEABILITY-7c9e6f6016` | `codex-prompts/90-traceability/P0107.md` | `traceability::input_traversal_audit_previous_provenance` | `docs/codex/90-traceability/read_every_file_audit_previous.md` | 新建 previous-provenance Markdown；不得成为当前验收入口 | 精确 override、非当前验收边界及上述通用检查 |
| `CODEX-1097-90-TRACEABILITY-15a234659d` | `codex-prompts/90-traceability/P0109.md` | `traceability::requirement_to_test_trace` | `docs/codex/90-traceability/requirement_to_test_trace.md` | 新建 Markdown requirement-to-test trace | 通用检查；确认只声明文档测试责任 |
| `CODEX-1098-90-TRACEABILITY-0318fc2dd7` | `codex-prompts/90-traceability/P0110.md` | `traceability::top_level_principle_trace` | `docs/codex/90-traceability/top_level_principle_trace.md` | 新建 Markdown top-level principle trace | 通用检查；确认顶层红线完整 |

## 实施策略

- 复用 B046–B049 已建立的简洁 trace-page 格式。
- 对 5 个既有目标只追加 B050 标记段；创建 3 个缺失目标。
- 记录 Prompt ID、prompt 路径、source path/SHA、current crate/module/output、
  docs-only disposition 与测试责任。
- 不复制 prompt provenance 中的历史 Rust、SQL、API、event、NATS、metric、
  workflow 或测试提案。

## 检查顺序

1. B050 最小检查：10 行解析、10/10 normalized/safe 映射一致、8 个目标存在、
   Prompt/source/SHA/module/output 闭合、Markdown 结构、docs-only 边界与敏感标签扫描。
2. S00 检查：`scripts/verify-governance-boundary.ps1`。
3. Workspace 检查：Cargo fmt/check/clippy/test、目标 visibility leakage、可用时
   `pnpm.cmd test`（补充）、`git diff --check`。
4. SQLx 与 Docker：本批不改数据库或部署面，记录为 N/A。
5. 按 `batch-prompts/accept/B050.md` 独立复跑最小检查与 S00 验收。

## 已知风险

- 较低优先级的 `per-file-prompt-index.md`、`per-file-prompt-manifest.md` 与
  `batches/B050.md` 仍保留 P0107 的旧建议 target/module；本批按高优先级 current
  maps 执行，不越权重写包输入。
- Cargo/Node 检查覆盖共享工作区基线，但本批没有产品代码或产品测试所有权。
