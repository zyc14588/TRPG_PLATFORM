---
document_id: RECORD-M0-CI-GITHUB-001
authority: implementation-record
status: ACTIVE
language: zh-Hans
baseline: M0
---

# M0 CI 与 GitHub 治理

## CI 矩阵

`M0 Baseline` 工作流只通过根 Justfile 调用统一 `projectctl` 门禁：

- `M0 / Linux`：Ubuntu 24.04 x64，执行完整 Go、Web、Linux Wails、Compose、文档、许可、范围和测试门禁；
- `M0 / Windows`：Windows Server 2025 x64，执行 Just/projectctl、Go、Web 和 Windows Wails 构建；
- `M0 / macOS`：macOS 15 arm64，执行 Just/projectctl 与 Web 类型、构建和测试门禁。

三个 Job 均上传由相同 Commit 生成的静态 M0 Web 工件。Actions 使用 `tools/toolchain.lock.json` 中登记的完整 Commit SHA；Runner 使用固定 OS 标签，不使用 `*-latest`。

## main 门禁

M0 PR 创建后，`main` Ruleset 必须要求：

1. 通过 Pull Request 合并；
2. Commit 签名有效；
3. `M0 / Linux`、`M0 / Windows`、`M0 / macOS` 三项检查成功；
4. 禁止 force push 与删除分支；
5. 仓库禁用 squash merge，保留逻辑签名 Commit。

本记录描述施工配置，不构成独立验收结论。
