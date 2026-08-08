---
document_id: SPEC-CREATOR-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# Creator Studio 与创作者工作流规范

<a id="SPEC-CREATOR-POSITIONING"></a>
## 1. 定位

Creator Studio 是 Windows/Linux 离线优先的无代码与混合创作桌面工具，不是正式游玩客户端。Web Player 是唯一正式多人游玩客户端。

<a id="SPEC-CREATOR-SOURCE"></a>
## 2. 唯一事实来源

普通项目目录中的规范化清单、Schema、Lua、内容、UI、素材、本地化、测试和依赖锁是唯一可移植事实来源。Studio 数据库只作可重建缓存和索引。

<a id="SPEC-CREATOR-NOCODE"></a>
## 3. 无代码覆盖

官方两个游戏的大部分席位、状态、命令、事件、阶段、卡牌、场景、线索、NPC、目标、AI 契约和内置 UI 组合可无代码维护。复杂算法、迁移和高级 UI 可使用受限代码扩展。

<a id="SPEC-CREATOR-ROUNDTRIP"></a>
## 4. 往返编辑

支持 Studio、文本编辑器、CLI 和 Git。未知字段不得静默删除；无法安全保留时阻止覆盖。外部修改冲突用语义差异界面处理，不采用最后写入静默覆盖。

<a id="SPEC-CREATOR-PREVIEW"></a>
## 5. 预览与多人测试

Studio 提供隔离本地单人预览。内容、素材和兼容 UI 可热更新；状态模式、核心规则和迁移需重启/显式迁移。正式多人开发测试通过 Web Player 私人开发房间。

<a id="SPEC-CREATOR-TEST"></a>
## 6. 测试

可视化场景、规范化测试 DSL 和 CLI 语义一致，支持固定随机种子、状态夹具、权限视图、无头运行和 CI。

<a id="SPEC-CREATOR-AI"></a>
## 7. AI 助手

可选本地或 BYOK，用于构思、草稿、规则建议、测试生成和修复建议。只访问明确授权项目范围，所有修改显示差异并由用户接受，不能自动发布或读取系统秘密。

<a id="SPEC-CREATOR-ASSETS"></a>
## 8. 素材与本地化

素材记录哈希、来源、权利、尺寸和派生文件。V1 建立稳定文本 ID、语言包和回退，官方内容至少完整简体中文。

<a id="SPEC-CREATOR-BUILD"></a>
## 9. 构建和认证

相同源文件、依赖锁和工具链产生内容等价可校验包。流程为本地验证 → 隔离预览 → 私人测试 → 自动认证 → 必要人工审核 → 签名。
