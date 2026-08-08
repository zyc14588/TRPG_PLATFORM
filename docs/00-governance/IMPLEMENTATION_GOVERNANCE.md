---
document_id: SPEC-IMPLEMENTATION-GOV-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R26
---

# 实施治理、Codex 自主规划与独立验收

<a id="SPEC-IMPLEMENTATION-GOV-MODES"></a>
## 1. 四种模式

- `PLAN`：只规划未开始路线，不修改业务代码。
- `IMPLEMENT`：按冻结的真实 milestone batch 契约施工，不接受 governance-maintenance target。
- `ACCEPT`：对真实 milestone batch 或 governance-maintenance contract 独立只读严格验收。
- `REPAIR`：对真实 milestone batch 的验收问题，或 governance-maintenance contract 的 source blocker 做最小修复。

只有 PLAN 可以修改 `MILESTONE_PLAN.yaml`。

<a id="SPEC-IMPLEMENTATION-GOV-PROGRESSIVE"></a>
## 2. 渐进式披露

Codex 从单一 `SESSION_START.md` 进入，通过 `projectctl codex route` 获取章节级阅读集合。禁止“读取整个 docs”“理解全部历史后施工”。Context ratio gate 按 executor/profile 决定：Codex 只记录 telemetry；`opencode-deepseek-v4-flash` 使用显式容量并执行 soft 0.55/hard 0.70；未知 profile fail closed。Section ID、禁止批量读取、binding 与 stale route 门禁对所有 profile 强制。

<a id="SPEC-IMPLEMENTATION-GOV-AUTONOMY"></a>
## 3. 自主权

Codex 可以自主决定工程施工顺序、批次拆分/合并和实现策略，但不能改变产品路线、V1、架构、许可、公共契约或发布门禁。

<a id="SPEC-IMPLEMENTATION-GOV-BATCH"></a>
## 4. 批次契约

每个批次必须含：目标、非目标、关联要求、前置条件、阅读图、允许/禁止目录、不变量、实施步骤、测试命令、交付物、风险和停止条件。

<a id="SPEC-IMPLEMENTATION-GOV-BRANCH"></a>
## 5. 分支和提交

每个批次使用短生命周期分支。提交按可审查逻辑组织并签名；验收通过后才能合并受保护 main。修复提交必须引用原 `ACC-*`。

<a id="SPEC-IMPLEMENTATION-GOV-ACCEPT"></a>
## 6. 验收顺序

批次契约 → 需求/规范 → 机器契约 → 基线和候选 Commit → Diff → 自动证据 → 恢复/安全证据 → 最后 Handoff。Handoff 不构成完成证明。

<a id="SPEC-IMPLEMENTATION-GOV-EVIDENCE"></a>
## 7. 证据

大型日志、覆盖率、E2E 录像、扫描、SBOM、恢复演练和性能报告存为 CI Artifact。仓库只保存稳定摘要、Run ID、Artifact 名称/哈希和问题 ID。

<a id="SPEC-IMPLEMENTATION-GOV-MAINTENANCE"></a>
## 8. Governance maintenance contract

治理控制面维护使用可验证的 `GOV-*` ID 和 tracked machine contract，不占用 milestone batch。ACCEPT/REPAIR 可选择该 target；PLAN/IMPLEMENT 不可选择。契约必须阻止产品功能、M1 plan 和 batch 分配。只有控制面自路由死锁且项目所有者明确授权时可一次性外部 Bootstrap；正式 route 自证成功后授权立即退休。
