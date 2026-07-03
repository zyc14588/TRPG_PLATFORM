# 当前顶层设计基线（自包含复制）

> 来源：`coc_ai_trpg_top_level_design.md`
> 清理日期：2026-07-01
> 当前用途：本文件是 v2 自包含施工包的产品与架构最高约束之一。若任何 Codex 源提示词、历史 V4/V5/V6 文档或旧报告与本文件冲突，以本文件与 v2 施工门禁为准。

# COC 首发的 AI / 真人 KP 在线跑团模拟器顶层设计

> 文档类型：顶层设计
> 当前基线：依据本轮对话已确认约束统合，冲突之处以最新约束为准
> 首发方向：COC 7 版完整可玩体验
> 架构方向：规则可扩展、Agent 化 AI 能力、Campaign 级权威锁定、一键部署、本地与云端模型并存

---

## 0. 设计总览

本项目是一个 **以 COC 7 版为首发体验、底层保留多规则扩展能力的在线跑团平台**。它不是单纯的 AI 聊天机器人，而是一个由结构化游戏状态、Agent 子系统、规则引擎、多人实时房间、事件日志、可见性系统和一键部署能力组成的完整跑团系统。

顶层目标：

```text
在线跑团平台
+ COC 规则运行时
+ 真人 KP / AI KP 两种权威模式
+ Campaign 级不可变 Authority Contract
+ Agent 化 AI 能力
+ 多人实时同步
+ 结构化游戏状态机
+ 事件日志、审计、回放、导出
+ 本地 LLM 与云端模型 Provider
+ Docker Compose 一键部署
```

首发重点是 **COC 可完整游玩闭环**，而不是追求所有未来功能一次性落地。

---

## 1. 产品定位

### 1.1 产品定义

本项目定位为：

> 以 COC 为首发体验的可扩展 AI 跑团平台，支持真人 KP 和 AI KP 两种互斥权威模式，支持单人和多人在线游玩。

核心使用场景：

| 场景 | 描述 |
|---|---|
| 单人 AI KP 跑团 | 一名玩家独自游玩，由 AI KP 提供完整主持、规则裁定、NPC、场景和剧情推进 |
| 多人 AI KP 跑团 | 多名玩家在线同桌游玩，由 AI KP 担任最终游戏裁定者 |
| 真人 KP 跑团 | 真人 KP 主持，AI Agents 只作为辅助工具提供建议、规则查询、草稿和复盘 |
| 自建私服跑团 | 通过 Docker Compose 一键部署，适合个人、小社群、朋友团使用 |
| 本地模型隐私跑团 | 使用 Ollama、llama.cpp 或 OpenAI-compatible local endpoint 提供 AI 能力 |

### 1.2 核心不是聊天，而是游戏运行时

系统不应设计成“AI 讲故事 + 聊天记录”，而应设计成：

```text
规则包
+ 角色卡
+ 场景
+ 线索
+ NPC
+ 时间线
+ 状态机
+ 骰子
+ 事件日志
+ Agent 裁定协议
```

AI 负责生成、理解、建议和裁定；但正式游戏状态必须由服务端规则引擎、状态服务和事件日志承载。

---

## 2. 顶层设计原则

### 2.1 总原则

```text
1. 首发以 COC 7 版为完整体验目标。
2. 底层保持 ruleset 可扩展，但首发只开放 COC 单规则房间。
3. Campaign 创建时选择真人 KP 或 AI KP。
4. Campaign 生命周期内 authority_mode 和 authority_owner 不可更改。
5. 真人 KP 模式下，AI 永远只是辅助。
6. AI KP 模式下，人类不能接管或覆盖正式裁定。
7. 权威模式变更只能通过 fork 创建新 Campaign。
8. 所有正式裁定必须事件化、可审计、可导出。
9. 所有 AI 能力必须通过 Agent 子系统提供。
10. Agent 不能绕过 Authority Contract、规则引擎、状态服务、可见性系统和事件日志。
11. 规则引擎负责骰子和数值，KP / AI KP 负责裁定与叙事。
12. 私密信息必须贯穿 Memory、RAG、Summary、Export、Replay、Agent Context 全链路隔离。
13. 一键部署必须包含初始化、健康检查、备份、恢复和升级。
14. 本地模型是一等 Provider，但不是特权执行路径。
15. 版权、隐私、外部模型调用和数据删除必须从第一版纳入设计。
```

### 2.2 业务层不得直接调用 LLM

所有 AI 能力必须经过：

```text
业务请求
  ↓
Agent Gateway
  ↓
Agent Orchestrator / Runtime
  ↓
Model Provider Adapter
  ↓
Cloud / Local LLM
```

禁止：

```text
业务层 → 直接调用 OpenAI / Ollama / llama.cpp
KP 服务 → 直接调用裸 LLM
规则引擎 → 直接调用裸 LLM
前端 → 直接调用模型服务
```

---

## 3. 首发范围与治理机制

### 3.1 V1 目标

V1 目标不是轻量 Demo，而是一个可以完整完成 COC 跑团闭环的首发版本。

V1 必须支持：

```text
部署系统
  ↓
创建管理员
  ↓
配置模型
  ↓
创建 Campaign
  ↓
选择真人 KP / AI KP
  ↓
锁定 Authority Contract
  ↓
选择 COC 规则和房规
  ↓
创建 / 导入模组
  ↓
邀请玩家
  ↓
车卡
  ↓
角色审核
  ↓
开场
  ↓
调查
  ↓
检定
  ↓
线索发现
  ↓
SAN 检定
  ↓
NPC 互动
  ↓
基础战斗 / 基础追逐
  ↓
结局
  ↓
成长结算
  ↓
导出战报
  ↓
复盘 / 下次摘要
```

