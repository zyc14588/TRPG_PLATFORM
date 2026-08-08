<!-- SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0 -->

# TRPG Platform

TRPG Platform 是一个由游戏包和模组驱动的 AI 原生在线桌面游戏平台。V1 目标同时覆盖有主持人的 TRPG 与无固定主持人的桌游，并支持真人与 AI 混合席位。

## 当前状态

仓库处于 **M0 重启基线**：只建立治理、许可、可构建工程空壳、统一工具入口和跨平台 CI。

**当前没有可玩功能。** M0 明确不实现账户、房间、SessionActor、Lua VM、游戏包加载、AI 模型接入、业务数据库 Schema 或官方游戏。

## 权威入口

1. [`docs/00-governance/DOCUMENT_AUTHORITY.md`](docs/00-governance/DOCUMENT_AUTHORITY.md)
2. [`docs/00-governance/V1_SCOPE.md`](docs/00-governance/V1_SCOPE.md)
3. [`docs/10-product/PRODUCT_DEFINITION.md`](docs/10-product/PRODUCT_DEFINITION.md)
4. [`docs/70-decisions/DECISION_REGISTER.yaml`](docs/70-decisions/DECISION_REGISTER.yaml)
5. [`docs/90-traceability/TRACEABILITY.md`](docs/90-traceability/TRACEABILITY.md)
6. [`docs/80-roadmap/M0_SCOPE_AND_EXIT_GATE.md`](docs/80-roadmap/M0_SCOPE_AND_EXIT_GATE.md)

Codex 的渐进式披露入口位于 [`.codex/SESSION_START.md`](.codex/SESSION_START.md)；该目录会在 M0 治理提交中安装并由 `projectctl` 检查。

## 许可与历史边界

新主线程序代码按 [PolyForm Noncommercial 1.0.0](LICENSE) 授权。旧项目冻结在签名注释 Tag `legacy/pre-r0-restart-20260807`；新主线不从旧项目迁入代码、测试、迁移、设计、提示词或素材。边界事实见 [`legal/LICENSE_BOUNDARY.md`](legal/LICENSE_BOUNDARY.md)。
