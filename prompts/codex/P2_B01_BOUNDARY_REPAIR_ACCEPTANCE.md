# Codex Prompt — P2 B01 Boundary Repair Acceptance

你是 TRPG_PLATFORM 仓库的独立验收 Codex。

本 session 只读验收，不要修改文件。

批次：P2 B01 boundary repair acceptance。

## 先收集状态

```powershell
git status --short
git branch --show-current
git diff --stat
git diff --name-status
cargo metadata --no-deps
```

如果当前工作区没有未提交变更，查看最近提交或与 base 的 diff。不要猜测；说明使用的 diff basis。

## 必查文件

```text
CODEX_P2_MASTER_PROMPT.md
docs/p2/INDEX.md
docs/p2/18_B01_SCOPE_REPAIR_GUIDE.md
docs/p2/19_INGEST_STATUS_SINGLE_SOURCE_POLICY.md
docs/status/P2_STATUS.md
crates/rag_core/**
crates/document_ingestor/**
crates/storage/src/lib.rs
```

## 阻塞判定

### 立即 FAIL

出现任一项则 FAIL：

```text
- B01 diff 仍触及 crates/storage/**。
- docs/status/P2_STATUS.md 不存在。
- P2_STATUS 没有记录 B01 命令证据。
- P2_STATUS 没有记录 storage ingest status alignment deferred to B02。
- B01 为了让 workspace test 通过，把 runtime DATABASE_URL 改成 postgres。
- B01 修改了 migrations、server、frontend、OpenAPI 或 .env.example。
```

### CONDITIONAL PASS 可接受条件

只有在以下全部满足时，才可给 CONDITIONAL PASS：

```text
- B01 diff 只包含 rag_core / document_ingestor compatibility / docs/status / docs/p2 / prompts/codex。
- cargo fmt/check/test for rag_core 通过。
- document_ingestor 如果存在且被触及，对应 check/test 通过。
- cargo test --workspace 唯一未通过或未运行的原因是缺少 TRPG_TEST_MIGRATOR_DATABASE_URL 或 TRPG_DATABASE_ADMIN_URL。
- P2_STATUS 明确记录该 DB blocker，且没有把 full workspace gate 写成 PASS。
```

### PASS 条件

```text
- 满足 CONDITIONAL PASS 的所有代码/文档条件。
- full workspace gate 在正确 DB/migrator 环境中通过。
- 没有未解释的 environment blocker。
```

## 建议命令

```powershell
cargo fmt --all --check
cargo check -p rag_core
cargo test -p rag_core
```

如果 `document_ingestor` crate 存在：

```powershell
cargo check -p document_ingestor
cargo test -p document_ingestor
```

如果存在 migrator/admin URL：

```powershell
cargo test --workspace
```

否则记录：

```text
workspace DB-sensitive tests blocked by missing TRPG_TEST_MIGRATOR_DATABASE_URL or TRPG_DATABASE_ADMIN_URL
```

## 最终报告格式

```markdown
## 验收结论
- Result: PASS / CONDITIONAL PASS / FAIL / BLOCKED
- Batch: P2 B01 boundary repair
- 当前分支:
- Diff basis:

## 阻塞问题
- 文件路径、原因、影响、建议修复方式。

## 批次越界检查
- B01 是否仍触及 storage/migrations/server/frontend/openapi/env。

## 状态文件检查
- docs/status/P2_STATUS.md 是否存在。
- 是否记录 B01 evidence。
- 是否记录 B02 deferred status-model alignment。

## 测试与命令
- 已运行命令及结果。
- 未运行命令及原因。

## 下一步
- 如果 PASS/CONDITIONAL PASS：进入 P2 B02 ingest status/storage alignment。
- 如果 FAIL/BLOCKED：最小 repair plan。
```
