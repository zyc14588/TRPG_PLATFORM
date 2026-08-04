# Source Bundle Integration Guide — v2.21

## 1. 目标

本指南定义源材料在当前仓库中的规范落位。它保证 Codex、开发者和审计人员不依赖外部附件、旧 zip、聊天上下文或根目录散落文档，就能找到唯一的当前权威来源。

## 2. 当前规范落位

| 内容 | 当前路径 | 使用规则 |
| --- | --- | --- |
| 根级代理约束 | `AGENTS.md` | 必须保留在仓库根，供 Codex 自动发现。 |
| 项目与文档入口 | `README.md`、`docs/README.md` | 分别面向项目读者与文档读者。 |
| 顶层产品设计 | `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md` | 当前产品与架构最高约束。 |
| Codex 启动和操作指南 | `docs/construction/**` | 当前施工入口；操作手册位于 `docs/construction/operator-guides/**`。 |
| 施工与测试规划 | `docs/planning/**` | 人读规划，不作为 concrete code prompt。 |
| 验收定义 | `docs/acceptance/**` | V1 权威验收条目和测试命令。 |
| 文档治理 | `docs/governance/**` | 目录、路径、引用和 provenance 边界。 |
| 规范化施工材料 | `docs/codex/**`、`codex-active-normalized/**` | 执行前必须先应用三个 current-safe map。 |
| 阶段与批次执行资产 | `stages/**`、`batches/**`、`batch-prompts/**`、`codex-prompts/**` | 保留在专用执行目录，不视为普通项目文档。 |
| 持久提示词 | `prompts/persistent/**` | 当前可执行辅助提示词。 |
| 测试与证据资产 | `fixtures/**`、`test-data/**`、`evidence/**` | 测试输入或机器证据，保持原执行路径。 |
| 生成型审计资产 | `inventory/**`、`manifests/**`、`MANIFEST.md` | 由校验流程维护，不手工复制成第二份。 |
| 历史来源 | `source-archive/**` | 只读 provenance；不得作为当前施工入口。 |

## 3. 文档收敛规则

1. 人读项目文档默认写入 `docs/` 的对应分类目录。
2. 根目录不得新增说明性 Markdown；只允许 `README.md`、`AGENTS.md` 和生成型 `MANIFEST.md`。
3. 组件级 README 可以与代码、策略或执行资产同目录保存，以维持上下文邻近性。
4. 文档迁移必须同步更新代码、测试、脚本和 active prompt 中的路径，不保留内容重复的根级兼容副本。
5. `source-archive/**` 中的历史路径保持原样，以免破坏 provenance。

## 4. 当前权威与历史信息

`docs/codex/**` 和 prompt 资产中可能出现 V3/V4/V5/V6、旧报告、旧 SHA 或旧中间路径。这些内容只证明来源，不得覆盖当前顶层设计、规范化映射或验收定义。

```text
当前实现目标 = 顶层设计 + v2.21 阶段方案 + normalized maps + active prompt/batch 约束
历史版本词 = provenance，不是产品范围
历史 fix/report = 审计输入，不是当前验收入口
```

任何 batch、category prompt 或 per-file prompt 执行前，必须依次读取：

```text
docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md
docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md
docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md
```

## 5. 执行流程

1. 读取 `AGENTS.md` 和 `docs/construction/CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`。
2. 读取顶层设计、三个 normalized map、阶段规划和 V1 验收定义。
3. 从 `stages/s00-governance-onboarding/START_PROMPT.md` 进入阶段流程。
4. 根据 batch 引用读取 `codex-prompts/**`，严格区分 primary、supplemental 和 documentation 角色。
5. 每阶段运行测试并生成绑定当前提交的 evidence，再执行 `ACCEPTANCE_PROMPT.md`。
6. 失败时使用对应 `REPAIR_PROMPT.md`，不得通过降低门禁或改写历史记录制造 PASS。

## 6. 禁止事项

- 禁止从 `source-archive/**` 提取当前 Rust module、migration、event、NATS subject、metric 或测试名称。
- 禁止跳过 Authority Contract、Agent Gateway、Visibility、Fact Provenance、Event Store 或服务端骰子边界。
- 禁止把生成型清单或历史报告复制为新的当前权威来源。
- 禁止让本地模型静默回退到云端，或在生产配置中使用占位凭据。
