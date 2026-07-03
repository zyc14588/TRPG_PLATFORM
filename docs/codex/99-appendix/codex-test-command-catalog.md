> [v2.21 自包含来源清理标记]
> 原始路径：`docs/codex/99-appendix/codex-test-command-catalog.md`
> 筛选状态：`appendix-reference`
> 清理日期：2026-07-01
> 使用规则：当前可引用：可由 Codex 读取并参与施工，但必须服从顶层设计与 v2.21 阶段门禁。
> 过时信息处理：正文中出现的 `V4`、`V5`、早期 audit/fix/report 标题、源文档 hash、旧中间路径与历史版本流水仅表示 provenance，不得作为当前产品范围、命名规则或验收标准。若与顶层设计、`AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md` 或 `V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 冲突，以后者为准。

> [v2.21 当前执行规范化覆盖]
> 执行任何 batch、category prompt 或 per-file prompt 前，必须先读取 `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`。
> 正文中的 V3/V4/V5/V6、v3/v4/v5/v6、legacy、fix-history、旧 manifest、旧 report、旧 hash 与旧中间路径仅保留为 provenance。任何 Rust module、输出文件、migration、event schema、NATS subject、metric label、测试名或验收入口必须采用 v2.21 normalized current-safe 名称。

# Codex Test Command Catalog

## 基础命令

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

## 单 crate 命令

```bash
cargo test -p trpg-shared-kernel --all-features
cargo test -p trpg-domain-core --all-features
cargo test -p trpg-runtime --all-features
cargo test -p trpg-agent-runtime --all-features
cargo test -p trpg-ruleset-coc7 --all-features
cargo test -p trpg-data-eventing --all-features
cargo test -p trpg-api --all-features
cargo test -p trpg-security-governance --all-features
cargo test -p trpg-testing --all-features
```

## 数据库与契约

```bash
sqlx migrate run
sqlx migrate revert
cargo test --test event_store_contract
cargo test --test projection_replay
cargo test --test visibility_leakage
cargo test --test openapi_contract
cargo test --test websocket_contract
cargo test --test nats_subject_contract
```

## 失败处理

不要删除测试或弱化约束。先记录失败，再最小修复，最后重跑相关测试。
