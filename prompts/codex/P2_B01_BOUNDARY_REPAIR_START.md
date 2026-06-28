# Codex Prompt — P2 B01 Boundary Repair Start

你是 Windows Codex App 中的 TRPG_PLATFORM repair agent。

批次：P2 B01 boundary repair。

目标：修复 B01 验收中暴露的批次隔离和状态报告问题。不要在本 session 修 storage、migrations、RLS 或 server/frontend。

## 必须阅读

```text
CODEX_P2_MASTER_PROMPT.md
docs/p2/INDEX.md
docs/p2/00_EXECUTION_RULES.md
docs/p2/03_RAG_CORE_DOMAIN_MODEL.md
docs/p2/18_B01_SCOPE_REPAIR_GUIDE.md
docs/p2/19_INGEST_STATUS_SINGLE_SOURCE_POLICY.md
```

如果某个文件不存在，先在最终报告中记录；不要因此越界修改 runtime。

## 允许修改

```text
crates/rag_core/**
crates/document_ingestor/**            # 仅限 B01 domain compatibility
docs/status/P2_STATUS.md
docs/p2/**                            # 仅限 repair/status 文档
prompts/codex/**                      # 仅限本 repair pack 的 prompt 落地
```

## 禁止修改

```text
crates/storage/**
migrations/**
crates/server/**
apps/web/**
schemas/openapi.json
.env.example
```

如果当前工作区已经有 `crates/storage/**` 未提交改动：

1. 先保存 patch 到仓库外的 `..\_trpg_codex_deferred\`。
2. 从当前 B01 工作区恢复 storage 文件。
3. 不要把保存的 patch 加入本次 B01 diff。
4. 在最终报告中说明 patch 路径，建议 B02 使用。

PowerShell 参考命令：

```powershell
$deferredDir = Join-Path (Split-Path (Get-Location) -Parent) "_trpg_codex_deferred"
New-Item -ItemType Directory -Force $deferredDir | Out-Null

git diff -- crates/storage/src/lib.rs | Out-File -FilePath (Join-Path $deferredDir "P2_B02_storage_worktree_deferred.patch") -Encoding utf8
git diff --cached -- crates/storage/src/lib.rs | Out-File -FilePath (Join-Path $deferredDir "P2_B02_storage_index_deferred.patch") -Encoding utf8

git restore --staged -- crates/storage/src/lib.rs
git restore -- crates/storage/src/lib.rs
```

如果 storage 改动已经是已提交历史的一部分，不要擅自 rewrite history。最终报告标记 BLOCKED，并建议从正确 base 创建 clean B01 branch 或由 owner 批准 reverse patch。

## 任务

1. 运行：

```powershell
git status --short
git branch --show-current
git diff --stat
git diff --name-status
cargo metadata --no-deps
```

2. 检查 B01 diff 是否触及 `crates/storage/**`。若触及且未提交，按上面流程保存并恢复。

3. 确认 `docs/status/P2_STATUS.md` 存在。若不存在，从 `docs/status/P2_STATUS_TEMPLATE.md` 或 `docs/p2/18_B01_SCOPE_REPAIR_GUIDE.md` 中的 skeleton 创建。

4. 更新 `docs/status/P2_STATUS.md`，记录：

```text
- 当前批次：P2 B01 — RAG Domain and Schema Contracts
- B01 允许范围内的变更摘要
- storage diff 已移除 / 无 storage diff
- cargo fmt/check/test 证据
- 如果 workspace test 未运行或因缺少 migrator/admin URL 失败：写明不是 B01 代码通过，而是 DB test environment blocker
- Deferred to B02：storage ingest job status alignment with rag_core::IngestJobStatus
```

5. 不要在 B01 中修改 `crates/storage/src/lib.rs` 来解决 `RagIngestJobStatus` / `IngestJobStatus` 平行模型。只在 status 中记录为 B02 deferred item。

6. 运行 B01 范围检查：

```powershell
cargo fmt --all --check
cargo check -p rag_core
cargo test -p rag_core
```

如果 `document_ingestor` crate 存在并被 B01 触及：

```powershell
cargo check -p document_ingestor
cargo test -p document_ingestor
```

7. 只有在环境中存在以下变量之一时，才运行 full workspace DB-sensitive gate：

```powershell
$env:TRPG_TEST_MIGRATOR_DATABASE_URL
$env:TRPG_DATABASE_ADMIN_URL
```

如果存在，运行：

```powershell
cargo test --workspace
```

如果不存在，不要把 `DATABASE_URL` 改成 postgres；在最终报告中写明 workspace gate blocked by missing migrator/admin URL。

## 最终报告格式

```markdown
## Batch summary
- Batch: P2 B01 boundary repair
- Branch:
- Files changed:
- Storage changes removed from B01: yes/no/not applicable
- Deferred patch path, if any:
- P2_STATUS updated: yes/no
- Tests/checks run:
- Results:
- Environment blockers:
- Deferred to B02:
- Ready for B01 acceptance: yes/no
```
