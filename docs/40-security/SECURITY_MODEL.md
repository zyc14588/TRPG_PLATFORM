---
document_id: SPEC-SECURITY-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 平台安全、权限、秘密与内容隔离模型

<a id="SPEC-SECURITY-NONNEGOTIABLE"></a>
## 1. 不可覆盖边界

身份与席位归属、命令授权、隐藏信息隔离、服务端权威、事件持久化、包沙箱、密钥隔离、租户边界、资源预算和审计不能由游戏包、高级模式或自托管管理员关闭。

<a id="SPEC-SECURITY-VIEWS"></a>
## 2. 最小视图

客户端、UI 扩展和 AI 只接收其席位允许的视图。严禁将全量状态发送后依赖前端隐藏或提示词自律。视图生成和频道成员资格由服务器验证。

<a id="SPEC-SECURITY-SECRETS"></a>
## 3. 秘密

BYOK、数据库、对象存储、Cookie、签名和备份密钥使用专门存储和最小解密路径。游戏包、Lua、UI 扩展、普通日志和导出均不能获得原始秘密。

<a id="SPEC-SECURITY-SANDBOX"></a>
## 4. Lua 沙箱

Lua Runner 独立进程、无公网端口、无数据库凭据；生产不加载 debug/io/os/原生加载器；无文件、网络、进程、环境变量和原始 SQL。Host Callback 按包模块来源、信任级别和 Execution Token 授权。

<a id="SPEC-SECURITY-UI"></a>
## 5. UI 扩展

高级 UI 运行于独立来源或沙箱 iframe，使用 CSP 和类型化 MessageChannel。只能读取席位过滤 View、提交类型化命令；不能控制全局导航、浏览器秘密、数据库、模型或任意网络。

<a id="SPEC-SECURITY-PROMPT-INJECTION"></a>
## 6. 指令注入

平台安全/权限 > 包 AI 契约 > 模组锁定事实 > 房间设置 > 权威状态 > 用户/检索内容。用户、规则文本和检索结果默认是数据，不能通过“忽略规则”等文本提升权限。

<a id="SPEC-SECURITY-RESOURCE"></a>
## 7. 资源治理

Mailbox、Lua、Host Callback、数据库结果、事件、任务、AI 调用和对象导入均有硬上限。超限回滚、暂停或隔离，不允许无界内存、无限重试或压缩炸弹。

<a id="SPEC-SECURITY-AUDIT"></a>
## 8. 审计分离

玩家日志、主持日志、运行日志、安全审计、Lua 回调审计和取证证据分开。普通房主和玩家不得查看系统审计、其他席位秘密或安全证据。
