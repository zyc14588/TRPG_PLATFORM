---
document_id: SPEC-PRODUCT-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 产品定义与平台对象模型

<a id="SPEC-PRODUCT-DEFINITION"></a>
## 1. 一句话定义

一个由游戏包和模组驱动的 AI 原生在线桌面游戏平台。用户可单独与 AI 完成整场游戏，也可邀请真人共同游玩；空缺席位由符合规则的 AI 补足。平台既支持有主持人的 TRPG，也支持无主持人的桌游。

<a id="SPEC-PRODUCT-HIERARCHY"></a>
## 2. 产品层级

1. 可实际游玩的在线游戏平台；
2. 承载不同规则与内容的扩展平台；
3. AI 主持和 AI 玩家系统；
4. 面向高级用户的模型、Agent、长上下文和叙事实验能力。

研究能力不能反向支配普通玩家流程。

<a id="SPEC-PRODUCT-OBJECTS"></a>
## 3. 核心对象

| 对象 | 定义 |
|---|---|
| Game System | 一套游戏规则 |
| Game Package | 使平台能运行某游戏的规则、状态、命令和 UI 契约 |
| Module/Content | 基于系统制作的模组、战役、牌组、场景或扩展 |
| Room | 开局前的成员、内容和席位配置空间 |
| Campaign | 跨 Session 的长期容器 |
| Session | 一次实际运行的同步游戏 |
| Seat | 可由真人或 AI 占据的游戏参与位置 |
| Host Seat | 某些游戏具有的主持、GM、KP 或裁判席 |
| Workspace | 用户、包、房间、Campaign、凭据和审计的租户边界 |

<a id="SPEC-PRODUCT-SEATS"></a>
## 4. AI 是一等参与者

真人玩家、AI 玩家、真人主持和 AI 主持使用同一席位与权限模型。AI 必须根据席位视图、动作空间和工具授权行动，不得作为全知聊天窗口。

<a id="SPEC-PRODUCT-EXTENSIBILITY"></a>
## 5. 跨游戏证明

通用能力必须由两个差异足够大的官方垂直切片验证：开放式规则异常 TRPG 与确定性隐藏信息竞争桌游。仅提供插件接口或演示游戏不能声称完成跨游戏平台。

<a id="SPEC-PRODUCT-CONTENT"></a>
## 6. 内容渠道

V1 包含官方原创内容、私人导入和私人房间传输。不建设公共搜索、市场、评分、交易和社区发布。
