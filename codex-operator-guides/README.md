# Codex 操作员指南目录 — v2.21

## 用途

本目录为人类操作员提供 Codex 启动、阶段施工、验收、测试、CI/CD、发布、证据审计、回滚和严格校验的可复制中文操作手册。

本目录面向人类程序员，提供可复制给 Codex 的中文操作提示词和专项手册。每份手册职责不同，不得互相替代。

1. `00_QUICK_START.md`：只读启动、识别下一阶段。
2. `01_WORKSPACE_AND_REPOSITORY_SETUP.md`：准备 git branch、工具链和 evidence 目录。
3. `02_STAGE_EXECUTION_PLAYBOOK.md`：按 S00-S13 单阶段施工。
4. `03_ACCEPTANCE_AND_REPAIR_PLAYBOOK.md`：执行严格 PASS/FAIL 验收和最小修复。
5. `04_TESTING_PLAYBOOK.md`：执行单元、集成、契约、fixture、Golden Scenario、Docker 和发布测试。
6. `05_CI_CD_SETUP_PLAYBOOK.md`：将 `ci-cd/workflows-extractable/*.md` 提取为 `.github/workflows/*.yml`。
7. `06_RELEASE_PREPARATION_PLAYBOOK.md`：准备 release candidate、rollback、restore 和 V1 evidence。
8. `07_EVIDENCE_AND_AUDIT_PLAYBOOK.md`：记录命令、事件、骰子、visibility、export 和 model-route 证据。
9. `08_TROUBLESHOOTING_AND_ROLLBACK.md`：处理失败、迁移、Docker、provider、restore 和 rollback。
10. `09_CODEX_SESSION_PROMPTS.md`：提供项目级中文会话提示词。
11. `10_STRICT_VALIDATION_COMMANDS.md`：提供外部严格校验命令。

## batch 级手动提示词

`batch-prompts/start/B001.md` 至 `batch-prompts/start/B052.md` 包含 52 个 batch 手动启动提示词，`batch-prompts/accept/B001.md` 至 `batch-prompts/accept/B052.md` 包含 52 个 batch 手动验收提示词。所有可复制 prompt 均以中文指令为主体；路径、命令、workflow name、test name、crate/module 等技术标识符保留英文。
