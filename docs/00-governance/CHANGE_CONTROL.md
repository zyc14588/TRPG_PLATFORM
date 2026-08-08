---
document_id: SPEC-CHANGE-CONTROL-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 变更控制与停止施工协议

<a id="SPEC-CHANGE-CONTROL-TRIGGERS"></a>
## 1. 必须停止并提交变更提案的情况

- 改变 R0—R24 的 ACTIVE 决策；
- 增加或删除 V1 功能；
- 修改里程碑出口门禁；
- 修改公开 API、事件语义、包格式或 Host API 主版本；
- 改变 Go/Lua/单节点架构；
- 改变许可、共同所有权或商标政策；
- 降低安全、恢复或验收标准；
- 引入未在许可允许清单中的关键依赖；
- 权威规范或机器契约冲突；
- 当前批次无法在非目标和允许目录内完成。

<a id="SPEC-CHANGE-CONTROL-PROPOSAL"></a>
## 2. 变更提案最小内容

```text
CHANGE-ID
触发原因
受影响决策、要求和规范
拟修改内容
V1 范围影响
安全、许可和迁移影响
工作量替换关系
候选方案
推荐方案
未批准时的安全停止状态
```

<a id="SPEC-CHANGE-CONTROL-NONTRIGGERS"></a>
## 3. 无需用户逐次批准的工程调整

Codex 可在 PLAN 模式自主拆分、合并和重排未开始批次，添加前置批次，或因验收失败插入修复轮次，前提是：

- 同一里程碑；
- 不改 V1 范围和出口门禁；
- 不改公共契约；
- 每个批次保持单一目标；
- 计划变更有版本、原因和墓碑记录。

<a id="SPEC-CHANGE-CONTROL-FROZEN"></a>
## 4. 批次冻结

批次进入 `IMPLEMENTING` 前冻结目标、需求、允许/禁止目录、机器契约、测试和停止条件。冻结后不能通过修改契约来容纳已经越界的实现。

<a id="SPEC-CHANGE-CONTROL-FAIL"></a>
## 5. 验收失败

原批次保留，建立 `M?-B???-R?` 修复轮次。修复仅处理 `ACC-*` 问题 ID，不得混入无关重构或新增功能。
