# TRPG Platform — Final Design Pack

这是一套可直接交给 Codex 初始化工程的最终设计文件组。所有产品待决定项已关闭。

## 阅读顺序

1. `DECISIONS.md`
2. `AGENTS.md`
3. `docs/PRODUCT_SYSTEM_DESIGN.md`
4. `docs/BACKEND_ARCHITECTURE.md`
5. `docs/UI_UX_SPEC.md`
6. `docs/IMPLEMENTATION_HANDBOOK.md`
7. `CODEX_MASTER_PROMPT.md`

P2 Codex reading order:
1. `CODEX_P2_MASTER_PROMPT.md`
2. `docs/p2/INDEX.md`
3. `docs/p2/00_EXECUTION_RULES.md`
4. `docs/p2/02_BATCH_PLAN.md`
5. Open the current batch document listed in `docs/p2/INDEX.md`.

P2 DB build reading order:
1. `CODEX_P2_MASTER_PROMPT.md`
2. `docs/p2/db/INDEX.md`
3. `docs/p2/db/02_DATABASE_URL_CONTRACT.md`
4. `docs/p2/db/08_B02_DATABASE_BUILD_RUNBOOK.md`
5. `docs/p2/db/09_TROUBLESHOOTING_DATABASE_URL_EMPTY.md`
6. `docs/p2/db/10_ACCEPTANCE_MATRIX.md`
7. `docs/p2/11_DATABASE_SETUP.md`
8. `docs/p2/15_DB_TEST_MIGRATOR_POLICY.md`

## 已锁定的差异化范围

- Generic Percentile、D&D SRD 5.2.1、COC/商业规则适配器与合法规则包机制。
- 场景板、正方形网格、六边形网格首发。
- Creator Agent 可选插图插件，默认关闭、人工审批。
- 战斗、回合、角色卡使用乐观锁 + 死锁检测；CRDT 仅笔记与线索布局。

## 用 Codex 启动

把本目录复制到空仓库根目录，然后将 `CODEX_MASTER_PROMPT.md` 的内容作为第一条 Codex 任务。Codex 必须先读取 `AGENTS.md` 和 `DECISIONS.md`。

## 推荐首轮目标

首轮只创建：

- Rust workspace 与 Next.js 应用
- 配置加载与数据库 migration
- Auth/权限骨架
- Health/Ready/Metrics
- JSON Schema 和基础合同骨架；OpenAPI 与 WebSocket 合同进入 Foundation 阶段
- Mock Provider、Mock Image Provider
- Docker Compose
- 关键测试骨架

不要在第一轮导入任何版权不明确的规则或模组正文。

## 开发工具链

SQLx CLI 必须与 workspace 中 `sqlx` 版本一致：

```bash
cargo install sqlx-cli --version 0.9.0 --no-default-features --features native-tls,postgres --locked
```

## Release source package

生成干净源码包：

```bash
bash scripts/package_source.sh
```

输出：`dist/trpg-platform-source.tar.gz`。脚本会排除 `.git/`、`node_modules/`、`target/`、`.next/`、`dist/`、`coverage/`、`*.tsbuildinfo`、`.env` 和临时文件；当前 P1 源码包应小于 5 MB。

## License

Project license: AGPL-3.0-or-later.

See LICENSE.

Documentation and bundled example data follow the same license unless a file explicitly states otherwise.