### 3.2 Scope Control System

为了防止首发范围失控，建立 Scope Control System。

功能分级：

| 等级 | 含义 | V1 处理 |
|---|---|---|
| P0 | 没有它无法完成 COC 游玩闭环 | 必须进入 V1 |
| P1 | 没有它体验明显残缺 | 应进入 V1 |
| P2 | 有价值但可延后 | 默认进入 Backlog |
| P3 | 生态、商业化或远期能力 | 不进入 V1 |

V1 只允许 P0 / P1 功能进入。

新增需求必须通过 Change Control Gate：

```text
1. 是否直接服务 V1 完整 COC 游玩闭环？
2. 是否破坏 Authority Contract / Agent / Visibility / Event Log 架构？
3. 是否需要修改核心数据模型？
4. 是否影响一键部署复杂度？
5. 是否已有验收用例？
6. 是否能被 Tutorial Scenario 或 Golden Scenario 验证？
```

不能被 Tutorial Scenario 或 Golden Scenario 验证的功能，默认不进入 V1。

### 3.3 Explicit Non-goals

V1 明确不做：

```text
图片地图
语音跑团
AI 生成图片
自动生成商业级完整模组
多规则并存房间
公开模组市场
用户间规则包交易
跨服务器联邦
移动原生 App
复杂战棋网格
实时语音转写
自动导入商业 PDF 并重排规则书
```

但需要预留模型空间：

```text
scene_assets
location_graph
relative_position
distance_band
ruleset abstraction
agent_pack abstraction
```

---

## 4. 游玩模式与 Authority Contract

### 4.1 两种游玩模式

Campaign 创建时必须选择一种模式：

```text
HUMAN_KP
AI_KP
```

Campaign 生命周期内不可更改。

### 4.2 真人 KP 模式

```text
真人 KP = 最终游戏裁定者
Agents = Copilot
系统 = 规则校验、状态管理、同步、记录、导出
```

AI Agents 可以提供：

```text
规则查询
检定建议
NPC 台词草稿
场景描写草稿
线索补偿建议
SAN 裁定建议
战斗处理建议
追逐处理建议
模组漏洞检查
复盘摘要
下次预告
```

AI Agents 不能：

```text
直接提交正式裁定
直接 reveal clue
直接 apply damage
直接 apply SAN loss
直接 change scene
直接修改角色卡
直接推进关键剧情
绕过真人 KP 写入正式事件
```

真人 KP 必须确认、编辑或拒绝 Agent 草稿后，系统才会写入正式事件。

### 4.3 AI KP 模式

```text
AI KP = 最终游戏裁定者
人类 = 玩家 / 房主 / 安全管理员 / 旁观者
系统 = 规则执行、状态提交、审计记录
```

人类可以：

```text
行动
提问
申请复议
暂停
举报
管理房间秩序
fork 新 Campaign
```

人类不能：

```text
中途接管 AI KP 裁定权
覆盖 AI KP 正式裁定
手动改判骰子结果
把原 AI KP Campaign 改成真人 KP Campaign
绕过事件日志修改游戏状态
```

### 4.4 Authority Contract

每个 Campaign 创建时生成不可变 Authority Contract：

```text
AuthorityContract
  ├─ authority_mode: HUMAN_KP / AI_KP
  ├─ authority_owner
  │   ├─ HUMAN_KP: user_id
  │   └─ AI_KP: ai_kp_profile_id
  ├─ ruleset_version
  ├─ house_rules_version
  ├─ scenario_version
  ├─ prompt_version
  ├─ agent_pack_version
  ├─ tool_schema_version
  ├─ safety_profile_version
  ├─ ai_provider_snapshot
  ├─ model_route_snapshot
  ├─ created_at
  ├─ locked: true
  └─ change_policy: FORK_ONLY
```

锁定内容包括：

```text
authority_mode
authority_owner
COC 规则包版本
房规版本
模组版本
Prompt 版本
Agent Pack 版本
工具 Schema 版本
安全配置版本
AI Provider / Model Route Snapshot
角色卡模板版本
```

---

## 5. Fork、正史与分支世界线

### 5.1 Fork 原则

由于 Campaign 权威模式不可更改，任何权威模式或权威主体变更都只能通过 fork 实现。

适用场景：

```text
真人 KP 失联
AI KP 服务长期不可用
玩家希望从某一场后改用另一种模式
严重争议后希望重开分支
想体验 What-if 路线
```

流程：

```text
原 Campaign
  ↓
冻结 / 归档 / 保持运行
  ↓
从指定 Session 结束状态 fork
  ↓
新 Campaign 重新选择真人 KP 或 AI KP
  ↓
生成新的 Authority Contract
```

原 Campaign 的事件日志、权威归属和审计历史不被修改。

### 5.2 正史状态

Campaign 需要支持：

```text
canon
non-canon
what-if
emergency-fork
archived
frozen
```

字段建议：

```text
parent_campaign_id
fork_source_session_id
canon_status
fork_reason
created_from_snapshot_hash
```

### 5.3 Fork 复制范围

默认复制：

```text
角色卡当前状态
已公开事件
已发现线索
世界状态
当前 NPC 状态
当前场景状态
```

可选复制：

```text
KP 暗线
隐藏线索
私密消息
AI 内部记忆
真人 KP 私密笔记
```

真人 KP 模式中的 KP 私密笔记不能默认复制给新 KP。

---

## 6. 总体技术架构

### 6.1 顶层架构

