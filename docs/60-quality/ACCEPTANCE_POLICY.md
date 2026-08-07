---
document_id: SPEC-ACCEPTANCE-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 严格批次验收政策

<a id="SPEC-ACCEPTANCE-INDEPENDENCE"></a>
## 1. 独立性

施工和验收必须使用独立 Codex 上下文。验收默认只读，不直接修复。失败后创建新的 REPAIR 上下文并再次独立验收。

<a id="SPEC-ACCEPTANCE-ORDER"></a>
## 2. 阅读顺序

批次契约 → 关联要求/规范 → 机器契约 → 基线和候选 Commit → 实际 Diff → 测试/CI → 迁移/恢复/安全证据 → 最后 Handoff。

<a id="SPEC-ACCEPTANCE-RESULT"></a>
## 3. 结果

`PASS`、`FAIL` 或 `BLOCKED`。存在未完成要求、越界修改、必须测试缺失、Required Check 失败、文档/实现不一致、追踪断裂、恢复未验证、安全/许可问题或证据不可复现时必须 FAIL。

<a id="SPEC-ACCEPTANCE-FINDINGS"></a>
## 4. 问题

每个问题具有稳定 `ACC-M?-B???-???`、严重级别、位置、复现、违反要求、预期/实际和修复边界。修复提交引用原 ID，原失败记录不删除。

<a id="SPEC-ACCEPTANCE-EVIDENCE"></a>
## 5. 证据

记录 Commit SHA、Diff 范围、执行命令、退出码、CI Run、Artifact、哈希和环境。施工 Handoff 中的“已通过”不是证据。

<a id="SPEC-ACCEPTANCE-M0"></a>
## 6. M0 特殊验收

M0 必须验证远端签名注释 Tag、实际旧许可状态、零代码迁移、新许可边界、决策登记、渐进式 Codex、可构建空骨架、跨平台 CI 和无业务功能。
