---
document_id: SPEC-M1-LINUX-ACCEPTANCE-SCOPE
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R26
---

# M1 Linux 验收范围

## 授权与适用范围

项目所有者在当前任务中直接指示：“授权，但本次M1暂时不涉及多平台部分，先完成linux平台内容”。`OWNER-DECISION-M1-LINUX-FIRST-20261006` 据此将本次 M1 交付与验收限定为 Linux amd64。原任务仍为“继续任务直至M1完成，有需要我决定的就停下来问题我”。授权原文、业务基线和 CHANGE 保存在 `/home/zyc14588/.codex/visualizations/2026/10/05/01a10c5c-1d16-7a91-8c2f-1909e2af4f43/m1-linux-first/`；治理实施范围由 `GOV-M1-LINUX-ACCEPTANCE-SCOPE` 契约限定。

该例外适用于本次 M1 当前及后续批次的平台验收执行范围。原机器批次合同、冻结摘要和业务义务保持不变；V1 兼容矩阵、Windows/macOS 产品支持以及 RC/Stable 的完整兼容门禁保留，须在后续获授权的平台工作中完成。本次完成结果必须标为 **M1 Linux 完成**，不能宣称全平台认证或 RC/Stable 发布通过。

## 当前必须项与延期项

所有 Linux 功能、生产 Profile/独立 runner、真实服务与对象存储、原子可见性、取消/故障/重启/重试、完整引用与结果绑定、workspace 授权隔离、安全/隐私/rights、恢复、依赖许可、固定工具链、前端和原生 Creator 门禁继续 required。`just check`、`just test`、`go vet ./...`、`just license-check`、`just ci` 及批次映射测试必须提供精确被测 SHA/tree、实际命令退出和非零用例证据。原生 Linux 本地完整 CI 可按既有验收政策的等价证据要求使用；独立验收仍必须判断证据完整性。

B003 原冻结前 19 行测试映射保留。仅对本次平台执行义务作以下补充裁定；它不修改原合同或原表，不把旧结果改成通过：

| 行 | 本次 M1 Linux 处置 | 执行状态 |
| --- | --- | --- |
| I01、I03—I08、R01、R02、C01、C04 | REQUIRED；由独立 ACCEPT 核验精确 Linux 候选证据 | 以原始证据和新验收回执为准 |
| I02、C02、C03、C05、C06 | DEFERRED_BY_OWNER_M1_LINUX_ONLY；Windows/macOS 原生义务保留 | NOT_RUN；不得记录为 PASS |
| S01 | 原合同中的补充/下游 browser E2E；按实际产品入口和原要求适用性裁定 | 不替代任何 required gate |
| S02、S03 | 原合同中的补充 cross-build | 编译证据不等于 native PASS |

后续 M1 批次同样保留 Linux 上实际适用的全部必需案例，并在验收矩阵中分别列出因本授权延期的 Windows/macOS 案例及 NOT_RUN。平台无关的安全、业务或恢复义务不能因用例名称含平台而整体排除。发现新的业务规范冲突、关键未批准依赖或需要改变公开合同的情况仍须停止提交 CHANGE。

## 历史证据与验收边界

`ACC-M1-B003-003` 的原 `OPEN_REQUIRED_NATIVE_EVIDENCE` 与原总体 FAIL 是旧范围下的事实，必须逐字保留。新 Linux 验收中只能将该平台欠项标为 `DEFERRED_BY_OWNER_M1_LINUX_ONLY`，不得全局 CLOSED；Windows/macOS 仍 NOT_RUN。`ACC-M1-B003-002` 和 `ACC-M1-B003-004` 的 `CLOSED_FOR_VERIFIED_SUCCESSOR` 只绑定已验证的业务 `df1fa6793317cf26660ee92803e63eaf5e7a254a` / tree `5d517c1b949a3e785629bb8563304bded0ed97ac`；原 FAIL 文件不修改。

允许复用的业务证据必须绑定该业务 SHA/tree、未变依赖和原始日志哈希；所有业务文件的内容、模式与路径须经独立比较证明不变。治理候选 SHA/tree 与业务被测身份分列，不能声称旧 CI 直接测试了新增治理 tree。B003 原 frozen digest 保持 `59e0456c8ed261f08b1d1211fbdd1436cc50f4e875f36482732fe84e39d5167e`。

本治理登记不代表 B003 业务 PASS、生命周期完成、main 已接收或 M1 完成。先独立 ACCEPT 该治理变更，再按本授权范围独立 ACCEPT B003；业务 PASS 后才可经正常 PLAN 路由登记合法生命周期转换。后续批次沿 WIP=1 和现有依赖顺序推进，不从本维护契约分配或施工。无远端多平台执行或发布授权从本维护产生。
