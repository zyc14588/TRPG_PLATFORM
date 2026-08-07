---
document_id: SPEC-CODEX-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# Codex 渐进式披露与自主规划协议

<a id="SPEC-CODEX-ENTRY"></a>
## 1. 单入口与模式

所有会话从 `.codex/SESSION_START.md` 进入，明确 PLAN、IMPLEMENT、ACCEPT 或 REPAIR。模式不明确时只进入 PLAN。

<a id="SPEC-CODEX-ROUTE"></a>
## 2. 路由

`projectctl codex route` 根据模式、里程碑、批次、Commit 和文档哈希生成 `.codex/runtime/` 短期阅读清单。路由只引用稳定 Section ID 和机器契约，不复制规范正文。

<a id="SPEC-CODEX-BUDGET"></a>
## 3. 上下文预算

路由材料软上限为目标模型上下文 55%，硬上限 70%。软超限缩小章节和证据；硬超限停止并在 PLAN 拆批。不得删安全、许可、非目标、停止条件或机器契约。

<a id="SPEC-CODEX-PLAN"></a>
## 4. 自主计划

只有 PLAN 可修改 `MILESTONE_PLAN.yaml`。Codex 可拆分、合并和重排未开始批次，ID 永不复用并保留墓碑。触及 V1、顶层、许可、公共契约或门禁时停止。

<a id="SPEC-CODEX-FREEZE"></a>
## 5. 批次冻结

批次进入 IMPLEMENTING 前冻结目标、要求、范围、阅读图、测试和停止条件。冻结后不能修改契约掩盖越界。

<a id="SPEC-CODEX-ACCEPT"></a>
## 6. 独立验收

ACCEPT 默认只读，客观证据优先，Handoff 最后读取且无验收权威。FAIL 后使用新的 REPAIR 上下文按问题 ID 最小修复。

<a id="SPEC-CODEX-CHECK"></a>
## 7. CI 检查

检查 Front Matter、ID、失效引用、无界阅读、规范复制、提示词膨胀、路由哈希、工作树门禁、生成文件保护和模式写权限。
