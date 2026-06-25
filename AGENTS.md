# AGENTS.md

本仓库用于实现 TRPG 在线游玩平台。开始任何任务前，按顺序阅读：

1. `DECISIONS.md`
2. `docs/PRODUCT_SYSTEM_DESIGN.md`
3. `docs/BACKEND_ARCHITECTURE.md`
4. `docs/UI_UX_SPEC.md`
5. 当前任务对应的 `prompts/*.md`

## 已定稿事项

不要重新询问或修改以下基线：云服务 + 自托管双形态、三档隐私模式、Generic Percentile + D&D SRD + 合法商业规则适配器、场景板 + 方格 + 六角格、Creator 可选插图、乐观锁 + 死锁检测、CRDT 仅笔记/线索布局。

## Architecture Rules

- 后端主要实现使用 Rust、Axum、Tokio、SQLx。
- 所有模型调用必须通过 `crates/llm_client`；图像调用通过 `ImageProvider` 边界。
- 所有检索必须通过 `crates/rag_core`。
- 规则系统代码适配器与规则正文内容包必须解耦。
- 权威状态写入必须携带 `expected_version`；可重试命令必须携带 `idempotency_key`。
- 捕获 PostgreSQL `40P01/40001/55P03`，只对幂等命令进行最多三次退避重试。
- 外部模型、媒体和通知副作用通过 Transactional Outbox 在事务提交后触发。
- CRDT 只能用于协作笔记和线索图布局。
- PL 客户端永远不能收到 KP-only 数据；不能先发送再用 CSS 隐藏。
- 骰子随机数与数学判定不得委托给 LLM。
- 所有会改变游戏状态的 Agent 输出必须经过 JSON Schema 校验。
- Creator 图片必须 draft-first、人工批准、费用和许可可审计。

## Security and Legal Rules

- API Key 不得进入前端、源码、日志、快照或测试 fixture。
- 未知许可进入 `pending_review`。
- 不自动抓取或捆绑未授权商业规则正文。
- 商业规则适配器测试使用自造或明确许可 fixture。
- `local_only` 房间不得调用云端 chat、embedding、rerank 或 image provider。
- 所有房间、文档、线索、Agent payload 查询逐请求鉴权，数据库启用 RLS。

## UI Rules

- 地图必须支持 scene board、square、hex flat-top、hex pointy-top。
- 关键过场默认 1200ms，支持 reduced/off。
- 音频向用户暴露七通道。
- 线索抽屉默认不覆盖中央舞台，并支持独立窗口。
- 版本冲突不得静默覆盖；保留草稿并同步最新快照。

## Build and Test

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo sqlx prepare --check
pnpm lint
pnpm typecheck
pnpm test
pnpm test:e2e
```

## Definition of Done

- 代码编译、格式化、lint、测试通过。
- 新接口有强类型请求/响应、OpenAPI、鉴权和测试。
- 新表有 migration、索引、RLS 和回滚/兼容说明。
- 新 Agent 调用有 token、费用、证据和 provider 审计。
- 新写路径有 optimistic conflict 与 deadlock retry 测试。
- 文档同步更新。
