---
document_id: SPEC-HOST-CALLBACK-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# Lua Host API、反向调用与数据库能力

<a id="SPEC-HOST-CALLBACK-ENTRYPOINTS"></a>
## 1. 标准入口

平台定义 `on_session_create`、`on_session_restore`、`on_session_start`、`validate_command`、`execute_command`、`list_legal_actions`、`project_view`、`create_checkpoint`、`restore_checkpoint`、`resume_continuation`、`on_safe_migration_boundary`、`on_session_end` 和 `cleanup` 等版本化入口。条件能力缺少对应入口时认证失败。

<a id="SPEC-HOST-CALLBACK-VERSION"></a>
## 2. Host API 版本

包声明主版本和允许次版本范围。主版本不兼容、服务器次版本不在范围、或缺少必需能力时拒绝加载，不允许捕获错误后“尽量继续”。

<a id="SPEC-HOST-CALLBACK-CAPABILITY"></a>
## 3. 能力类别

`host.state`、`host.event`、`host.random`、`host.time`、`host.content`、`host.db`、`host.task`、`host.ai`、`host.rules`、`host.log`。每次调用绑定 Execution Token、Session、包模块来源和能力清单。

<a id="SPEC-HOST-CALLBACK-STATE"></a>
## 4. 状态代理

Lua 通过路径化、模式化代理读写 `MutationWorkspace`，不能获得 Go 内存地址。每项变化记录路径、前后摘要、脚本位置、命令和事件关联。

<a id="SPEC-HOST-CALLBACK-DB-PRIVATE"></a>
## 5. 私人包数据库

只允许命名空间型 `get/put/delete/list/compare_and_set`，Schema、索引、行数和字节预算预先声明。不得任意 SQL、跨包、跨工作区或访问平台核心表。

<a id="SPEC-HOST-CALLBACK-DB-TRUSTED"></a>
## 6. 官方与受信包

可安装包命名空间内的关系表、迁移和命名查询/更新操作。Lua 只调用操作 ID；Go 执行、校验输入输出并审计影响表。运行期 DDL 永久禁止。

<a id="SPEC-HOST-CALLBACK-ATOMIC"></a>
## 7. 原子性

状态补丁、包数据库变更、事件、幂等结果、任务、Continuation 和 Outbox 在同一命令事务中提交。任一失败全部回滚，客户端在提交前看不到结果。

<a id="SPEC-HOST-CALLBACK-ASYNC"></a>
## 8. 外部任务

AI、网络、文件下载和长任务只能通过持久任务异步执行。结果作为新的系统命令进入 SessionActor，不恢复原 Coroutine 栈。

<a id="SPEC-HOST-CALLBACK-AUDIT"></a>
## 9. 审计

记录回调类型、模式、参数/结果摘要、持续时间、预算、状态版本、模块来源和最终事件。生产永不记录原始密钥；AUDIT-2 限时授权；AUDIT-3 仅开发。
