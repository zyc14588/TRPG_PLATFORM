---
document_id: SPEC-LUA-RUNTIME-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# Lua 5.5 Session VM、事务工作区与恢复

<a id="SPEC-LUA-RUNTIME-PROFILE"></a>
## 1. Platform Lua Profile

标识建议为 `platform-lua-5.5-p1`。只接受 UTF-8 Lua 源码，拒绝预编译 Bytecode。默认允许受限 basic、table、string、utf8、math、coroutine；生产不加载 `io`、`os`、`debug`、原生 package 加载器、C Module 或动态库。

<a id="SPEC-LUA-RUNTIME-VM"></a>
## 2. Session 专属 VM

每个运行中 Session 拥有独立长期 Lua VM。不同 Session 不共享全局变量、模块实例、协程、随机状态或能力句柄。编译后只读代码可按包哈希缓存。Session 结束后销毁 VM。

<a id="SPEC-LUA-RUNTIME-TRANSACTION-WORKSPACE"></a>
## 3. 命令级事务工作区

Lua 对状态、包数据库、事件、任务、Continuation 和 Outbox 的变化先写入 `MutationWorkspace`。Lua 正常结束、模式验证和 Go 权威验证全部通过后统一提交。脚本错误、超时、能力违规或数据库提交失败时全部回滚。

<a id="SPEC-LUA-RUNTIME-HOST-CALLBACK"></a>
## 4. Host Callback

Lua 位于独立进程，通过本地双向 IPC 调用 Go Host API。允许能力包括受控 `state`、`event`、`random`、`time`、`content`、`db`、`task`、`ai`、`rules` 和脱敏日志。Lua 不持有 Go 指针、数据库连接或凭据。

<a id="SPEC-LUA-RUNTIME-DETERMINISM"></a>
## 5. 确定性

权威随机和游戏时间只来自 Host API；关键资源、计分和货币使用整数或固定精度；Map 遍历、序列化和事件顺序稳定。相同包、状态、命令和随机输入必须产生相同事件。模型调用只能异步请求，不能在权威函数内同步等待。

<a id="SPEC-LUA-RUNTIME-GLOBALS"></a>
## 6. 全局状态

Lua 全局只可作缓存和临时运行状态。影响正确性、存档、恢复或重放的事实必须进入权威状态、事件或显式检查点。执行失败后 VM 默认标记污染并在下一命令前重建。

<a id="SPEC-LUA-RUNTIME-CHECKPOINT"></a>
## 7. 检查点

检查点只包含 `nil`、布尔、受限数值、UTF-8 字符串、规范数组和字符串键表。禁止函数、闭包、Coroutine、Userdata、句柄、循环引用、Metatable 行为和能力令牌。检查点绑定 Session 状态版本、包哈希、依赖锁、Lua Profile 和运行时版本。

<a id="SPEC-LUA-RUNTIME-CONTINUATION"></a>
## 8. Continuation

跨命令等待不保存 Coroutine 栈。Lua 创建持久任务和 Continuation Token；`workerd` 完成后以新的系统命令进入 SessionActor，Lua 从标准 `resume_continuation` 入口继续。

<a id="SPEC-LUA-RUNTIME-BUDGET"></a>
## 9. 资源预算和审计

限制 CPU/指令、墙钟、内存、回调次数、递归、状态补丁、数据库行数/字节、事件、任务和输出。超限回滚并重建 VM。生产至少启用 AUDIT-0，标准为 AUDIT-1；AUDIT-3 仅开发环境。
