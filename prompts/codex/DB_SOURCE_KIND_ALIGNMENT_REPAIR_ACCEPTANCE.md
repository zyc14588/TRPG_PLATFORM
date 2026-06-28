# Codex Prompt — DB SourceKind Alignment Repair Acceptance

你是 `TRPG_PLATFORM` 仓库的独立验收 Codex，运行在 Windows Codex App 环境中。

本 session 是只读验收，不是实现。

## 只读规则

- 不要修改源代码、测试、迁移、文档或 lockfile。
- 不要运行会自动改写 tracked files 的命令。
- 可以运行只读检查，例如 `cargo fmt --all --check`、`cargo check`、`cargo test`、`cargo sqlx prepare --check --workspace`。
- 如果 DB 不可用，继续做静态/compile 审查，并明确报告环境 blocker。
- 不要因为环境缺失伪造 PASS。

## 验收对象

P2 B02 repair — SourceKind / storage / DB CHECK alignment。

原始 blocker：

```text
crates/storage/src/lib.rs
source_kind_as_str 未覆盖 SourceKind::KpPrivateModule 与 SourceKind::SystemInternal。
source_kind_from_str 未解析 kp_private_module 与 system_internal。
migration CHECK 约束未包含这些 source kind。
cargo sqlx prepare --check --workspace、cargo test -p storage、cargo test --workspace 在编译或 schema 阶段失败。
```

## 收集仓库状态

运行：

```powershell
git status --short
git branch --show-current
git log --oneline -8
git diff --stat
git diff --name-status
cargo metadata --no-deps
rg -n "enum SourceKind|source_kind_as_str|source_kind_from_str|kp_private_module|system_internal|document_sources_source_kind_check|documents_source_kind_check|open_license|user_upload|commercial_adapter" crates migrations docs
```

## 必须检查的文件

```text
crates/rag_core/src/lib.rs
crates/storage/src/lib.rs
migrations/**
docs/p2/db/13_SOURCE_KIND_DB_ALIGNMENT.md
docs/status/P2_DATABASE_STATUS.md
docs/status/P2_STATUS.md
```

## 验收标准

### 1. 批次范围

PASS 要求：

- 修改集中在 `crates/storage/**`、`migrations/**`、状态文档和本 repair 文档/提示词。
- 没有新增 server API、frontend UI、Rig agent engine。
- 没有改掉 `rag_core::SourceKind` 的核心语义。

FAIL 条件：

- 为了通过测试删除 `KpPrivateModule` 或 `SystemInternal`。
- 将 storage 修复混入 API/UI/Rig。
- 改动削弱 RLS 或 app role policy。

### 2. storage mapping

PASS 要求：

`source_kind_as_str` 或等价写入映射包含：

```text
OfficialSrd               -> official_srd
OpenText                  -> open_text
UserProvidedText          -> user_provided_text
CampaignNotes             -> campaign_notes
CharacterSheet            -> character_sheet
ModulePrivateNotes        -> module_private_notes
KpPrivateModule           -> kp_private_module
CommercialAdapterMetadata -> commercial_adapter_metadata
SystemInternal            -> system_internal
Unknown                   -> unknown
```

`source_kind_from_str` 或等价读取映射解析：

```text
official_srd
open_text
user_provided_text
campaign_notes
character_sheet
module_private_notes
kp_private_module
commercial_adapter_metadata
system_internal
unknown
```

并且如果 legacy aliases 仍存在于 migrations/DB，解析：

```text
open_license       -> OpenText
user_upload        -> UserProvidedText
commercial_adapter -> CommercialAdapterMetadata
```

FAIL 条件：

- `kp_private_module` 或 `system_internal` 仍缺失。
- 任意未知 DB 字符串被静默映射为 `Unknown`。
- `source_kind_as_str` 使用 `_ => ...` 掩盖 future enum variants。

### 3. tests

PASS 要求至少有等价覆盖：

```text
all canonical variants stringify
all canonical strings parse
legacy aliases parse
invalid DB value rejects
roundtrip canonical values
```

### 4. migration

PASS 要求：

- 新增 additive migration。
- 不重写旧 committed migration。
- migration drop/recreate `document_sources_source_kind_check`。
- migration drop/recreate `documents_source_kind_check`。
- 两个 CHECK 约束都包含：
  - `kp_private_module`
  - `system_internal`
- 约束仍保留现有合法 source_kind values，避免破坏老数据。
- `documents.source_kind` 如果 nullable，CHECK 继续允许 NULL。

FAIL 条件：

- 只修 Rust，不修 DB constraints。
- 只修 DB constraints，不修 Rust mapping。
- old migration 被不当重写。
- DB constraints 仍拒绝 canonical variants。

### 5. commands

运行：

```powershell
cargo fmt --all --check
cargo check --workspace
cargo test -p rag_core
cargo test -p storage
```

如果 DB 可用，运行：

```powershell
. .\scripts\dev\db\env.ps1
cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"
.\scripts\dev\db\grant-app-role.ps1
.\scripts\dev\db\verify.ps1
cargo sqlx prepare --check --workspace
cargo test -p storage
cargo test --workspace
```

如果 DB 不可用，报告具体缺什么，例如：

```text
DATABASE_URL missing
TRPG_DATABASE_ADMIN_URL missing
PostgreSQL/pgvector unavailable
Docker unavailable
```

不能因此声称 full PASS。

## 最终报告格式

```text
## 验收结论
- Result: PASS / CONDITIONAL PASS / FAIL / BLOCKED
- Batch: P2 B02 repair — SourceKind / DB CHECK alignment
- 当前分支:
- 变更范围摘要:

## 阻塞问题
- 文件路径:
- 原因:
- 影响:
- 建议修复:

## 批次越界检查
- 结果:

## Storage mapping 检查
- canonical write mapping:
- canonical read mapping:
- legacy alias handling:
- invalid value behavior:

## Migration / DB CHECK 检查
- additive migration:
- document_sources_source_kind_check:
- documents_source_kind_check:
- DB proof:

## 测试与命令
- 已运行命令及结果:
- 未运行命令及原因:

## 文档与状态
- status 文档是否更新:
- remaining risks:

## 下一步
- PASS: 回到 P2 B02 Storage/RLS full acceptance。
- CONDITIONAL PASS: 补跑 DB proof 后再进入 full acceptance。
- FAIL/BLOCKED: 最小 repair plan。
```