```text
Web / Admin / KP / Player UI
        ↓
API Server
        ↓
Realtime Server
        ↓
Campaign Session Service
        ↓
Agent Gateway
        ↓
Agent Orchestrator / Runtime
        ↓
Tool Permission Gate
        ↓
Decision Commit Pipeline
        ↓
Rules Engine / State Service / Event Log
        ↓
PostgreSQL / Redis / pgvector / Object Storage
```

横切系统：

```text
Authority Contract
Visibility Label Propagation
Fact Provenance Layer
Agent Execution Policy
Observability & Evaluation
Safety Profile
Audit Log
```

### 6.2 核心服务

| 服务 | 职责 |
|---|---|
| Web Frontend | 玩家、KP、管理员、开发者调试界面 |
| API Server | REST / RPC 业务接口 |
| Realtime Server | WebSocket 房间同步、断线重连 |
| Agent Worker | 执行 Agent、模型调用、异步任务 |
| Rules Engine | COC 检定、骰子、SAN、战斗、追逐规则 |
| State Service | 提交正式状态变化 |
| Event Log Service | 事件溯源、审计、回放 |
| Memory / RAG Service | 规则、模组、记忆、术语检索 |
| Export Service | 玩家版、KP 版、审计版导出 |
| Admin Service | 模型、部署、权限、日志、健康检查 |

---

## 7. Agent 子系统

### 7.1 Agent 子系统定位

所有 AI 相关能力必须通过 Agent 子系统提供。

```text
LLM ≠ Agent
Agent = LLM + 职责 + 工具权限 + 上下文策略 + 输出协议 + 审计机制
```

Agent 可以：

```text
理解玩家输入
检索规则和模组
生成建议
生成叙事
请求裁定工具
整理记忆
生成导出
执行测试
```

Agent 不能：

```text
直接写数据库
伪造骰子
绕过规则引擎
绕过状态服务
绕过事件日志
泄露 keeper_only / private 内容
修改 Authority Contract
```

### 7.2 Agent 子系统结构

```text
Agent Gateway
  ↓
Agent Orchestrator
  ↓
Agent Runtime
  ↓
Specialized Agents
  ↓
Tool / Capability Registry
  ↓
Rules Engine / RAG / State Service / Event Log / Export / Safety
```

核心模块：

| 模块 | 职责 |
|---|---|
| Agent Gateway | 所有 AI 请求统一入口 |
| Agent Orchestrator | 决定调用哪些 Agent、顺序和策略 |
| Agent Runtime | 执行 Agent、管理模型、工具、上下文 |
| Agent Registry | 注册 Agent 及能力 |
| Tool Registry | 注册可调用工具及 Schema |
| Context Assembler | 组装最小必要且可见的上下文 |
| Model Provider Adapter | 统一云端 / 本地模型调用 |
| Decision Commit Pipeline | 把裁定请求转为正式状态变化 |
| Observability Layer | 记录模型、Prompt、工具、Token、错误 |
| Evaluation Layer | 执行 Golden Scenario 和安全测试 |

### 7.3 COC Agent Pack

首发提供 `coc7_agent_pack`：

```text
coc7_agent_pack/
  ├─ AI Keeper Orchestrator
  ├─ COC Rules Arbiter
  ├─ Investigation Director
  ├─ Clue Manager Agent
  ├─ Sanity Arbiter
  ├─ Madness Narrator
  ├─ NPC Actor Agent
  ├─ Atmosphere Writer
  ├─ Combat Resolver Assistant
  ├─ Chase Resolver Assistant
  ├─ Mythos Lore Agent
  ├─ Character Creation Agent
  ├─ Keeper Copilot Agent
  └─ Session Recap Agent
```

### 7.4 Agent 职责划分

| Agent | 职责 |
|---|---|
| AI Keeper Orchestrator | AI KP 模式下的裁定总控 |
| COC Rules Arbiter | 规则检索、规则解释、检定建议 |
| Investigation Director | 调查节奏、卡关检测、失败补偿 |
| Clue Manager | 线索状态、线索可见性、线索图 |
| Sanity Arbiter | SAN 检定、疯狂状态、心理影响 |
| NPC Actor | NPC 对白、动机、态度、谎言 |
| Atmosphere Writer | 氛围表达，不新增事实 |
| Mythos Lore Agent | 神话知识、禁忌信息、可见性控制 |
| Keeper Copilot | 真人 KP 模式下的辅助 Agent |
| Memory Curator | 整理正式事件和已确认事实 |
| Scenario Validator | 检查模组结构、线索断链和逻辑漏洞 |
| Export Agent | 生成不同权限版本的导出 |

### 7.5 裁定 Agent 与表达 Agent 分离

必须区分：

```text
裁定 Agent：决定发生什么
表达 Agent：负责怎么描述
```

表达 Agent 不能新增事实。例如 Atmosphere Writer 只能根据已经提交的事件生成描述，不能在描写中擅自 reveal clue。

### 7.6 Agent 输出协议

Agent 输出必须结构化，不允许只返回自然语言。

示例：

```json
{
  "agent_id": "coc_ai_keeper_orchestrator",
  "mode": "AI_KP",
  "intent": "investigate_object",
  "player_visible_text": "你蹲下身检查书桌底部，指尖摸到一处不自然的凹槽。",
  "private_messages": [],
  "proposed_checks": [
    {
      "type": "skill_check",
      "skill": "Spot Hidden",
      "difficulty": "normal",
      "reason": "检查书桌暗格"
    }
  ],
  "tool_requests": [
    {
      "tool": "request_skill_check",
      "args": {
        "character_id": "char_001",
        "skill": "Spot Hidden",
        "difficulty": "normal"
      }
    }
  ],
  "state_changes": [],
  "keeper_notes": ["玩家正在接近 clue_012，但尚未正式发现。"],
  "visibility_labels": ["public"],
  "requires_human_confirmation": false
}
```

