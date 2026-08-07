---
document_id: SPEC-INCIDENT-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 安全事件、强制终止与取证恢复

<a id="SPEC-INCIDENT-SEVERITY"></a>
## 1. 严重度

- `SEC-0`：密钥泄露、沙箱逃逸、数据库越权、跨租户、大规模秘密泄露、供应链权威篡改；
- `SEC-1`：高影响且范围有限的利用或系统性控制失效；
- `SEC-2`：中等风险、可恢复或无明确利用证据；
- `SEC-3`：加固和低风险配置问题。

涉及 R17 明确触发条件时必须提升为 SEC-0。

<a id="SPEC-INCIDENT-TERMINATION"></a>
## 2. SEC-0 自动动作

阻止新安装和 Session → 定位受影响依赖图 → 停止新命令 → 取消在途 Lua/AI/任务 → 回滚未提交工作区 → 冻结最后已提交状态 → 写平台安全终止事件 → 断开连接 → 标记 `SECURITY_TERMINATED` → 建证据保留锁。

不得等待回合/场景结束，也不得调用被撤销包 cleanup 或快照入口。

<a id="SPEC-INCIDENT-EVIDENCE"></a>
## 3. 证据

保留精确包、依赖、签名、认证、密钥链、事件范围、快照、检查点、Host Callback 审计、Mailbox 摘要、任务、服务器/Runner 版本和终止时间。证据加密、内容寻址、不可覆盖并记录访问。

<a id="SPEC-INCIDENT-PRIVACY"></a>
## 4. 证据隐私

不得自动收集 BYOK 原始密钥或向普通管理员导出全部私聊。证据访问和导出需独立授权与审计。

<a id="SPEC-INCIDENT-RECOVERY"></a>
## 5. 审计后处置

原 `SECURITY_TERMINATED` Session 永不复活。可从可信状态创建后继 Session：确认误报、迁移修复包、回退更早恢复点，或永久归档。所有处置关联 incident、审计结论、包锁和迁移报告。
