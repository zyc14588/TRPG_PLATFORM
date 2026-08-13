---
document_id: PLATFORM-CHANGE-PROPOSAL-R2-INT-010-REM-001
document_kind: change-proposal
authority: proposal-only
status: PROPOSED
approval_status: NOT_APPROVED
source_commit: "02dd02447ed07fe6248108365847dccf4023ca00"
source_tree: "dc15acda453948fc78afdc259fea3e543f9e001c"
---

# R2-INT-010 通用 Game Package 命名空间扩展与 Creator 无损往返提案

## CHANGE-ID

`PLATFORM-CHANGE-R2-INT-010-REM-001`

本文件只是一份待裁决提案，不是批准、冻结批次、实现合同或验收证据。它不得被解释为 `R2-INT-010=PASS`，也不得修改或扩张已冻结的 `M1-B002`。

## 触发原因

固定平台基线 `02dd02447ed07fe6248108365847dccf4023ca00` / `dc15acda453948fc78afdc259fea3e543f9e001c` 的 manifest v1 对未知字段闭合解析，在 Host load 前拒绝第三方 `rules_residue.equipment_rule` 数据；当前 Creator Studio 又没有可执行的通用 import / inspect-edit / export / re-import 路径。

修复需要改变公开 Game Package 清单/模型合同并增加 Creator 实际编辑能力，因此命中 `SPEC-CHANGE-CONTROL-TRIGGERS`。当前 `M1-B002` 已冻结、状态为 `BLOCKED`、`parallel_safe=false`，其允许路径和非目标不包含 package manifest 或 Creator；`SPEC-M1-FORBIDDEN` 也明确禁止 Creator Studio 实际编辑能力。

## 受影响决策、要求和规范

- `SPEC-PACKAGE-001` 的 manifest、Schema、不可变发布物和导入安全边界；
- `schemas/package/**` 与 `internal/package/manifest/**`、`internal/package/model/**` 的公开机器合同；
- Creator 的规范化源项目、CLI 等价、未知字段保留和冲突合同，当前路线归属 `SPEC-V1-ROADMAP-M6`；
- `SPEC-M1-ALLOWED`、`SPEC-M1-FORBIDDEN` 与 M1 requirement/test/traceability 行；若要求在 M1 内交付，必须先由 Change Control 明确修改，不得由实现者暗改；
- 新的通用 extension requirement、正负测试和 traceability 行；不得把 `rules-residue` 或装备语义写入平台规范。

## 拟修改内容

提议为任意合法 Game Package 增加显式、第三方可用的命名空间扩展声明与无损往返合同。具体 TOML 字段名、JSON 文件布局和版本号只能在提案批准后的新 PLAN 中按平台版本策略冻结，但冻结结果必须至少表达：

```text
namespace
contract/schema version
schema reference
schema SHA-256 digest
required or optional
payload reference or equivalent inline data
Host compatibility requirement
```

命名空间使用稳定的反向域名或平台已批准的等价语法，并与声明它的稳定 `package_id` 建立可验证关系。扩展 payload 是数据，不是代码、入口点、宿主路径或能力升级手段。

### Required / optional

- required extension 缺少兼容 handler 时返回 typed load failure；
- optional extension 缺少专用 handler 时保留原有结构和语义，不解释、不丢弃、不提升权限；
- handler 支持状态必须显式、版本化且参与 load 结果，不允许按官方游戏身份隐式放行。

### Host preservation

扩展必须进入 canonical package model，并在 parse、validate、canonicalize、load、内部模型、export/repack、reload 全链保持语义无损。只把原始 manifest 字节旁路保留而让 Host 模型看不见，不满足本提案。

### Creator roundtrip

Creator 必须让第一方和第三方包走同一套 import、inspect、edit、export、re-import 路径，并阻止静默删改未知字段。最终“edit”合同必须由获批 PLAN 从现行 Creator 权威中明确选择并冻结为可执行验收：通用结构化编辑、raw JSON 编辑或 schema-driven 编辑。Opaque read-only preservation 单独不足以证明 edit；在该选择冻结和真实 Creator 执行前，`R2-INT-010` 不得 PASS。

### Canonicalization 与兼容

- canonical serialization、field ordering、package digest 与第二次 roundtrip 必须确定；
- 不含 extensions 的旧 manifest 解析、语义和构建保持不变；
- 不通过全局 `additionalProperties=true` 接受未知字段；
- 不静默改变已冻结 v1 语义。获批 PLAN 必须依据现行版本策略，在“v1 向后兼容可选字段”“minor contract version”或“新 manifest schema version”中作出有权威依据的选择。

