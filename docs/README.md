# 文档中心

本目录是 COC AI TRPG Platform 的项目文档入口。面向开发者、运维人员和审计人员的长期文档统一放在 `docs/`；根目录只保留项目入口、Codex 自动发现入口和生成清单。

## 从这里开始

- [当前顶层设计](top-level-design/CURRENT_TOP_LEVEL_DESIGN.md)：产品范围、核心架构与不可突破的系统边界。
- [施工与代理约束](../AGENTS.md)：Codex 执行顺序、输出边界和验收纪律。
- [V1 验收定义](acceptance/V1_ACCEPTANCE_EVIDENCE_MATRIX.md)：17 项权威产品验收条目及测试命令。
- [文档与审计边界](governance/DOCUMENT_ORGANIZATION_AND_AUDIT_BOUNDARY.md)：当前资料、执行资产和历史 provenance 的分区规则。

## 目录导航

| 目录 | 内容 | 主要读者 |
| --- | --- | --- |
| `top-level-design/` | 当前产品与架构最高基线 | 所有人 |
| `architecture/` | 专题架构和网络拓扑 | 架构、平台、安全工程师 |
| `planning/` | 总体施工、阶段、目录、测试和 CI/CD 规划 | 工程负责人 |
| `construction/` | Codex 启动、实施、验收、发布指南和操作手册 | 开发者、Codex 操作员 |
| `acceptance/` | 权威验收定义 | 测试、发布负责人 |
| `governance/` | 文档组织、路径与引用校验、治理决策 | 维护者、审计人员 |
| `audit/` | 各阶段和修复批次的审计记录 | 审计、评审人员 |
| `reports/` | 阶段测试、验收与历史交付报告 | 测试、发布负责人 |
| `codex/` | 规范化后的施工语料、模块约束与追踪映射 | Codex、工程负责人 |

## 仓库根与专用资产目录

以下内容有明确的工具发现、执行或证据语义，因此不作为普通项目文档迁入 `docs/`：

- `README.md`：项目首页。
- `AGENTS.md`：Codex 自动发现的根级施工约束。
- `MANIFEST.md`：由仓库脚本生成的源文件清单。
- `stages/`、`batches/`、`batch-prompts/`、`codex-prompts/`、`prompts/`：可执行施工提示词与阶段控制资产。
- `fixtures/`、`test-data/`、`evidence/`：测试输入和机器证据。
- `inventory/`、`manifests/`：生成型审计库存与包清单。
- `source-archive/`：只读历史来源证明，不是当前执行入口。

组件级 `README.md` 可以与对应代码或运行资产同目录保存，以避免脱离实现上下文。

## 文档维护规则

1. 新增的人读项目文档默认写入上述 `docs/` 分类目录，不在仓库根新增说明性 Markdown。
2. 移动权威文档时必须同步更新代码、脚本、提示词和测试中的路径引用，不保留内容重复的兼容副本。
3. `source-archive/**` 中的历史路径只作 provenance，不随当前路径迁移而改写。
4. 报告不得把未运行、未绑定当前提交或缺少证据的检查声明为 `PASS`。
5. 文档与实现冲突时，按 [AGENTS.md](../AGENTS.md) 中的权威顺序处理。