真人 KP 模式下必须包含：

```json
{
  "requires_human_confirmation": true,
  "draft_only": true
}
```

---

## 8. Agent 治理层

Agent 子系统必须包含四个强制治理层。

### 8.1 Tool Permission Gate

所有 Agent 工具调用默认拒绝，只有通过检查才允许执行。

检查项：

```text
Authority Contract Check
Agent Permission Profile Check
Campaign State Check
Ruleset Compatibility Check
Visibility Check
Tool Schema Version Check
Safety Check
```

模式规则：

```text
真人 KP 模式下，正式状态变更工具默认降级为 draft 工具。
AI KP 模式下，只有 AI Keeper Orchestrator 可以请求正式裁定工具。
表达类 Agent 永远不能调用状态变更工具。
Memory / Summary / Export Agent 永远不能调用裁定工具。
Safety / Moderator Agent 不能调用游戏裁定工具。
```

### 8.2 Fact Provenance Layer

正史只能来自正式事件日志和正式状态变化。

事实状态：

```text
proposed
rumor
player_belief
character_belief
npc_claim
confirmed
keeper_secret
contradicted
invalidated
retconned
```

只有以下来源可以写入 `confirmed`：

```text
正式 GameEvent
正式 DecisionRecord
正式 DiceRoll
正式 CharacterSheetVersion
正式 Clue Reveal Event
真人 KP 确认的事件
AI KP 通过工具提交并成功落库的事件
```

不得进入 confirmed：

```text
Agent 草稿
玩家推理
NPC 台词
氛围描写
未确认摘要
复议中的裁定
未落库工具请求
```

### 8.3 Agent Execution Policy

每次玩家行动先分级，再决定 Agent 调用链。

| 等级 | 用途 | 策略 |
|---|---|---|
| Tier 0 | 查看角色卡、查看公开日志等 | 不调用 LLM |
| Tier 1 | 简单问答、普通描写 | 轻量 Agent |
| Tier 2 | 普通检定、普通线索互动 | 标准裁定链 |
| Tier 3 | SAN、战斗、死亡风险、核心线索、复议 | 完整链路 + 强模型 |
| Tier 4 | 摘要、导出、记忆、评测 | 异步后台 |

预算控制：

```text
每次玩家行动最大 Agent 调用数
每次裁定最大 Token
每轮最大延迟
每个 Campaign 每日 Token 上限
每个房间并发 Agent Run 上限
每个模型超时阈值
失败重试次数
备用模型策略
```

### 8.4 Visibility Label Propagation System

所有数据对象必须携带可见性标签。

标签：

```text
public
party_visible
private_to_player
private_to_group
keeper_only
ai_internal
system_only
spectator_visible
spectator_hidden
```

继承规则：

```text
派生内容的可见性不得高于来源内容。
多个来源合成时，使用最严格可见性。
keeper_only 内容参与生成，则结果默认 keeper_only。
private_to_player 内容参与生成，则结果只能对该玩家或更高权限可见。
ai_internal 内容不得进入玩家导出。
```

适用范围：

```text
Message
GameEvent
DecisionRecord
DiceRoll
Clue
NPC Secret
MemoryFact
RAG Chunk
Summary
Export
Replay
Search Result
Agent Context
Tool Result
Debug Log
Error Log
```

---

## 9. AI Provider 与本地 LLM 支持

### 9.1 Provider 抽象

所有模型调用经过统一 Provider Adapter：

```text
Agent Runtime
  ↓
Model Provider Adapter
  ↓
Cloud / Local LLM Provider
```

Provider 类型：

```text
Cloud Provider
  ├─ OpenAI-compatible Provider
  ├─ OpenRouter Provider
  └─ 其他云端 Provider

Local Provider
  ├─ Ollama Provider
  ├─ llama.cpp Server Provider
  └─ Generic OpenAI-compatible Local Provider
```

统一接口：

```text
list_models()
get_model_info()
health_check()
chat_completion()
responses()
embeddings()
stream()
tool_call()
count_tokens()
capability_probe()
```

### 9.2 AI KP 创建时选择模型

创建 AI KP Campaign 时流程：

```text
选择 KP 模式
  ↓
选择 AI KP
  ↓
选择模型来源
      ├─ 云端模型
      ├─ Ollama 本地模型
      ├─ llama.cpp 本地模型
      └─ OpenAI-compatible Local Endpoint
  ↓
选择具体模型版本
  ↓
能力检测
  ↓
锁定 AI KP Profile / Model Route Snapshot
  ↓
生成 Authority Contract
```

配置项：

```text
provider_type
base_url
model_id
model_tag
model_file
quantization
context_size
embedding_model
tool_call_support
json_schema_support
streaming_support
vision_support
hardware_profile
fallback_policy
timeout_policy
cost_policy
```

### 9.3 Local Model Registry

本地模型登记：

```text
LocalModel
  ├─ provider
  ├─ display_name
  ├─ model_id
  ├─ model_tag
  ├─ model_path
  ├─ file_hash
  ├─ quantization
  ├─ context_size
  ├─ tokenizer_info
  ├─ chat_template
  ├─ license
  ├─ capability_profile
  ├─ hardware_requirement
  ├─ installed_at
  └─ status
```

### 9.4 Local Model Trust Policy

记录本地模型来源和可信状态：

```text
model_name
model_source
license
file_hash
download_url
quantization
created_by
verified_at
allowed_use: personal / internal / commercial / unknown
trust_status
```

