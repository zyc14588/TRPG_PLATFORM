# COC AI TRPG Platform

一个以 Rust 为核心、面向《克苏鲁的呼唤》第七版（COC 7）的在线跑团平台。它同时支持真人 Keeper（HUMAN_KP）与 AI Keeper（AI_KP），目标不是包装一个聊天窗口，而是提供可回放、可审计、可多人协作的完整游戏运行时。

平台把角色卡、调查、线索、检定、SAN、战斗、追逐、场景与 NPC 状态纳入统一规则和事件系统。AI 只能通过受治理的 Agent 工具链提出行动或裁定，不能直接改写正式状态、伪造骰子或绕过权限与可见性检查。

> 当前发布状态以 [V1 验收定义](docs/acceptance/V1_ACCEPTANCE_EVIDENCE_MATRIX.md) 和绑定具体提交的 CI evidence 为准；本 README 不单独宣告 release readiness。

## 核心能力

- **双 Keeper 模式**：Campaign 创建时锁定 HUMAN_KP 或 AI_KP 权威模式；Authority Contract 不可原地修改，只能通过 fork 创建新世界线。
- **COC 7 完整玩法骨架**：角色创建与审核、调查与核心线索、奖励/惩罚骰、SAN 与疯狂、NPC、基础战斗、追逐、成长和结局流程。
- **可信状态与骰子**：所有正式写入遵循 `Command -> Workflow -> Decision -> Event Store -> Projection`；服务端骰子、裁定和重试都留下可验证记录。
- **多人实时协作**：房间同步、断线恢复、分组调查、私密场景与按角色/玩家隔离的信息流。
- **受治理的 AI**：统一经过 Agent Gateway、Runtime、工具权限门和 Model Provider Adapter；真人 KP 模式下 AI 只能生成待批准草稿。
- **隐私与来源追踪**：Visibility Label 和 Fact Provenance 贯穿 API、Event、Agent Context、RAG、摘要、导出、回放、日志与指标。
- **可替换模型 Provider**：支持云端和本地适配器、本地模型认证及显式跨隐私边界授权；禁止从本地模型静默回退到云端。
- **生产化运行面**：Docker Compose、PostgreSQL/pgvector、Redis、NATS JetStream、MinIO、OpenFGA、OPA、TLS、备份恢复、迁移和审计证据链。

## 架构概览

```text
Web / Admin
    │
API / Realtime ──> Command ──> Workflow ──> Decision ──> Event Store
    │                                                        │
    └──────────── Projection / Cache / Search / Export <─────┘

Agent Job ──> Agent Gateway / Runtime ──> Provider Adapter ──> Model
                    │
                    └── Tool Request ──> 同一条正式 Decision Pipeline
```

Event Store 是正史；Projection、Cache、RAG Index 和 Summary 都是可重建读模型。权限决策由 OpenFGA/OPA 与领域策略共同执行，AI 输出不能绕过这条边界。

## 技术栈

| 层 | 主要技术 |
| --- | --- |
| 服务与领域 | Rust 1.96、Tokio、Axum、SQLx |
| 数据与消息 | PostgreSQL 18、pgvector、Redis、NATS JetStream、MinIO |
| 授权与策略 | OpenFGA、OPA、审计 HMAC、Visibility/Provenance contracts |
| Web | 原生 ES Modules、HTML/CSS、WebSocket 客户端 |
| 交付与测试 | Docker Compose、GitHub Actions、Cargo、Python、Node/pnpm |

## 快速开始

### 1. 准备工具链

仓库固定了主要开发版本：Rust `1.96.0`、Node `24.17.0`、Python `3.14.6` 和 pnpm `11.9.0`。还需要 Docker 与 Docker Compose v2。

```bash
rustc --version
node --version
python3 --version
pnpm --version
docker compose version
```

安装前端依赖并检查工作区：

```bash
pnpm install --frozen-lockfile
cargo check --workspace --all-targets --all-features --locked
pnpm --filter ./apps/web... build
```

### 2. 查看安全初始化参数

生产拓扑需要生成并挂载多组秘密，不能直接用占位环境变量启动。统一入口会创建私有状态、证书和初始账户，启动 Compose，并执行初始化自检：

```bash
bash scripts/bootstrap/bootstrap.sh --help
```

正式运行时必须传入绝对路径的私有状态目录、受限权限的 Provider 凭据文件、精确模型标识和 SHA-256 身份；凭据不得写入命令行、仓库或日志。初始化完成后，脚本会报告本机 HTTPS 入口和私有凭据文件位置。

### 3. 运行测试

常用的本地检查：

```bash
cargo test -p trpg-domain-core --all-features --locked
pnpm --filter ./apps/web... test
python3 scripts/ci/check_product_boundaries.py
```

完整合并门禁由下列入口统一编排；它要求干净工作树、固定工具链、PowerShell、网络访问以及相应的真实后端环境：

```bash
bash scripts/ci/test-all.sh
```

当前 release provider matrix 使用 `deepseek-v4-flash` 执行真实云端 chat 合约，本机 Ollama 与 llama.cpp 只执行 embedding 合约。密钥和本地模型路径通过 CI secret/variable 或仓库外私有文件注入，不进入版本控制。

## 仓库结构

| 路径 | 职责 |
| --- | --- |
| `apps/` | API、Realtime、Agent Worker、Admin、Migration Runner 和 Web 入口 |
| `crates/` | 领域、运行时、规则、数据、AI、平台、安全、测试和扩展 SDK |
| `migrations/` | PostgreSQL 前向迁移与安全角色定义 |
| `policy/` | OpenFGA 模型与 OPA 策略 |
| `config/`、`deploy/` | 运行配置和部署资产 |
| `scripts/` | 初始化、CI、备份恢复、投影重建和运维脚本 |
| `docs/` | 项目设计、架构、规划、施工、治理、验收、审计和报告 |
| `fixtures/`、`test-data/` | 可执行测试输入 |
| `stages/`、`batches/`、`codex-prompts/` | Codex 阶段与施工执行资产 |
| `source-archive/` | 只读历史来源证明，不是当前实现入口 |

完整文档导航见 [docs/README.md](docs/README.md)。Codex 或自动修复代理还必须先读取根目录 [AGENTS.md](AGENTS.md)。

## 不可突破的系统边界

- HUMAN_KP / AI_KP 权威模式创建后不可原地切换。
- 业务层、规则引擎、KP 服务和前端不得直接调用裸 LLM。
- Agent 不得直接写数据库、生成无记录正式骰子或修改 Authority Contract。
- 正式状态只能通过 Decision Pipeline 写入 Event Store。
- 私密事实不得泄露到摘要、RAG、导出、回放、日志或指标。
- 未通过 Level 4 认证的本地模型不能担任 AI Keeper Orchestrator。
- 跨 Provider 或跨隐私边界的回退必须显式配置、提示并审计。

更完整的产品范围和设计理由见 [CURRENT_TOP_LEVEL_DESIGN.md](docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md)。
