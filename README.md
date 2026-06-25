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