不明来源模型不得默认推荐为 AI KP 模型。

### 9.5 Local Model Certification System

本地模型必须认证后才能担任 AI Keeper Orchestrator。

认证测试：

```text
Capability Probe
Golden Scenario Tests
Tool-use Stability Tests
Visibility Leakage Tests
Prompt Injection Tests
COC Rules Mini-Eval
Latency Benchmark
Context Stress Test
```

认证等级：

```text
LOCAL_MODEL_LEVEL_0：不可用
LOCAL_MODEL_LEVEL_1：只能普通聊天 / 氛围描写
LOCAL_MODEL_LEVEL_2：可用于 NPC / 摘要 / 规则辅助
LOCAL_MODEL_LEVEL_3：可稳定 JSON / 工具请求，但不建议最终裁定
LOCAL_MODEL_LEVEL_4：可担任 AI Keeper Orchestrator
```

只有 Level 4 可作为 AI KP 最终裁定模型。

### 9.6 No Silent Cloud Fallback Policy

默认不得从本地模型静默 fallback 到云端模型。

必须明确：

```text
是否允许 fallback
fallback 到哪个模型
哪些 Agent 可以 fallback
fallback 是否提示用户
fallback 是否进入审计日志
是否跨越本地 / 云端隐私边界
```

跨隐私边界的模型路由必须显式配置，并写入 Model Route Snapshot。

### 9.7 Local Inference Scheduler

本地推理必须由调度器管理：

```text
本地模型并发限制
推理队列
模型预热
模型加载 / 卸载策略
显存预算
CPU / GPU / NPU profile
最大上下文限制
每个 Campaign 的本地推理配额
超时策略
降级策略
```

多人在线下，AI Keeper Orchestrator 应串行或低并发；Memory、Export、Evaluation 默认异步。

### 9.8 开发阶段 API Key 测试

开发阶段允许使用 Ollama / llama.cpp 的 OpenAI-compatible endpoint 模拟 API-key 模式：

```text
LOCAL_OPENAI_BASE_URL=http://localhost:11434/v1
LOCAL_OPENAI_API_KEY=ollama

LOCAL_OPENAI_BASE_URL=http://localhost:8080/v1
LOCAL_OPENAI_API_KEY=sk-no-key-required
```

用于测试：

```text
API Key 配置页面
Provider 连接测试
模型列表读取
Agent Gateway 调用链
工具调用协议
流式输出
错误处理
超时重试
审计日志
Fallback 策略
```

生产环境不得把占位 key 当作安全认证。

---

## 10. Trust & Transparency System

### 10.1 目标

用户必须知道：

```text
我的数据去了哪里？
这个 Campaign 使用本地模型还是云端模型？
是否混合路由？
AI 为什么这样裁定？
骰子是否由服务器生成？
我的私聊是否会被其他人看到？
玩家版导出是否会泄露 KP 暗线？
```

### 10.2 AI Data Usage Notice

Campaign 创建和玩家加入时必须展示：

```text
模型来源：本地 / 云端 / 混合
Provider 名称
AI KP / 辅助 Agent 使用模型
哪些数据会进入 AI 上下文
哪些数据可能发送到云端
哪些数据只在本地处理
是否允许 fallback
是否允许私密消息进入 AI Context
```

### 10.3 AI 裁定解释分层

关键 AI KP 裁定生成三层解释：

```text
Public Decision Summary
Private / Keeper Decision Notes
Audit Decision Record
```

玩家可见解释示例：

```text
你需要进行侦查检定，因为你正在寻找隐藏结构。
本次难度为困难，因为房间光线不足且你时间有限。
骰子结果为 37，达到困难成功，因此你发现了暗格。
```

不可公开内容保留在 KP / 审计层。

### 10.4 可验证记录

玩家或权限用户可查看：

```text
骰子记录
裁定记录
复议历史
角色卡变更历史
模型路由摘要
导出权限说明
```

---

## 11. COC Ruleset 与扩展设计

### 11.1 Ruleset-agnostic Core

平台核心只认识抽象概念：

```text
Attribute
Skill
Resource
Status
Check
Roll
Modifier
Effect
Scene
Clue
Entity
Visibility
Event
```

COC 专有字段属于 `coc7_ruleset_pack`。

### 11.2 COC Ruleset Package

```text
coc7/
  ├─ character_schema.json
  ├─ dice_rules.json
  ├─ skill_check_rules.json
  ├─ sanity_rules.json
  ├─ combat_rules.json
  ├─ chase_rules.json
  ├─ status_effects.json
  ├─ keeper_prompts/
  ├─ character_creation_prompts/
  ├─ rag_index_config.json
  ├─ terminology.json
  └─ ui_schema.json
```

### 11.3 首发单规则房间

V1 只允许单个房间使用一个 ruleset：

```text
ruleset_id = coc7
```

未来可以支持 DND、PF2e、FATE 等，但 V1 不支持多规则并存房间。

---

## 12. COC 角色卡与车卡

### 12.1 车卡流程

```text
选择时代
  ↓
选择职业
  ↓
填写基础信息
  ↓
生成 / 填写属性
  ↓
计算派生属性
  ↓
分配职业技能点
  ↓
分配兴趣技能点
  ↓
填写背景锚点
  ↓
合法性检查
  ↓
提交 KP / AI KP 审核
  ↓
锁定初始角色卡版本
```

### 12.2 角色卡内容

COC 角色卡包括：

