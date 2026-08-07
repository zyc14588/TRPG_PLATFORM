---
document_id: SPEC-QUALITY-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 测试策略、故障注入与 AI 认证

<a id="SPEC-QUALITY-LAYERS"></a>
## 1. 测试层级

Go 单元与集成、Lua 包生产 Profile、TypeScript 组件、协议契约、真实 PostgreSQL/对象存储集成、Web/Studio 跨平台构建和完整 E2E 各自承担职责。

<a id="SPEC-QUALITY-REPLAY"></a>
## 2. 固定事件重放

使用人工设计的版本化固定场景，保存命令、随机输入、期望事件、状态哈希和席位视图。每个状态一致性缺陷必须新增回归场景。不使用生成式完整 Session 状态机探索。

<a id="SPEC-QUALITY-FUZZ"></a>
## 3. Fuzz 与属性测试

持续覆盖 WebSocket 信封、包解压、TOML/Schema、Host Callback 参数、事件反序列化/Upcaster 和迁移入口。完整 Session 命令序列不纳入生成式测试。

<a id="SPEC-QUALITY-REAL-SERVICES"></a>
## 4. 真实服务

事务、迁移、Outbox、原子安装、备份和恢复使用临时隔离 PostgreSQL 与对象存储兼容环境；Mock 只用于单元层。

<a id="SPEC-QUALITY-AI"></a>
## 5. AI

普通 PR 使用确定性模拟模型和录制响应。真实云端/本地模型在手工、定时和认证流水线执行，覆盖 AI 玩家、主持、提示注入、上下文压力、成本、延迟和后备。

<a id="SPEC-QUALITY-FAULT"></a>
## 6. 故障注入

高风险变更按需测试；RC 前必须注入 platformd/workerd/lua-runner 崩溃、数据库提交失败、对象缺失、任务重复、Outbox 重复、Lua 超限、升级中断、备份失败和安全撤销。

<a id="SPEC-QUALITY-PERFORMANCE"></a>
## 7. 单节点性能

在固定参考硬件定义连接数、活跃 Session、Mailbox 延迟、Lua 执行、数据库事务、对象操作和 AI 队列预算。性能目标不得按未实现分布式规模虚构。

<a id="SPEC-QUALITY-FLAKY"></a>
## 8. Flaky

Required Check 不得通过自动重复掩盖失败。Flaky Test 必须登记所有者、影响和期限；安全、权限、数据和发布关键测试保持阻塞。
