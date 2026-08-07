---
document_id: SPEC-MIGRATION-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 包、事件、数据与 Session 迁移兼容

<a id="SPEC-MIGRATION-EVENTS"></a>
## 1. 事件历史

已提交事件 ID、原始负载、模式版本和语义不可修改。新版本使用兼容投影、确定性 Upcaster 或新事件版本读取旧历史。

<a id="SPEC-MIGRATION-LOCK"></a>
## 2. Session 包锁

Session 保存所有游戏系统、内容、素材、UI、library 的精确版本、内容哈希、依赖锁、Feature、Lua Profile、Host API 和 Schema 集。运行中不重新解析范围或自动加载补丁。

<a id="SPEC-MIGRATION-SAFE-BOUNDARY"></a>
## 3. 升级边界

只允许 Session 未开始、已结束，或包声明且平台验证的安全边界；必须无在途 Lua Execution、关键 Continuation，并创建恢复点。

<a id="SPEC-MIGRATION-FLOW"></a>
## 4. 流程

停止新旧版本 Session → 完整备份/恢复点 → 锁定新包 → 隔离副本预演 → 迁移命名空间/关系数据、快照和 Lua 检查点 → 验证旧事件回放和不变量 → 原子正式迁移 → 启用新哈希。

<a id="SPEC-MIGRATION-ROLLBACK"></a>
## 5. 失败与降级

迁移失败不得留下部分修改。回退通过恢复升级前数据库、对象清单、旧包、旧依赖锁、快照和检查点完成；不承诺任意版本自动向下迁移。

<a id="SPEC-MIGRATION-DDL"></a>
## 6. DDL

DDL 只在安装或升级迁移 Profile 执行。正常 Session Lua VM 无论信任等级都不能创建/删除表、索引、触发器、函数或数据库扩展。
