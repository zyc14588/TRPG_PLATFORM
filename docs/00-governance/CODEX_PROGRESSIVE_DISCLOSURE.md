---
document_id: SPEC-CODEX-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R26
---

# Codex 渐进式披露与自主规划协议

<a id="SPEC-CODEX-ENTRY"></a>
## 1. 单入口与模式

所有会话从 `.codex/SESSION_START.md` 进入，明确 PLAN、IMPLEMENT、ACCEPT 或 REPAIR，不新增第五种模式。模式不明确时只进入 PLAN。REPAIR/ACCEPT 可指向真实 milestone batch 或治理维护契约；IMPLEMENT 只能指向真实 milestone batch。

<a id="SPEC-CODEX-ROUTE"></a>
## 2. 路由

`projectctl codex route` 根据模式、target、executor/profile、Commit 和文档哈希生成 `.codex/runtime/` 短期阅读清单。target 为 milestone、真实 batch 或 `GOV-*` governance maintenance。路由只引用稳定 Section ID 和机器契约，不复制规范正文。

<a id="SPEC-CODEX-BUDGET"></a>
## 3. 上下文预算

Context policy 按 executor/profile 生效。`codex/codex-default` 不启用项目级 soft/hard ratio gate；材料字节数及显式容量存在时的 ratio 只作 telemetry，不因 55%/70% 阈值阻止任何模式。`opencode/opencode-deepseek-v4-flash` 必须显式提供容量，并执行 soft 0.55、hard 0.70：软区间缩减 on-demand Section，不能继续降低时进入 review state；超过 hard 必须停止并缩小 route 或拆 batch。未知 profile 返回 `PROFILE_REQUIRED`。所有 profile 都继续强制 Section ID、禁止批量读取、binding、stale route、schema 和 target 校验。此规则不声明任何模型的物理上下文无限。

`R23-A05` 的 context-overflow 强制拆批仅适用于 enforcement-enabled profile。

<a id="SPEC-CODEX-MAINTENANCE"></a>
## 4. Governance maintenance

Governance control-plane maintenance 不属于 milestone product batch，ID 必须匹配 `GOV-[A-Z0-9-]+`，并由 `.codex/maintenance/<id>/CONTRACT.yaml` 机器契约限定目标、source blocker、允许/禁止范围、规范、验收、测试和停止条件。只有控制面无法为自身生成合法 route 的 bootstrap deadlock 才可使用一次性仓库外 Bootstrap；正式 REPAIR route 通过检查后 Bootstrap 永久失效，同类后续任务必须使用正式 route。Governance maintenance 不得实现产品功能、生成 M1 plan 或分配 M1 batch。

<a id="SPEC-CODEX-PLAN"></a>
## 5. 自主计划

只有 PLAN 可修改 `MILESTONE_PLAN.yaml`。Codex 可拆分、合并和重排未开始批次，ID 永不复用并保留墓碑。触及 V1、顶层、许可、公共契约或门禁时停止。

<a id="SPEC-CODEX-FREEZE"></a>
## 6. 批次冻结

批次进入 IMPLEMENTING 前冻结目标、要求、范围、阅读图、测试和停止条件。冻结后不能修改契约掩盖越界。

<a id="SPEC-CODEX-ACCEPT"></a>
## 7. 独立验收

ACCEPT 默认只读，客观证据优先，Handoff 最后读取且无验收权威。FAIL 后使用新的 REPAIR 上下文按问题 ID 最小修复。

<a id="SPEC-CODEX-CHECK"></a>
## 8. CI 检查

检查 Front Matter、ID、失效引用、无界阅读、规范复制、提示词膨胀、路由哈希、工作树门禁、生成文件保护和模式写权限。
