# CURRENT_TOKEN_REWRITE_TABLE

> v2.21 当前施工 Token 重写表。此文件是唯一 canonical token rewrite 入口。任何旧 `CURRENT_TOKEN_REWRITE_TABLE.md` 只能在 `source-archive/**` 中作为 provenance alias 出现，不得作为当前施工入口。

| 来源历史 token 类别 | 当前施工动作 |
|---|---|
| `V3` / `V4` / `V5` / `V6` 作为标题或 provenance | 保留为历史说明，不进入 module、文件名、测试名、metric、NATS subject、migration。 |
| `v3` / `v4` / `v5` / `v6` 出现在可执行建议中 | 改写为 `previous` 或删除版本片段。 |
| `legacy` 出现在可执行建议中 | 改写为 `previous` 或 `provenance`；不得成为当前 module/output 名称。 |
| `poc` 出现在可执行建议中 | 改写为 `prototype`。 |
| 源 SHA / 文件名 hash 出现在可执行建议中 | 删除；只保留在 Prompt ID 或 provenance 字段。 |
| `fix-history/**` | 只读隔离，不得作为当前施工入口。 |
| 旧 manifest / 旧 delivery report / 旧 acceptance report | 只读隔离，不得作为当前验收结果。 |
| 旧 workflow Markdown | 只读隔离，不得作为当前 CI/CD extraction source。 |
| `CURRENT_TOKEN_REWRITE_TABLE.md` | 只读隔离；当前入口统一为 `CURRENT_TOKEN_REWRITE_TABLE.md`。 |