## 安全、许可和迁移影响

实现必须 fail closed 拒绝：未声明 payload、重复或非法 namespace、保留空间伪造、Schema digest 不匹配、缺失 Schema、路径穿越、绝对路径、symlink escape、remote/network `$ref`、未批准外部 Schema、核心字段覆盖、payload/深度超限、可执行代码、宿主路径、动态能力升级和非确定性序列化。

该能力不得引入第三方受保护规则或素材，也不得改变仓库许可。旧包无需迁移；若版本策略要求新 manifest schema version，迁移工具和兼容读取必须在后续冻结合同中显式列出，不能自动重写旧源项目。

## V1 范围与工作量替换关系

能力本身符合 V1 的通用 Game Package 与 Creator 方向，但 Creator 实际编辑当前排在 M6，且 M1 明确禁止。若项目所有者要求立即解除 `R2-INT-010`，必须以显式 Change Control 将一个窄的通用 Creator extension 编辑面提前，并相应增加 requirement/test/traceability；不得挤入 `M1-B002` 或以 governance maintenance 伪装产品功能。

工作量只替换/提前通用 extension contract 与最小 Creator roundtrip 基础，不提前官方 TRPG、装备业务语义、Campaign、AI、Room、marketplace 或其他 M6 Creator 功能。

## 候选方案

1. 保持现行路线：平台核心与 Creator 都留到 M6；安全但 `R2-INT-010`、rules-residue `M2-B002/M2-B009/M2-B015` 继续阻塞。
2. 批准窄范围路线变更：在 `M1-B002` 合法结束或由其所有者解除 WIP 后，由新 PLAN 增加独立通用能力批次，完成 package/Host preservation 与最小 Creator edit roundtrip。此方案最快解除跨仓库阻塞，但必须显式修改 M1 范围/追踪，不能复用 B002 契约。
3. 只提前 package extension，Creator 保持 M6：可降低后续风险，但不能完成 `R2-INT-010`，不得登记 PASS。

## 推荐方案与后续合法批次

推荐候选方案 2，但仅在项目所有者正式批准本提案后生效。请求的后续批次 ID 为 `M1-B010`（当前机器计划的下一未分配序号）；其单一目标应为“通用 Game Package namespaced extension + Host preservation + 最小 Creator edit roundtrip”，并在全新 PLAN 中冻结 exact allowed/forbidden paths、版本策略、security limits、Creator edit 形式、测试和回滚。

`M1-B010` 不得在 `M1-B002` 仍占用 WIP 时进入 IMPLEMENT，也不得作为 `GOV-*` maintenance。若项目所有者不批准 M1 范围调整，则合法目标保持 M6，且本 remediation 维持阻塞。

## 最低验收矩阵

正向至少覆盖：旧 manifest、单/多第三方 namespace、required+supported、optional+unknown preserved、Host parse/load/export/reload、确定 canonicalization/build、Creator 无编辑 roundtrip、Creator 合法编辑 roundtrip、Schema digest，以及第一方/第三方同路径。

负向至少覆盖：未声明 payload、duplicate/illegal/reserved namespace、digest 错误、Schema 缺失、路径穿越/绝对路径/symlink escape、remote `$ref`、required 无 handler、核心字段覆盖、payload/深度超限、非确定序列化、Creator/loader 丢失或改写未知字段、任何 `rules-residue` 特判，以及代码/宿主路径声明。

## 集成与回滚

批准后的顺序必须是：平台 PLAN → 平台 IMPLEMENT candidate → 独立平台 ACCEPT → rules-residue 最小 adapter candidate → 独立 rules ACCEPT → 固定 commit/tree 对跨仓库 ACCEPT → 新证据登记 → 两仓库分别 `git merge --ff-only`。任一验收失败均丢弃或修复候选分支，不回写已接受历史证据，不 rebase/force/amend 已验收 candidate。

## 未批准时的安全停止状态

```text
Result: BLOCKED_PLATFORM_WIP
R2-INT-010: FAIL
M2-B002: NOT_STARTED
M2-B002_READY: NO
```

精确下一动作：平台项目所有者审议 `PLATFORM-CHANGE-R2-INT-010-REM-001`，明确批准或拒绝 M1 范围调整及请求的 `M1-B010`；在该裁决前不得创建 extension IMPLEMENT worktree。
