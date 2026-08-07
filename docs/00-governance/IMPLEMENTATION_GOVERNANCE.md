---
document_id: SPEC-IMPLEMENTATION-GOV-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 实施治理、Codex 自主规划与独立验收

<a id="SPEC-IMPLEMENTATION-GOV-MODES"></a>
## 1. 四种模式

- `PLAN`：只规划未开始路线，不修改业务代码。
- `IMPLEMENT`：按冻结批次契约施工。
- `ACCEPT`：独立只读严格验收。
- `REPAIR`：只按验收问题 ID 修复。

只有 PLAN 可以修改 `MILESTONE_PLAN.yaml`。

<a id="SPEC-IMPLEMENTATION-GOV-PROGRESSIVE"></a>
## 2. 渐进式披露

Codex 从单一 `SESSION_START.md` 进入，通过 `projectctl codex route` 获取章节级阅读集合。禁止“读取整个 docs”“理解全部历史后施工”。路由材料软上限为模型上下文 55%，硬上限 70%；超过硬上限必须拆批。

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
