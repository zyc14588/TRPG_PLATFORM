---
document_id: PLATFORM-CHANGE-APPROVAL-R2-INT-010-REM-001
schema_version: 1
document_kind: owner-change-approval
authority: governance-policy
status: ACTIVE
source_commit: "a767115af691b728c60bd272dce3fc01f2258bc7"
source_tree: "758f4b241669339e34948c9a4b7f3b318246fd43"
owner_decision_id: OWNER-DECISION-PLATFORM-CHANGE-R2-INT-010-REM-001
proposal_id: PLATFORM-CHANGE-R2-INT-010-REM-001
proposal_document_id: PLATFORM-CHANGE-PROPOSAL-R2-INT-010-REM-001
decision: APPROVED_WITH_CONDITIONS
approval_status: APPROVED_FOR_PLAN
approved_at: '2026-08-13'
approved_timezone: Australia/Brisbane
wip_handoff: SERIAL_WIP_HANDOFF
wip_limit: 1
retained_batch: M1-B002
authorized_batch: M1-B010
creator_editor_type: SCHEMA_VALIDATED_GENERIC_JSON_EDITOR
capability_status: NOT_IMPLEMENTED
integration_status: NOT_AUTHORIZED
---

# PLATFORM-CHANGE-R2-INT-010-REM-001 项目所有者批准登记

<a id="PLATFORM-CHANGE-APPROVAL-DECISION"></a>
## 1. 裁决与效力

| 字段 | 登记值 |
|---|---|
| `owner_decision_id` | `OWNER-DECISION-PLATFORM-CHANGE-R2-INT-010-REM-001` |
| `proposal_id` | `PLATFORM-CHANGE-R2-INT-010-REM-001` |
| `proposal_document_id` | `PLATFORM-CHANGE-PROPOSAL-R2-INT-010-REM-001` |
| `decision` | `APPROVED_WITH_CONDITIONS` |
| `approval_status` | `APPROVED_FOR_PLAN` |
| `approved_at` | `2026-08-13` |
| `approved_timezone` | `Australia/Brisbane` |

本批准只允许在独立治理 `ACCEPT` 通过后创建并冻结 `M1-B010` PLAN。它不授权 IMPLEMENT，不证明平台或 Creator 已具有相关能力，不改变 `R2-INT-010=FAIL`，也不构成任何 rules-residue 证据或集成授权。

<a id="PLATFORM-CHANGE-APPROVAL-SCOPE"></a>
## 2. 批准范围与禁止范围

批准范围：

- 一个独立的 `M1-B010`，目标限于通用 Game Package namespaced extension contract、Host canonical preservation、Creator import / inspect / edit / validate / export / re-import，以及 rules-residue 的最小第三方 adapter proof；
- 第一方与第三方使用同一通用扩展路径，不得根据 package ID、官方身份或 rules-residue 身份提供特权；
- 在后续 PLAN 中冻结 manifest extension container、namespace 与 descriptor、required/optional、local schema 与 SHA-256、payload、Host compatibility、canonical model/serialization、安全限制、版本兼容、Creator 边界、Host preservation、迁移、测试、allowed/forbidden paths 与停止条件；
- 平台候选独立验收通过后，才可开展 rules-residue 最小 probe adapter；之后必须绑定精确 platform commit/tree、rules commit/tree 和 Creator identity 做跨仓库验收。

明确禁止：

- 批准或启动 `M2-B002`、`M2-B009`，或恢复 `M1-B002` IMPLEMENT；
- 把 `M1-B010` 塞入 `M1-B002`、修改其业务范围，或让两个批次并行；
- rules-residue 专用平台分支、package-ID 特判、官方游戏特权格式或 rules-residue 专用 Creator UI；
- 完整 Creator Studio、正式装备语义或规则、装备 Lua/runtime、Campaign、Room、数据库、Session、AI 或网络能力；
- 修改已冻结 rules-residue R2 authority，或把 proposal、批准、`NOT_RUN`、fixture 或推测登记为 PASS；
- 在独立验收前集成，在任意仓库 rebase/force push，或修改失败测试、放宽 Schema、删除门禁来制造绿色结果。

<a id="PLATFORM-CHANGE-APPROVAL-WIP"></a>
## 3. 串行 WIP handoff

本裁决使用平台现有 milestone batch 生命周期和以下状态，不建立第二套状态机：

```yaml
wip_handoff: SERIAL_WIP_HANDOFF
wip_limit: 1
parallel_execution: false
M1-B002:
  state: BLOCKED
  frozen_contract: retained
  active_wip: false
  resume_implement: forbidden
M1-B010:
  authorization: sole_authorized_next_platform_batch
  plan_allocation: deferred_to_phase_d
  implement: forbidden_until_plan_frozen_and_accepted
```

