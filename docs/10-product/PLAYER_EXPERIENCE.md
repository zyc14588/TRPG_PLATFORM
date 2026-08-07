---
document_id: SPEC-PLAYER-EXPERIENCE-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 玩家闭环、房间、席位与安全体验

<a id="SPEC-PLAYER-EXPERIENCE-HOME"></a>
## 1. 首页

首页以“继续游戏、快速开始、创建房间、我的游戏”为主。模型供应商、API Key、Agent 拓扑和创作者调试进入高级页面。

<a id="SPEC-PLAYER-EXPERIENCE-QUICKSTART"></a>
## 2. 快速开始

官方 TRPG 单次模组提供推荐角色、AI 主持、席位和安全默认值。快速开始自动填充配置，但不得绕过包、模型、权限和内容门禁。

<a id="SPEC-PLAYER-EXPERIENCE-ROOM"></a>
## 3. 私人房间流程

选择游戏与内容 → 创建私人房间 → 配置邀请/审批 → 加入者查看并同意包 → 选择或申请席位 → 配置 AI → 确认内容边界 → 准备检查 → 启动同步 Session。

邀请不绑定席位。房间所有者、管理员、主持人和席位控制者分别授权。

<a id="SPEC-PLAYER-EXPERIENCE-VISITOR"></a>
## 4. 账户和访客

房主、创作者和长期 Campaign 成员需要账户。受邀玩家可访客加入单次游戏，后续认领。访客不能拥有 Campaign、上传包、保存长期 BYOK 或获得管理权限。

<a id="SPEC-PLAYER-EXPERIENCE-SAFETY"></a>
## 5. 内容边界与安全工具

- 模组内容标签与强度；
- 参与者私密边界；
- 立即暂停；
- 跳过或弱化内容；
- 离开 Session；
- AI 生成、重试和后备模型继承同一边界。

安全暂停不依赖主持人批准。

<a id="SPEC-PLAYER-EXPERIENCE-CHANNELS"></a>
## 6. 频道

公共、阵营、席位私密、主持沟通和系统日志由服务器验证。无席位管理员不能旁观。普通观察者不属于 V1；角色死亡或桌游淘汰后的原参与者可进入有限公共只读状态。

<a id="SPEC-PLAYER-EXPERIENCE-DISCONNECT"></a>
## 7. 断线与接管

断线时默认暂停需要该席位决策的流程。AI 接管必须在包声明的安全点显式授权。重连恢复席位、事件游标、私有视图和待处理行动，不重复旧命令。

<a id="SPEC-PLAYER-EXPERIENCE-SAVE"></a>
## 8. 保存与导出

每个已接受事件持续保存；回合、场景、Session、主持交接和升级前创建恢复点。导出按权限生成公共、个人、主持或管理备份版本，不泄露其他席位秘密。
