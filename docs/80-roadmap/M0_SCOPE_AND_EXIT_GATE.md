---
document_id: SPEC-M0-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# M0 仓库重启范围与出口门禁

<a id="SPEC-M0-PURPOSE"></a>
## 1. 目的

在原仓库保留完整历史并冻结旧默认分支，随后在 `restart/v1-baseline` 中从零建立新主线治理和空骨架。

<a id="SPEC-M0-ALLOWED"></a>
## 2. 允许

- 旧项目冻结证据和远端签名注释 Tag；
- 将当前默认分支规范化为受保护 `main`；
- 清空活动树旧代码、测试、迁移、设计、提示词和素材；
- 新许可、权威文档、机器决策、追踪和法律草案状态；
- 单根 Go Module、Web/Studio/lua-runner/process 骨架；
- projectctl、Justfile、开发 Compose、跨平台 CI；
- `.codex/` 渐进式披露和自主计划机制；
- GitHub Legacy 标签/Milestone/Issue 处理。

<a id="SPEC-M0-FORBIDDEN"></a>
## 3. 禁止

账户、房间、邀请、SessionActor、Lua VM、Host Callback、业务数据库 Schema、包加载、AI 模型、官方游戏、Studio 编辑功能、正式备份和生产部署。

<a id="SPEC-M0-DESTRUCTIVE-GATE"></a>
## 4. 清理前门禁

工作树干净；本地 HEAD 与远端默认分支一致；冻结记录完整；签名注释 Tag 本地验证、推送、从远端重新读取并验证；旧许可实际状态记录。签名或远端验证失败即停止。

<a id="SPEC-M0-BRANCH-RESOLUTION"></a>
## 5. 当前仓库分支解析

公开仓库当前观测默认分支为 `master`，而治理基线要求受保护 `main`。M0 应在冻结 `master` 后通过 GitHub 受支持的分支重命名或等价无历史重写流程建立 `main`，再创建 `restart/v1-baseline`。执行时以 `gh repo view --json defaultBranchRef` 为事实来源。

<a id="SPEC-M0-EXIT"></a>
## 6. 出口门禁

- 远端签名 Tag 可验证；
- 从 Tag 可完整检出旧项目；
- 新活动树无旧实现和无 legacy 树；
- 新许可边界明确且法律草案不可执行；
- 455 项决策机器登记有效，生成索引无漂移；
- 根 README 和权威阅读顺序准确；
- Go/Web/Studio/lua-runner/Compose 空骨架可构建；
- Linux/Windows/macOS M0 CI 按矩阵通过；
- Just 与 projectctl 为统一入口；
- Codex 施工、验收、修复分离并渐进披露；
- 未实现任何业务功能；
- 独立验收结果 PASS。