`M1-B002` 的机器状态继续为现有合法枚举 `BLOCKED`，冻结合同不变且不得恢复施工。Phase B 不在 `MILESTONE_PLAN.yaml` 分配或冻结 `M1-B010`；Phase D 才能以 `PLAN` 使用机器 schema 的原生 batch state 创建并冻结它。在此之前，`M1-B010` 只是唯一获准进入下一 PLAN 的平台批次，不得被描述为已 IMPLEMENTING。任何时刻活动施工 WIP 不得超过一项。

<a id="PLATFORM-CHANGE-APPROVAL-CREATOR"></a>
## 4. Creator 最低编辑合同

最低编辑形态固定为 `SCHEMA_VALIDATED_GENERIC_JSON_EDITOR`。

当 extension 已显式声明、schema 位于合法本地包路径、schema digest 匹配、schema version 受平台接受，且 payload 满足后续 PLAN 冻结的大小与深度限制时，Creator 必须允许：

1. import；
2. inspect；
3. edit JSON value；
4. validate against declared schema；
5. export；
6. re-import。

Opaque preservation、只读视图、仅 export、外部文本编辑器和直接修改压缩包均不能证明 edit。对于 schema 不可用、schema version 不受支持或 optional unknown extension，Creator 必须进入只读模式并保持 exact 或 semantic lossless preservation，不解释、不删除。对于 unsupported required extension，必须返回 typed failure。

该合同只是通用 extension 编辑面；不得实现 rules-residue 专用装备 UI、正式装备 schema、装备规则或装备 runtime。

<a id="PLATFORM-CHANGE-APPROVAL-ACCEPTANCE"></a>
## 5. 验收要求

- Owner approval candidate 必须先从固定 proposal commit/tree 形成，并由全新只读治理 `ACCEPT` worktree 验证 proposal → approval 追踪、WIP=1、`M1-B002=BLOCKED`、B010 唯一授权、无实现代码、无业务漂移和无伪造能力 PASS；
- 只有治理 ACCEPT=PASS 才可进入 `M1-B010` PLAN；只有 PLAN 独立验收通过才可 IMPLEMENT；
- 平台 IMPLEMENT candidate 必须由全新只读平台 ACCEPT 验证 extension、Host、Creator、determinism、正负矩阵、回归、许可与 scope；失败只能 REPAIR 后形成新 candidate；
- rules-residue adapter 只能在平台 candidate PASS 后施工，并须独立验收；
- 最终跨仓库验收必须绑定 exact platform commit/tree、exact rules commit/tree 与 exact Creator build/identity，实际执行 Creator JSON edit/export/re-import、Host reload、deterministic second run、semantic equality 与安全负向用例；
- 没有真实 Creator edit 证据时 `R2-INT-010` 不得 PASS；所有证据必须保留 `previous_status=FAIL` 和先前证据，不得由 B001/EXT-001 历史证据覆盖；
- 只允许把独立验收通过的 commits 以 `git merge --ff-only` 集成；默认不 push、不建 PR。

<a id="PLATFORM-CHANGE-APPROVAL-BASELINE"></a>
## 6. 有效基线与变更控制来源

```yaml
effective_baseline:
  platform_implementation:
    commit: 02dd02447ed07fe6248108365847dccf4023ca00
    tree: dc15acda453948fc78afdc259fea3e543f9e001c
  approved_proposal:
    commit: a767115af691b728c60bd272dce3fc01f2258bc7
    tree: 758f4b241669339e34948c9a4b7f3b318246fd43
change_control_lineage:
  - PLATFORM-CHANGE-R2-INT-010-REM-001 proposal
  - OWNER-DECISION-PLATFORM-CHANGE-R2-INT-010-REM-001
  - GOV-R2-INT-010-OWNER-APPROVAL independent ACCEPT
  - M1-B010 PLAN and independent ACCEPT
  - platform IMPLEMENT and independent ACCEPT
  - rules-residue minimal adapter IMPLEMENT and independent ACCEPT
  - exact-pair cross-repository ACCEPT
  - R2-INT-010 evidence transition, then ff-only integration
```

若 proposal commit/tree、批准范围、WIP handoff 或禁止范围与本登记不一致，停止并报告 scope/base blocker；不得自行扩大批准。

<a id="PLATFORM-CHANGE-APPROVAL-NONPASS"></a>
## 7. 非 PASS 声明与下一动作

```text
platform_extension_capability: NOT_IMPLEMENTED
creator_edit_roundtrip: NOT_RUN
cross_repository_acceptance: NOT_RUN
R2-INT-010: FAIL
M2-B002_READY: NO
M2-B002: NOT_STARTED
```

精确下一动作：从本次固定 Owner approval candidate 创建全新只读 worktree，执行 `mode=ACCEPT maintenance=GOV-R2-INT-010-OWNER-APPROVAL`。在该验收 PASS 前不得进入 Phase D。
