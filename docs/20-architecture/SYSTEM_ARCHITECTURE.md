---
document_id: SPEC-SYSTEM-ARCH-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 单节点 Go 游戏服务器总体架构

<a id="SPEC-SYSTEM-ARCH-PRINCIPLES"></a>
## 1. 原则

- 平台核心使用 Go；
- 参考 Due 的组件、路由、中间件和 Actor Mailbox 思想，但不直接依赖 Due；
- 单主机、单权威 `platformd`；
- `workerd` 和 `lua-runner` 为同主机隔离进程，不持有权威写入权；
- 不设计分布式、服务发现、跨节点 Actor、自动主备、滚动零停机或消息集群。

<a id="SPEC-SYSTEM-ARCH-TOPOLOGY"></a>
## 2. 拓扑

```text
Reverse Proxy
└── platformd
    ├── AppContainer / Components
    ├── REST/OpenAPI
    ├── WebSocket Gateway + ConnectionHub
    ├── Identity / Workspace / Room / Campaign
    ├── Static Command Router + Middleware
    ├── Session Scheduler + SessionActors
    ├── Event Store / Projection / Outbox
    ├── Lua Execution Coordinator
    ├── AI Job Coordinator
    └── Audit / Administration

workerd
├── AI model jobs
├── memory compression
├── package validation
├── asset processing
└── export / backup checks

lua-runner
├── per-Session Lua 5.5 VM
├── restricted Host API
└── resource limits

PostgreSQL + Object Storage
```

<a id="SPEC-SYSTEM-ARCH-COMPONENTS"></a>
## 3. 组件生命周期

组件显式声明依赖，按顺序 `Init/Start/Ready`，失败向上传播；关闭时进入维护模式并反向 `Stop/Destroy`。组件不得以全局 Service Locator 隐式访问其他模块。

<a id="SPEC-SYSTEM-ARCH-SINGLETON"></a>
## 4. 单权威实例

启动时取得部署级 Singleton Guard；第二个指向同一部署数据库的 `platformd` 必须拒绝成为权威实例。该锁只防误启动，不实现选主或高可用。

<a id="SPEC-SYSTEM-ARCH-GATEWAY"></a>
## 5. Gateway 与路由

外部仅 HTTPS REST/OpenAPI 和 WebSocket。WebSocket 使用版本化统一信封。路由静态注册并固定执行协议、身份、工作区、房间/席位、幂等、状态版本、限流和预算中间件。游戏包不能注册外部网络路由。

<a id="SPEC-SYSTEM-ARCH-WORKERS"></a>
## 6. Worker 权限

`workerd` 与 `lua-runner` 只能返回结果或建议。它们不能直接提交权威事件、修改 Session 版本或广播最终结果。所有结果通过持久任务/本地 IPC 回到 `platformd`。