```text
基础信息：姓名、年龄、职业、时代、出身地、背景故事
属性：STR、CON、SIZ、DEX、APP、INT、POW、EDU
派生属性：HP、MP、SAN、Luck、Damage Bonus、Build、Move
技能：侦查、聆听、图书馆使用、心理学、神秘学、克苏鲁神话、自定义技能
战斗：武器、护甲、闪避、伤害记录
背景锚点：重要人物、信念、珍视之物、重要地点、伤痕与恐惧
状态：临时疯狂、不定性疯狂、恐惧症、狂躁症、伤势、自定义状态
```

### 12.3 角色卡版本化

每次角色变化都记录版本：

```text
初始角色卡
第 1 场后成长
SAN 损失
HP 损失
获得物品
失去物品
疯狂状态
伤势恢复
手动修正
```

记录：

```text
谁改的
为什么改
依据哪个事件
改前是什么
改后是什么
玩家是否可见
```

---

## 13. COC 检定、骰子、SAN、战斗与追逐

### 13.1 检定系统

V1 必须支持：

```text
百分骰
普通成功
困难成功
极难成功
大成功
大失败
奖励骰
惩罚骰
Luck 消耗
Pushed Roll
对抗检定
组合检定
团队检定
暗骰
自动判定
手动裁定记录
```

流程：

```text
玩家描述行动
  ↓
KP / AI KP 判断是否需要检定
  ↓
选择技能、难度、奖励 / 惩罚骰
  ↓
服务端骰子系统掷骰
  ↓
规则引擎计算成功等级
  ↓
KP / AI KP 解释结果
  ↓
状态变化写入事件日志
```

### 13.2 可信骰子系统

所有正式骰子由服务端生成。

禁止：

```text
前端生成正式骰
玩家伪造骰点
AI 编造骰点
KP 后改骰点但无记录
```

骰子记录：

```text
roll_id
roller
roll_type
formula
raw_result
final_result
bonus_dice
penalty_dice
difficulty
visibility
created_at
linked_decision_id
```

### 13.3 SAN 与疯狂系统

V1 必须支持：

```text
SAN 检定
SAN 损失公式
短期疯狂
长期疯狂
疯狂发作
恐惧症
狂躁症
神话知识影响
恢复与治疗
```

SAN 结果影响：

```text
角色状态
叙事风格
玩家可见信息
幻觉内容
NPC 反应
后续检定难度
长期心理影响
```

### 13.4 基础战斗

V1 支持抽象战斗状态，不做复杂战棋地图。

支持：

```text
先攻 / 行动顺序
近战
射击
闪避
反击
伤害
护甲
重伤
昏迷
濒死
急救
医学
怪物攻击
多人战斗同步
```

战斗状态：

```text
CombatState
  ├─ 参战者
  ├─ 距离关系
  ├─ 掩体
  ├─ 光照
  ├─ 武器
  ├─ 当前轮次
  ├─ 当前行动者
  └─ 持续状态
```

### 13.5 基础追逐

V1 支持：

```text
追逐参与者
距离段
障碍
检定
成功接近 / 拉开
失败产生代价
追逐结束条件
```

---

## 14. 调查、线索、场景、NPC 与时间系统

### 14.1 线索系统

线索状态：

```text
hidden
discoverable
discovered
misunderstood
confirmed
expired
```

线索类型：

```text
核心线索
辅助线索
氛围线索
误导线索
私密线索
KP 暗线
神话线索
危险线索
```

核心原则：

```text
核心线索不能因为一次失败永久丢失。
失败只改变代价、风险、时间和暴露程度。
```

### 14.2 Scenario Validator

检查：

```text
是否有开场场景
是否有至少一个结局
核心线索是否可达
核心线索是否至少有两种获取方式
是否存在断链线索
是否存在无法触发场景
是否存在无用途 NPC
是否存在矛盾时间线
是否有安全提示
是否定义推荐人数和时代背景
```

### 14.3 场景系统

场景结构：

```text
Scene
  ├─ 地点名称
  ├─ 时间
  ├─ 氛围描述
  ├─ 可见 NPC
  ├─ 可调查对象
  ├─ 隐藏对象
  ├─ 可触发线索
  ├─ 可触发危险
  ├─ 可转场地点
  └─ KP 私密备注
```

### 14.4 NPC 系统

NPC 至少包含：

```text
公开身份
真实身份
表面动机
真实动机
秘密
态度
恐惧点
谎言
可透露线索
触发条件
与玩家关系
```

### 14.5 时间系统

支持：

```text
Campaign Clock
Scene Time
Countdown Timer
Scheduled Event
Threat Timeline
```

用于处理：

```text
午夜仪式
图书馆闭馆
NPC 行程
伤势恶化
疯狂持续时间
追兵接近
日夜变化
怪物或邪教徒主动行动
```

### 14.6 卡关处理

检测：

```text
连续多轮没有新线索
玩家反复调查同一对象
关键线索迟迟未发现
玩家不知道下一步
```

AI KP 模式自动触发低侵入补偿；真人 KP 模式只给 KP 提醒和建议。

---

## 15. 多人在线、私密信息与分组调查

### 15.1 多人实时能力

支持：

```text
实时房间
WebSocket 同步
断线重连
玩家席位
旁观者
私聊 KP
暗骰
分组调查
多 active_scene
角色当前位置
跨场景影响
```

多人模式不是多人聊天室，而是多角色、多场景、多权限同步系统。

### 15.2 分组调查

COC 常见场景：

```text
A 组在图书馆
B 组在警局
C 玩家单独跟踪 NPC
```

系统应支持：

```text
Campaign 可以有多个 active_scene。
Character 当前绑定某个 scene。
不同 scene 有独立可见信息和时间推进。
跨场景事件可以影响其他场景。
```

