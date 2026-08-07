---
document_id: SPEC-SESSION-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# Session Actor、命令、事件与实时同步

<a id="SPEC-SESSION-ACTOR"></a>
## 1. Actor 范围

仅活跃 Game Session 使用 Actor。每个 Session 在 `platformd` 内只有一个逻辑 `SessionActor` 和一个有界 Mailbox。Room Lobby、账户、工作区和普通 Campaign 管理使用事务应用服务。

<a id="SPEC-SESSION-COMMAND"></a>
## 2. 命令信封

命令至少包含 `command_id`、`session_id`、`expected_state_version`、实际身份、`seat_id`、类型化负载和 `correlation_id`。重复命令返回原结果，不重复执行。

<a id="SPEC-SESSION-PIPELINE"></a>
## 3. 处理管线

```text
接收 → 路由/权限/幂等/版本验证
→ Mailbox 串行
→ 创建事务工作区
→ Lua 规则执行与 Host Callback
→ Go 最终验证
→ 原子写 Event + State + DB Mutation + Outbox
→ 更新 Actor 内存状态
→ 生成席位视图
→ ConnectionHub 广播
```

<a id="SPEC-SESSION-PERSISTENCE"></a>
## 4. 权威历史

事件日志是权威变更历史，快照只用于恢复加速。已发布事件不改写；随机结果、接受的 AI 结构化动作和工具结果记录在历史中。状态重放不重新调用模型。

<a id="SPEC-SESSION-BACKPRESSURE"></a>
## 5. 背压

Mailbox 有容量上限。饱和时返回稳定背压错误，不使用无界 Channel。管理命令不能无限插队。Actor Panic、Lua 失败或数据库回滚不得破坏其他 Session。

<a id="SPEC-SESSION-LIFECYCLE"></a>
## 6. 激活与休眠

首次访问从快照和事件恢复；空闲后创建必要检查点并休眠。Actor 可重新激活。Session 结束后关闭 Actor 并销毁专属 Lua VM。

<a id="SPEC-SESSION-REALTIME"></a>
## 7. 实时同步

先提交后广播。客户端重连提供最后事件游标，服务器重新验证席位并补发允许事件或当前投影。任何视图均在发送前按席位过滤。

<a id="SPEC-SESSION-DISCONNECT"></a>
## 8. 断线

需要断线席位决策时默认暂停。AI 接管需包允许、处于安全点并有明确授权。网络波动不得静默移交控制权。
