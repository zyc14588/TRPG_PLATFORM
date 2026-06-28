# Codex Prompt — P2 B02 Ingest Status / Storage Alignment Acceptance

你是 TRPG_PLATFORM 仓库的独立验收 Codex。

本 session 只读验收，不要修改文件。

批次：P2 B02 ingest status/storage alignment acceptance。

## 先收集状态

```powershell
git status --short
git branch --show-current
git diff --stat
git diff --name-status
cargo metadata --no-deps
```

## 必查文件

```text
docs/p2/19_INGEST_STATUS_SINGLE_SOURCE_POLICY.md
docs/status/P2_STATUS.md
crates/rag_core/src/lib.rs
crates/storage/src/lib.rs
migrations/**
```

## 静态搜索

```powershell
rg -n "enum .*IngestJobStatus|RagIngestJobStatus|IngestJobStatus|pending_review|denied|claimed|parsing|embedding|indexed|failed|CHECK.*ingest_jobs|BYPASSRLS|TRPG_TEST_MIGRATOR_DATABASE_URL|TRPG_DATABASE_ADMIN_URL" crates/rag_core crates/storage migrations docs scripts
```

## 验收标准

### 必须 PASS 的代码/设计条件

```text
- `rag_core::IngestJobStatus` 是 canonical semantic source。
- storage 不再定义独立的平行 ingest job status semantic enum。
- 如果 storage 保留 RagIngestJobStatus 名称，它必须是 type alias 或窄 adapter，不是第二套 enum。
- DB string conversion 显式且有 unknown value negative behavior。
- DB CHECK constraint 与 canonical statuses 对齐，至少覆盖 denied / pending_review / failed 等 P2 必需语义。
- 如果旧 migration 缺 status，修复以 additive migration 完成，而不是改旧 migration。
- migration/bootstrap tests 使用 migrator/admin URL。
- ordinary runtime DATABASE_URL 仍是 app role，不是 postgres。
- trpg_app 或等价 ordinary app role 不是 superuser，也没有 BYPASSRLS。
```

### 立即 FAIL

```text
- storage 与 rag_core 仍各自定义独立 enum，且没有明确转换/alignment tests。
- denied 不能持久化或 DB CHECK 不接受 canonical denied status。
- 为了让测试通过，把 DATABASE_URL 改成 postgres。
- 给 ordinary app role 授予 BYPASSRLS。
- 跳过 migration/RLS tests 或用 #[ignore] 制造绿色结果。
- 修改 server/frontend/API/UI。
```

### 可 BLOCKED / CONDITIONAL 的环境情况

如果本地没有 PostgreSQL/pgvector 或没有 migrator/admin URL，DB-backed proof 可以 BLOCKED，但必须满足：

```text
- 非 DB 编译/单测已运行；
- 静态审查未发现上述 FAIL 项；
- final report 写明缺少的 exact env var / service；
- 没有把 DB-backed gate 声称为 PASS。
```

## 建议命令

```powershell
if (Test-Path .\scripts\dev\db\env.ps1) { . .\scripts\dev\db\env.ps1 }

cargo fmt --all --check
cargo check --workspace
cargo test -p storage
```

如果存在 migrator/admin URL：

```powershell
cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"
cargo sqlx prepare --check --workspace
cargo test --workspace
```

## 最终报告格式

```markdown
## 验收结论
- Result: PASS / CONDITIONAL PASS / FAIL / BLOCKED
- Batch: P2 B02 ingest status/storage alignment
- 当前分支:
- Diff basis:

## 阻塞问题
- 文件路径、原因、影响、建议修复方式。

## 状态模型检查
- Canonical source:
- Storage strategy:
- DB CHECK alignment:
- Tests proving alignment:

## DB / migrator URL 检查
- DATABASE_URL role:
- Migrator/admin URL present:
- RLS proof role:

## 批次越界检查
- 是否修改 server/frontend/API/UI。

## 测试与命令
- 已运行命令及结果。
- 未运行命令及原因。

## 下一步
- PASS：回到 P2 B02 full Storage/RLS acceptance。
- FAIL/BLOCKED：最小 repair plan。
```
