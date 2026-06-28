# Codex Prompt — DB SourceKind Alignment Repair Start

你是正在 Windows Codex App 中处理 `TRPG_PLATFORM` 仓库的 Codex。

批次：P2 B02 repair — SourceKind / storage / DB CHECK alignment。

## 目标

修复当前数据库构建阻塞：

```text
crates/storage/src/lib.rs
source_kind_as_str 未覆盖 SourceKind::KpPrivateModule 与 SourceKind::SystemInternal。
source_kind_from_str 未解析 kp_private_module 与 system_internal。
migration CHECK 约束未包含这些 source kind。
cargo sqlx prepare --check --workspace、cargo test -p storage、cargo test --workspace 在编译或 schema 阶段失败。
```

这是 B02 storage/database contract repair，不是 B01 domain repair，不是 DB 环境 repair。

## 必须先阅读

如存在，必须阅读：

```text
CODEX_P2_MASTER_PROMPT.md
CODEX_DB_MASTER_PROMPT.md
docs/p2/INDEX.md
docs/p2/db/INDEX.md
docs/p2/db/02_DATABASE_URL_CONTRACT.md
docs/p2/db/06_MIGRATIONS_SQLX_POLICY.md
docs/p2/db/07_STORAGE_TEST_HARNESS_POLICY.md
docs/p2/db/13_SOURCE_KIND_DB_ALIGNMENT.md
docs/status/P2_DATABASE_STATUS.md
docs/status/P2_STATUS.md
crates/rag_core/src/lib.rs
crates/storage/src/lib.rs
migrations/**
```

## Shell 规则

- 使用 Windows PowerShell 兼容命令。
- 不要使用 Bash-only 语法，除非当前 shell 明确是 Git Bash/WSL。
- 如果 `rg` 不存在，用 `git grep -n` 或 `Select-String`。
- 不要假设 DB 已经运行。
- 不要把 runtime `DATABASE_URL` 改成 postgres superuser。

## 开始前收集状态

运行：

```powershell
git status --short
git branch --show-current
git log --oneline -8
git diff --stat
git diff --name-status
cargo metadata --no-deps
rg -n "enum SourceKind|source_kind_as_str|source_kind_from_str|kp_private_module|system_internal|document_sources_source_kind_check|documents_source_kind_check" crates migrations docs
```

## 允许修改范围

允许：

```text
crates/storage/**
migrations/**
docs/status/P2_DATABASE_STATUS.md
docs/status/P2_STATUS.md
docs/p2/db/13_SOURCE_KIND_DB_ALIGNMENT.md
prompts/codex/DB_SOURCE_KIND_ALIGNMENT_REPAIR_*.md
```

不允许：

```text
crates/rag_core/**，除非只是测试/文档极小修正且不得改 enum 语义
crates/server/**
apps/web/**
Rig / agent engine 相关代码
旧 committed migrations 的重写
RLS disable / FORCE RLS removal
.env.example 中把 DATABASE_URL 改为 postgres superuser
```

## 修复要求

### 1. storage 映射

找到当前等价函数：

```text
source_kind_as_str
source_kind_from_str
row mapper / conversion helpers
```

修复为：

```text
SourceKind::OfficialSrd               -> official_srd
SourceKind::OpenText                  -> open_text
SourceKind::UserProvidedText          -> user_provided_text
SourceKind::CampaignNotes             -> campaign_notes
SourceKind::CharacterSheet            -> character_sheet
SourceKind::ModulePrivateNotes        -> module_private_notes
SourceKind::KpPrivateModule           -> kp_private_module
SourceKind::CommercialAdapterMetadata -> commercial_adapter_metadata
SourceKind::SystemInternal            -> system_internal
SourceKind::Unknown                   -> unknown
```

`source_kind_from_str` 必须解析以上全部 canonical strings。

如果当前 migration 或 DB 已经使用 legacy aliases，也要解析这些 aliases：

```text
open_license       -> SourceKind::OpenText
user_upload        -> SourceKind::UserProvidedText
commercial_adapter -> SourceKind::CommercialAdapterMetadata
```

不要把任意未知 DB 字符串静默映射为 `SourceKind::Unknown`。只有 literal `"unknown"` 映射为 `SourceKind::Unknown`。其他未知值必须返回项目现有 storage/repository data error。

### 2. tests

新增或补齐 storage tests：

```text
source_kind_as_str_covers_all_canonical_variants
source_kind_from_str_accepts_all_canonical_values
source_kind_from_str_accepts_legacy_aliases
source_kind_from_str_rejects_invalid_database_value
source_kind_round_trip_canonical_values
```

如果项目测试命名不同，可以使用等价测试名，但最终报告必须列出映射。

### 3. additive migration

新增一个 migration，名称类似：

```text
p2_b02_source_kind_alignment
```

不要修改旧 committed migration。

新 migration 必须 drop/recreate：

```text
document_sources_source_kind_check
documents_source_kind_check
```

并包含：

```text
official_srd
open_license
open_text
user_upload
user_provided_text
campaign_notes
character_sheet
module_private_notes
kp_private_module
commercial_adapter
commercial_adapter_metadata
system_internal
unknown
```

如果 `documents.source_kind` 当前可为 NULL，CHECK 必须保留 `source_kind IS NULL OR ...`。

### 4. DB proof

如果 DB 环境可用：

```powershell
. .\scripts\dev\db\env.ps1

cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"
.\scripts\dev\db\grant-app-role.ps1
.\scripts\dev\db\verify.ps1

cargo sqlx prepare --check --workspace
cargo test -p storage
cargo test --workspace
```

如果 `TRPG_DATABASE_ADMIN_URL` / `DATABASE_URL` 不存在，不要伪造 DB proof。报告 BLOCKED 或 CONDITIONAL，取决于 compile/static tests 是否通过。

## 必须避免的错误修法

- 不要删除 `SourceKind::KpPrivateModule`。
- 不要删除 `SourceKind::SystemInternal`。
- 不要用 `_ => "unknown"` 掩盖 future enum variants。
- 不要把 `DATABASE_URL` 设置成 postgres superuser。
- 不要给普通 app role 授予 `BYPASSRLS`。
- 不要跳过 SQLx prepare。
- 不要 `#[ignore]` 掉关键 storage tests 来制造绿色结果。
- 不要改 server/frontend/Rig。

## 建议检查命令

```powershell
cargo fmt --all --check
cargo check --workspace
cargo test -p rag_core
cargo test -p storage
```

DB 可用时追加：

```powershell
. .\scripts\dev\db\env.ps1
cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"
.\scripts\dev\db\grant-app-role.ps1
.\scripts\dev\db\verify.ps1
cargo sqlx prepare --check --workspace
cargo test -p storage
cargo test --workspace
```

## 最终报告格式

```text
## Batch summary
- Batch: P2 B02 repair — SourceKind / DB CHECK alignment
- Files changed:
- Storage mapping changes:
- Migration added:
- Tests added/changed:
- Commands run:
- Results:
- DB proof:
- Remaining blockers:
- Next recommended gate:
```
