# Document Organization and Audit Boundary — v2.21

## Purpose

本文件是当前仓库的文档与审计分区权威。它区分人读项目文档、可执行施工资产、生成型证据与历史 provenance，防止同一资料在多个目录形成冲突权威。

## Project boundary

项目目标是 COC 7 首发的 AI / 真人 Keeper 在线 TRPG 平台，而不是聊天机器人。当前实现必须维持规则运行时、Authority Contract、事件溯源、Visibility、Fact Provenance、受治理 Agent、多玩家同步、部署和验收证据的完整边界。

## Current execution areas

以下路径是当前 Codex 或自动化流程可以直接执行、解析或用作测试输入的资产：

```text
AGENTS.md
docs/construction/**
docs/acceptance/V1_ACCEPTANCE_EVIDENCE_MATRIX.md
docs/codex/**
codex-active-normalized/**
prompts/persistent/**
stages/**
batches/**
batch-prompts/**
codex-prompts/**
fixtures/**
test-data/**
ci-cd/workflows-extractable/**
```

这些目录中的 Markdown 可能具有可执行 prompt、fixture 包装或 CI 提取语义，因此不按普通说明文档迁移。

## Design and audit areas

以下路径用于人类阅读、设计、追踪、验收或审计：

```text
README.md
docs/README.md
docs/top-level-design/**
docs/architecture/**
docs/planning/**
docs/governance/**
docs/acceptance/**
docs/audit/**
docs/reports/**
inventory/**
manifests/**
MANIFEST.md
```

`inventory/**`、`manifests/**` 和 `MANIFEST.md` 是生成型审计资产；它们保留固定路径供自动化消费，不作为手工维护的项目说明文档。

## Provenance-only areas

以下路径只用于追踪历史输入，不能决定当前名称、门禁、prompt、CI/CD 来源或发布状态：

```text
source-archive/**
```

历史文件中的旧根路径、版本 token 和报告结论保持原样，不因当前文档迁移而改写。

## Canonical token rewrite rule

唯一当前 token rewrite 权威是：

```text
docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md
```

历史 rewrite alias 只能存在于 `source-archive/**`。任何 current module、output、migration、event schema、NATS subject、metric label 或测试名都必须经过 normalized current-safe 映射。

## Canonical CI/CD source rule

唯一当前 workflow Markdown 提取源是：

```text
ci-cd/workflows-extractable/target-*.yml.md
```

历史 `github-actions-*.yml.md` 只能作为 provenance，不能用于生成当前 `.github/workflows/*.yml`。

## Root documentation rule

仓库根只允许以下 Markdown：

```text
README.md
AGENTS.md
MANIFEST.md
```

新的人读项目文档必须进入 `docs/`。组件 README 可以留在对应代码或资产目录；执行 prompt、fixture、evidence 和生成型 inventory/manifest 保持专用路径。

## Review checklist

审计必须确认：

1. 根目录只有三个允许的 Markdown 入口。
2. `README.md` 先介绍产品、架构和启动方式，不把历史修复包当作项目定位。
3. `docs/README.md` 能导航所有长期文档分类。
4. 被迁移文档只有一个当前副本，active 代码、脚本、测试和 prompt 不引用旧根路径。
5. `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md` 是唯一 active token rewrite 表。
6. `source-archive/**` 只作 provenance，未出现在 Current execution areas 中。
7. workflow 提取只使用 `ci-cd/workflows-extractable/target-*.yml.md`。
8. 验收报告不会把未运行或未绑定当前提交的检查写成 PASS。