### 15.3 玩家知识与角色知识分离

需要区分：

```text
player_known_facts
character_known_facts
party_known_facts
keeper_known_facts
```

玩家看到的信息不一定等于角色知道的信息。AI KP 裁定时必须判断角色是否有理由知道某事实。

---

## 16. 事件日志、复议与裁定记录

### 16.1 事件溯源

所有正式变化写入 GameEvent：

```text
玩家发言
KP 裁定
AI 建议
检定请求
骰子结果
线索发现
SAN 损失
HP 变化
物品变化
NPC 态度变化
场景切换
战斗开始
战斗结束
复议记录
导出记录
```

事件字段：

```text
event_id
campaign_id
session_id
authority_mode
decided_by
proposed_by
visible_to
timestamp
before_state
after_state
reason
rules_reference
source_context_hash
```

### 16.2 复议机制

复议不能删除历史，只能追加事件。

```text
原裁定事件
  ↓
复议请求事件
  ↓
复核事件
  ↓
修正裁定事件
```

AI KP 模式下由 AI KP 复核；真人 KP 模式下由真人 KP 复核。

### 16.3 Ad-hoc Ruling Policy

当规则未覆盖时，KP / AI KP 可做临时裁定。

要求：

```text
标记为 ad_hoc_ruling
写入 DecisionRecord
说明原因
不得自动修改 Campaign House Rules
可复议
可在 Session 后由 KP / 管理员转为房规草案
```

---

## 17. Memory、RAG、Summary 与 Export

### 17.1 长团记忆分层

```text
Raw Event Log
  ↓
Scene Summary
  ↓
Session Summary
  ↓
Campaign Long-term Memory
  ↓
Locked Facts
```

关键事实必须锁定：

```text
谁已经死亡
谁已经疯狂
哪些线索已发现
哪些 NPC 已暴露
哪些 NPC 仍在撒谎
当前仪式倒计时
玩家获得了哪些神话知识
玩家之间有哪些秘密
```

### 17.2 RAG 分域

```text
Rules RAG
Scenario RAG
Memory RAG
Terminology RAG
World Lore RAG
User Upload RAG
```

每个 RAG Chunk 必须带：

```text
source_type
visibility
copyright_status
version
owner
allowed_use
```

### 17.3 导出类型

| 导出类型 | 内容 |
|---|---|
| 玩家版战报 | 只包含玩家已知内容 |
| KP 完整版 | 包含暗线、隐藏线索、NPC 秘密 |
| 单玩家私密版 | 包含该玩家收到的私密信息 |
| 审计版 | 包含骰子、裁定、状态变化 |
| 续团摘要 | 给下次开团使用 |
| Fork 包 | 用于创建新 Campaign |

---

## 18. 模组系统与文件格式

### 18.1 首发策略

首发支持结构化模组创建、导入、编辑、运行。
不承诺一句话生成商业级完整模组。

### 18.2 Stable Scenario File Format

首发定义稳定文件格式：

```text
scenario.yaml
scenario.json
```

结构：

```text
metadata
keeper_truth
scenes
npcs
clues
timeline
threats
endings
safety_notes
assets_placeholder
visibility
```

### 18.3 Tutorial Scenario 与 Golden Scenario

首发提供两个原创、非侵权模组：

```text
Tutorial Scenario：教学模组，短流程，测试车卡、调查、检定、SAN、线索、结局。
Golden Scenario：验收模组，测试 Agent、Visibility、复议、分组调查、暗骰、导出。
```

用途：

```text
开发验收
Agent 测试
一键部署后自检
用户首次体验
Codex 自动测试
```

---

## 19. 权限矩阵、平台管理与安全工具

### 19.1 角色

```text
Server Owner
Campaign Owner
Human KP
AI KP
Moderator
Player
Spectator
Guest
```

### 19.2 游戏裁定权 ≠ 平台管理权

平台管理者可以：

```text
暂停房间
踢人
禁言
处理举报
删除违规内容
备份数据
关闭服务器
```

但不能：

```text
覆盖游戏裁定
改判骰子结果
伪装成 KP 修改游戏历史
```

### 19.3 Room Moderation Policy

支持：

```text
发言频率限制
复议频率限制
私聊权限
旁观者权限
踢人 / 禁言
举报
黑名单
邀请链接过期
房间密码
```

### 19.4 安全工具

支持：

```text
X-card / 暂停按钮
Lines & Veils
恐怖强度设置
成人内容禁用
过度血腥描写限制
玩家个人禁忌主题
私密求助
房主暂停房间
```

安全暂停不是游戏裁定，不应直接改变游戏结果。

---

## 20. 部署、运维与环境边界

### 20.1 一键部署

首发必须支持：

```text
docker compose up -d
```

服务组成：

```text
Web Frontend
API Server
Realtime Server
Agent Worker
PostgreSQL
pgvector
Redis
Object Storage
Reverse Proxy
Admin Console
```

### 20.2 初始化向导

首次启动：

```text
创建管理员
配置模型 API Key
测试模型连接
初始化 COC 规则包
初始化数据库
创建示例模组
测试 WebSocket
测试 RAG
测试骰子系统
```

### 20.3 运维能力

必须包含：

```text
健康检查
备份
恢复
升级
失败回滚
日志查看
数据库迁移检查
RAG 索引检查
模型连接测试
WebSocket 检查
```

### 20.4 Dev / Prod Provider Security Boundary

开发环境允许占位 API key。
生产环境必须检查：

```text
本地模型服务是否未鉴权
base_url 是否暴露在非 localhost
API key 是否是占位符
反向代理是否启用认证
是否配置 IP allowlist
```

