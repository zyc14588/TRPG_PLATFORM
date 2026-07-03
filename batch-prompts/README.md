# Batch Prompt References

本目录是 v2.21 当前唯一的 batch 级人工提示词入口。

- 启动提示词：`batch-prompts/start/B001.md` 至 `batch-prompts/start/B052.md`
- 验收提示词：`batch-prompts/accept/B001.md` 至 `batch-prompts/accept/B052.md`

使用规则：

1. 先读取 `README.md`、`AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`codex-active-normalized/**`。
2. 按 `batches/B###.md` 找到要执行的 batch。
3. 将对应 `batch-prompts/start/B###.md` 中的中文提示词复制给 Codex。
4. Codex 完成后，使用对应 `batch-prompts/accept/B###.md` 进行验收。
5. 旧路径只允许在 `inventory/PATH_REWRITE_MAP.md` 中作为 provenance 出现。