---

## 21. 数据、隐私、版权与删除

### 21.1 数据保留与删除

需要设计：

```text
用户数据保留策略
私聊删除策略
Campaign 删除策略
被删除用户的角色记录处理
审计日志保留时间
API Key 加密保存
上传规则书删除
导出后的撤回边界
```

### 21.2 COC Copyright and Scenario Distribution Policy

首发采用保守策略：

```text
平台提供规则包结构、角色卡模板、骰子逻辑和用户自定义导入能力。
不默认内置未授权商业规则书全文。
不默认提供商业模组全文。
```

支持：

```text
用户上传自有资料
管理员导入授权资料
公开授权资料
自写模组
自定义规则包
```

---

## 22. UI 分层

### 22.1 Layered UI Complexity Policy

内部系统复杂，但用户界面要分层。

| UI | 目标 |
|---|---|
| Player UI | 沉浸、简单、低认知负担 |
| KP UI | 可控、完整、支持裁定和管理 |
| Admin UI | 部署、模型、权限、日志、健康检查 |
| Developer UI | Agent 调试、测试、审计、模型评估 |

玩家不应直接面对过多底层概念，例如 Tool Permission Gate、Fact Provenance、Model Route Snapshot。
这些应在 KP / Admin / Developer 层中展示。

---

## 23. 测试、验收与可观测性

### 23.1 Golden Scenario Tests

固定输入、固定状态，测试 Agent 是否符合约束。

测试重点：

```text
是否跳过检定
是否泄露隐藏线索
是否混淆玩家身份
是否忽略骰子结果
是否乱改角色卡
是否能处理 Prompt Injection
是否能保护私密信息
是否能 fail-forward
是否能在卡关时合理补线索
是否能遵守 Keeper Constitution
```

### 23.2 模式差异测试

同一输入在不同模式下输出不同：

```text
真人 KP 模式：Agent 只生成建议，requires_human_confirmation = true。
AI KP 模式：Agent 可以请求正式裁定工具，requires_human_confirmation = false。
```

### 23.3 Agent 观测记录

每次 Agent 调用记录：

```text
agent_run_id
campaign_id
session_id
agent_id
authority_mode
authority_owner
model_provider
model_name
prompt_version
ruleset_version
input_context_hash
tool_calls
output
visibility_labels
token_usage
latency
error
linked_event_ids
```

### 23.4 AI Decision Reproducibility Record

关键 AI 裁定额外记录：

```text
agent_pack_version
prompt_version
model_provider
model_id
model_file_hash
quantization
temperature
seed
context_hash
tool_calls
decision_summary
linked_events
```

不要求完全复现，但必须可审计、可解释、可追踪。

---

## 24. V1 Acceptance Definition

V1 完成标准：

```text
1. Docker Compose 一键部署成功。
2. 可配置云端模型、本地 Ollama、本地 llama.cpp。
3. 可创建 AI KP / 真人 KP Campaign。
4. Authority Contract 不可修改。
5. 可完成 COC 车卡与角色审核。
6. 可运行一个完整原创教学模组。
7. 可完成调查、检定、线索、SAN、NPC、基础战斗、基础追逐。
8. 可多人在线同步和分组调查。
9. 私密信息不泄露到摘要、RAG、导出、回放。
10. AI KP 所有正式裁定都通过工具和事件日志。
11. 真人 KP 模式下 AI 只能生成草稿。
12. 可 fork Campaign。
13. 可导出玩家版、KP 版、审计版战报。
14. Golden Scenario Tests 通过。
15. 本地模型认证机制可运行，未认证模型不能担任 AI Keeper Orchestrator。
16. 不得静默从本地 fallback 到云端。
17. 关键 AI 裁定有玩家可见解释和审计记录。
```

---

## 25. 核心数据实体

建议顶层实体：

```text
User
Campaign
AuthorityContract
CampaignFork
Room
Session
Ruleset
AgentPack
AIKPProfile
ModelProvider
ModelRouteSnapshot
LocalModel
ModelCapabilityProfile
Scenario
Scene
Clue
NPC
Entity
Character
CharacterSheetVersion
GameEvent
DecisionRecord
DiceRoll
SanityEvent
CombatState
ChaseState
MemoryFact
RAGChunk
Summary
PrivateMessage
ExportJob
AuditLog
Permission
SafetyProfile
```

最关键实体：

```text
AuthorityContract：锁定最终裁定权和版本快照
GameEvent：记录所有正式游戏变化
DecisionRecord：记录每次正式裁定
DiceRoll：记录所有服务端骰子
CharacterSheetVersion：记录角色变化历史
MemoryFact：记录事实来源、状态和可见性
ModelRouteSnapshot：记录模型来源、路由、fallback 和隐私边界
```

---

## 26. 最终总结

本项目的最终形态是：

```text
一个以 COC 7 版为首发目标的在线跑团平台。
底层按规则可扩展设计。
AI 能力全部由 Agents 提供。
真人 KP 与 AI KP 是 Campaign 级互斥权威模式。
Campaign 创建时锁定 Authority Contract，生命周期内不可变更。
正式裁定必须通过工具、规则引擎、状态服务和事件日志落地。
本地 Ollama / llama.cpp 与云端模型都是 AI Provider，但都不能绕过 Agent 治理层。
私密信息、事实来源、模型路由、裁定记录和导出权限必须可审计、可解释、可控制。
```

最重要的产品判断：

```text
首发不是做“万能 AI 跑团宇宙”。
首发是交付“可信、可部署、可审计、可完整游玩的 COC 在线跑团平台”。
```
