# TRPG 在线游玩平台总体设计与 Codex 执行文档（最终决策版）

> **状态：最终决策基线（2026-06-25）**  
> 本文档中的产品与技术决策已经完成确认。Codex 不得重新询问这些已决定事项，也不得退回旧的“待确认/未指定”方案。任何实现偏离都必须先写 ADR 并说明迁移与兼容影响。

## 执行摘要

本方案建议把平台做成**权限隔离的多知识库 RAG + 多 Agent 协作系统**：规则书、模组书、公开线索、会话日志、长期记忆分别建库，检索时先做权限过滤，再做混合检索与证据组装，最后交给 Dice Agent、Character Agent、KP Agent、Creator Agent 和 Memory Agent 消费。这样的设计能同时满足“自动判定”“不泄露 KP-only 内容”“可追溯规则依据”和“后续可扩展到多规则系统”的要求。

技术栈上，**Rust 后端 + Axum/Tokio + SQLx + PostgreSQL/pgvector + Next.js App Router**是当前最稳健的默认组合：Axum 适合做类型清晰、可组合的 HTTP 与 WebSocket 服务，SQLx 适合异步、强类型查询与 CI 校验，pgvector 允许把向量与业务数据存放在同一 PostgreSQL 中，Next.js App Router 适合把实时房间页、管理页与服务端渲染页面统一起来。

在模型接入上，平台应统一走**OpenAI-compatible 抽象层**：云端可接 OpenAI API，本地可接 Ollama 或 llama.cpp server。OpenAI 的 Structured Outputs、Ollama 的 Structured Outputs、llama.cpp server 的 schema-constrained JSON 都适合作为 Agent 的结构化输出机制，从而降低“KP 输出无法解析”“骰娘字段缺失”“Creator Agent 漏字段”等工程风险。

合规上，平台不应默认依赖“合理使用”作为自动抓取和全文输出的依据。美国版权局明确指出，合理使用没有固定的字数或百分比安全线，是否成立取决于全部情境；因此工程上更稳妥的策略是：**只接收明确许可的开放规则内容、官方 SRD、ORC/CC 等开放资料，以及用户声明有权使用的本地上传文件**；许可证不明即进入 `pending_review`，且 RAG 结果只返回短证据片段而非大段原文。

## 最终决策基线

以下决策为项目单一真相源；完整表见 [`DECISIONS.md`](../DECISIONS.md)。

- **部署**：同时支持平台云服务与 Docker Compose 自托管；首发单区域，保留 `region_id`。
- **容量**：MVP 目标 20 个活跃房间、单房最多 8 人、200 个并发 WebSocket；压测目标 1000 连接。
- **身份**：Magic Link + OIDC，Passkey 与企业 SSO 后续扩展。
- **隐私路由**：`standard`、`private_hybrid`、`local_only` 三档；故障切换绝不跨越数据隐私边界。
- **规则支持**：内置 `generic_percentile`、`dnd5e_srd_5_2_1`，并提供 COC/其他商业规则的适配器与合法授权规则包机制。项目不得捆绑或自动抓取未获授权的商业正文。
- **地图**：MVP 同时支持无网格场景板、正方形网格、六边形网格（平顶与尖顶）；等距与 3D 延后。
- **Creator 插图**：提供可插拔图像生成/素材提供商，默认关闭、逐图人工审核、独立预算与许可审计；不阻塞纯文字导出。
- **并发控制**：战斗、回合、角色卡采用乐观锁 + 数据库死锁检测/重试；CRDT 仅用于协作笔记与线索布局。
- **媒体**：MVP 使用外部语音链接；Beta 可接托管 LiveKit。TTS 插件化且默认关闭。
- **演出**：关键过场默认 1200 ms；全部动画与音频可关闭或降级。
- **音频**：用户可见七通道，内部映射四条总线。
- **数据保留**：原始模型输入/输出加密保存 30 天；操作审计 180 天；费用聚合 365 天。
- **记忆**：长期记忆必须人工批准，并在战役结束或 180 天后复审。
- **灾备**：MVP RPO 24h / RTO 4h；生产阶段升级至 RPO 15m / RTO 1h。
- **Token 预算**：优先降级到确定性逻辑、本地模型或简短响应，并执行月度硬上限。

## 优先参考资源与总体取舍

本项目适合直接交给 Codex 执行，因为 Codex 的官方文档明确说明它可用于读取、编辑和运行代码，而 `AGENTS.md` 会被 Codex 在开始工作前自动读取。换句话说，这个项目的关键不是写“一段很长的 prompt”，而是把项目边界、测试命令、许可证策略、目录规范、阶段目标固定在仓库里，让 Codex 按仓库约束持续迭代。

建议 Codex 优先查阅下列原始资料，并把它们写入仓库的 `docs/REFERENCES.md`：OpenAI Codex/AGENTS 文档、OpenAI Structured Outputs 与 Embeddings 文档、Axum 官方文档、SQLx 官方文档、pgvector README、Qdrant 过滤与索引文档、Tantivy README、Next.js App Router 文档、React Flow 文档、Ollama API/Structured Outputs/OpenAI Compatibility 文档、llama.cpp server README。它们分别覆盖了**编码代理执行方式、结构化输出、语义检索、HTTP/WebSocket、全文检索、实时 UI 与本地模型兼容层**。

从工程取舍看，推荐默认架构是：

- **后端**：Rust + Axum + Tokio + SQLx。
- **数据库**：PostgreSQL 为主；MVP 可选 SQLite 用于本地离线演示或测试。
- **向量层**：MVP 先做可插拔接口；默认优先 pgvector，同机部署简单；若后续检索规模和过滤复杂度明显上升，再考虑 Qdrant。
- **关键词检索**：Rust 端优先 Tantivy。
- **前端**：Next.js App Router + React Flow。
- **模型网关**：OpenAI-compatible 客户端，同时支持 OpenAI、Ollama、llama.cpp。

下表是建议优先使用的资源与原因：

| 资源 | 应优先查阅的内容 | 对本项目的直接作用 |
|---|---|---|
| OpenAI Codex | `AGENTS.md`、最佳实践、Codex 文档 | 规范 Codex 的仓库内行为与阶段执行方式 |
| OpenAI API | Structured Outputs、Embeddings、API key 安全 | Agent JSON 输出、检索、服务端密钥管理 |
| Axum | extractors、WebSocket、路由 | REST + WS 服务骨架 |
| SQLx | compile-time checked queries、Postgres/SQLite 支持 | 强类型数据库访问、CI 中 `sqlx prepare --check` |
| pgvector | exact/approx search、cosine/L2/IP | 一体化关系数据 + 向量搜索 |
| Qdrant | payload indexes、filtered vector search | 高过滤负载和独立向量库选项 |
| Tantivy | Rust 全文检索库 | 规则书/模组书关键词索引与混合检索 |
| Next.js | App Router | 前端路由、服务端渲染、房间页结构 |
| React Flow | 节点/边可视化 | 线索图、NPC/地点关系图 |
| Ollama | 本地 API、OpenAI compatibility、Structured Outputs、Embeddings | 本地模型、离线演示、家庭实验环境 |
| llama.cpp | OpenAI-compatible server、embeddings routes、schema-constrained JSON | 高可控本地推理服务与结构化响应 |

## 系统总体架构

整体上，这个平台不应是“一个大模型 + 一个向量库”，而应是**按权限和用途拆开的多知识域系统**。至少应拆成：规则书知识库、模组知识库、公开线索知识库、会话日志知识库、长期记忆知识库。规则与模组必须分库或分集合管理；即使实现上共用 pgvector/Qdrant，也要确保其 metadata、payload 与权限过滤字段独立。Qdrant 官方文档强调过滤性能依赖 payload index；PostgreSQL 官方文档则说明，默认没有行级安全策略时，有表访问权限的用户可访问全部符合权限的行，而启用行级安全后若没有策略则是默认拒绝。这意味着**权限约束既要在检索前做应用层过滤，也最好在数据库层再做一层 RLS/策略加固**。

```mermaid
flowchart TB
    subgraph Client["apps/web"]
        UI[房间页 / 角色页 / 规则库页 / 导出页]
        WS[WebSocket Client]
    end

    subgraph Server["crates/server"]
        API[REST API]
        HUB[WS 房间总线]
        AUTH[auth]
    end

    subgraph Core["Rust workspace"]
        GAME[game_core]
        RULES[trpg_rules]
        DICE[dice_engine]
        AGENT[agent_core]
        KP[kp_agent]
        DICEA[dice_agent]
        CHARA[character_agent]
        CREA[creator_agent]
        MEM[memory_core]
        LLM[llm_client]
        RAG[rag_core]
        INGEST[document_ingestor]
        EXP[export_core]
    end

    subgraph Data["Storage"]
        PG[(PostgreSQL)]
        VEC[(pgvector / Qdrant / Local)]
        OBJ[(Object Storage)]
        IDX[(Tantivy Index)]
    end

    UI --> API
    UI --> WS
    WS --> HUB
    API --> AUTH
    API --> GAME
    API --> INGEST
    API --> RAG
    API --> KP
    API --> DICEA
    API --> CHARA
    API --> CREA
    API --> MEM
    API --> EXP

    GAME --> RULES
    GAME --> DICE
    KP --> LLM
    DICEA --> RAG
    CHARA --> RAG
    KP --> RAG
    CREA --> RAG
    MEM --> RAG

    INGEST --> OBJ
    INGEST --> IDX
    INGEST --> VEC
    INGEST --> PG

    RAG --> IDX
    RAG --> VEC
    RAG --> PG

    GAME --> PG
    EXP --> PG
    EXP --> OBJ
```

建议的 monorepo 目录如下。这个划分把“服务接入”“规则语义”“实时会话”“Agent 输出”“导出与记忆”清晰分层，便于 Codex 分阶段实现：

```text
apps/
  web/

crates/
  server/
  auth/
  rag_core/
  document_ingestor/
  trpg_rules/
  dice_engine/
  agent_core/
  kp_agent/
  dice_agent/
  character_agent/
  creator_agent/
  game_core/
  llm_client/
  memory_core/
  export_core/
```

下表可直接作为 `ARCHITECTURE.md` 的骨架。优先级中的 **MVP** 表示第一批必须落地，**MVP+** 表示第二批即可，**Later** 表示接口预留即可。

| 模块 | 职责 | 输入 | 输出 | 关键接口 | 主要表/结构 | 优先级 |
|---|---|---|---|---|---|---|
| `apps/web` | 房间 UI、角色页、线索图、战况面板、导出页 | REST/WS 响应 | 页面、交互事件 | `fetch`, WS client, React Flow adapter | `RoomViewModel`, `ClueGraphVM` | MVP |
| `crates/server` | Axum REST + WS 网关 | HTTP/WS 请求 | JSON/事件流 | `Router`, extractor, `WebSocketUpgrade` | `ApiError`, `AuthContext` | MVP |
| `crates/auth` | 认证、房间角色、权限决策 | token、session、room role | `AuthzDecision` | `authorize()`, `filter_visibility()` | `users`, `room_members`, `roles` | MVP |
| `crates/rag_core` | 检索接口、embedding trait、vector trait、hybrid retrieval | query、filters | evidence list | `EmbeddingProvider`, `VectorStore`, `SearchIndex`, `retrieve()` | `DocumentChunk`, `Evidence`, `RetrievalFilter` | MVP |
| `crates/document_ingestor` | 文件/URL 导入、许可检查、解析、切块、索引 | pdf/md/html/txt/URL | chunks、embeddings、manifest | `parse()`, `check_license()`, `chunk()`, `ingest()` | `documents`, `document_chunks`, `embeddings` | MVP |
| `crates/trpg_rules` | 规则系统抽象 | system id、rule refs | 通用判定接口 | `RuleSystem`, `RollPolicy`, `CharacterSchema` | `SystemId`, `RuleRef` | MVP |
| `crates/dice_engine` | 骰式解析、随机数、奖惩/优势劣势 | dice formula、seed | roll result | `parse_roll()`, `roll()` | `DiceExpr`, `RollOutcome` | MVP |
| `crates/agent_core` | Agent 共通协议与 JSON schema | context | validated json | `AgentRequest`, `AgentResponse`, `SchemaValidator` | `AgentEnvelope` | MVP |
| `crates/dice_agent` | 行动→判定→检索规则→掷骰 | player action、char state、rule evidence | 检定结果 JSON | `resolve_action()` | `DiceDecision` | MVP |
| `crates/character_agent` | 车卡引导、合法性检查、修正建议 | character draft、system | 审核结果 JSON | `assist_create()`, `validate_sheet()` | `CharacterSheet`, `ValidationReport` | MVP |
| `crates/kp_agent` | 剧情推进、场景裁定、调用规则/模组 RAG | action、scene、memories、evidence | narration + state updates | `adjudicate_turn()` | `KpTurnResult`, `SceneState` | MVP |
| `crates/creator_agent` | 把 log 改写为故事与复盘 | session log、scope | story/export | `write_story()` | `StoryDraft`, `Recap` | MVP+ |
| `crates/game_core` | 房间、session、状态机、线索与战斗主循环 | actions、agent outputs | updated session state | `apply_turn()`, `save_session()` | `sessions`, `characters`, `clues`, `combat_states` | MVP |
| `crates/llm_client` | OpenAI-compatible 模型网关 | prompt、schema、provider config | model response | `chat_json()`, `embed()` | `ModelProvider`, `CompletionRequest` | MVP |
| `crates/memory_core` | 复盘提取、长期记忆写入、反馈闭环 | session logs、feedback | memory docs、preferences | `extract_memory()`, `retrieve_memory()` | `agent_memories`, `agent_feedback` | MVP+ |
| `crates/export_core` | Markdown/HTML/JSON 导出 | session、story、scope | export artifact | `export_markdown()`, `export_html()` | `session_exports` | MVP+ |

## RAG pipeline 设计

RAG 部分的核心目标，不是“尽可能多地塞文档”，而是**把合法来源、结构化切块、权限字段、可证据化返回**做扎实。OpenAI 的 Embeddings 文档明确把搜索列为嵌入的典型用途；pgvector 支持 exact 与 approximate nearest neighbor、cosine/L2/inner product 等距离；Qdrant 则明确要求对过滤字段建立 payload index，以保证带过滤条件的向量搜索性能。因此，本项目应把检索 pipeline 设计成：**发现/导入 → 许可审查 → 解析 → 按章节切块 → 生成嵌入 → 写向量/关键词索引 → 权限过滤检索 → 返回短证据**。

### RAG 总流程

```mermaid
sequenceDiagram
    participant U as 用户/KP
    participant API as server
    participant ING as document_ingestor
    participant LIC as license checker
    participant PARSE as parser+chunker
    participant EMB as embedding provider
    participant IDX as vector+keyword index
    participant RAG as rag_core
    participant AG as Agent

    U->>API: 上传文件 / 提交 URL
    API->>ING: ingest(request)
    ING->>LIC: 检查许可证与来源
    alt 许可证明确
        LIC-->>ING: approved
        ING->>PARSE: 解析 pdf/md/html/txt
        PARSE-->>ING: structured chunks + metadata
        ING->>EMB: 批量 embedding
        EMB-->>ING: vectors
        ING->>IDX: 写入 vector + BM25
        IDX-->>ING: chunk ids
        ING-->>API: ingest success
    else 许可证不明
        LIC-->>ING: pending_review
        ING-->>API: pending_review + reason
    end

    U->>API: query + room + role
    API->>RAG: retrieve(query, filters)
    RAG->>IDX: 先过滤可见性再检索
    IDX-->>RAG: ranked chunks
    RAG-->>AG: evidence list
    AG-->>API: 结构化裁定/叙事
    API-->>U: 可见结果
```

### 发现、许可检查、解析与切块

“自动发现”应只对**公开且可确认许可**的规则资源生效；对模组书，现实里大部分并不具有开放再利用许可，因此默认流程应以**用户上传 + 权利声明**为主。官方 SRD 是最适合做自动导入的第一批数据源。以 D&D 为例，D&D Beyond/Wizards 官方页面明确说明 SRD 5.2.1/5.1 可在 Creative Commons 下使用和引用；Creative Commons 官方页面则说明不同许可证的可分发、改编、商用与署名要求不同。对于 ORC，Paizo 官方 FAQ/AxE 把它描述为开放共享游戏机制的许可框架，而世界观、故事弧、独特角色与视觉素材等并不当然开放。基于这些原始资料，平台的默认规则应是：**开放规则内容可入库；模组剧情、世界观、美术和特有文本若未获明确许可，不自动抓取，只允许用户上传并声明权利；许可证不明一律 `pending_review`**。

解析层建议支持 `pdf/md/html/txt` 四种输入。`md/html/txt` 可以直接保留 heading 层级和段落边界；`pdf` 的目标不是“完美重建版式”，而是保留**页码映射、标题行、列表关系、可检索正文**。工程上，MVP 可以先把 PDF 支持定义为“优先文本抽取；遇到扫描 PDF 则失败并记录待人工 OCR”，而不是假装所有 PDF 都能自动高质量解析。这是出于可靠性考虑的工程建议。

切块规则建议如下：

| 项目 | 推荐值/策略 | 说明 |
|---|---|---|
| 主切块依据 | heading / section / subsection | 尽量沿文档结构切块，便于引用 |
| 目标大小 | 500–1200 tokens | 在可读性、命中率、证据短引用之间折中 |
| 重叠 | 60–120 tokens | 保留上下文连续性 |
| 强制切分 | 超长表格、法术列表、装备表、怪物 statblock | 避免一个 chunk 混入多个主题 |
| 禁止混合 | 不同章节、不同角色可见性、不同来源许可 | 避免权限与归属污染 |
| 模组特殊规则 | “章节正文”和“隐藏线索/NPC 真相/结局条件”分离 | 服务 KP-only 检索与 PL 无剧透展示 |

推荐的 `chunk_metadata` 字段如下，可直接写进 `rag_core::DocumentChunk` 与数据库：

```json
{
  "chunk_id": "uuid",
  "document_id": "uuid",
  "system_name": "string",
  "document_type": "rulebook|module|clue|session_log|memory",
  "document_title": "string",
  "section_path": ["Combat", "Initiative"],
  "heading": "string",
  "page_start": 12,
  "page_end": 13,
  "source_url": "string|null",
  "source_type": "official_srd|official_web|user_upload|pending_review",
  "license_name": "CC-BY-4.0",
  "license_url": "string|null",
  "visibility_scope": "public_rule|room_rule|kp_only_module|pl_visible_clue|kp_secret|character_private|session_log",
  "room_id": "uuid|null",
  "session_id": "uuid|null",
  "created_by": "uuid|null",
  "content_hash": "sha256",
  "token_estimate": 847,
  "embedding_model": "text-embedding-3-small",
  "parser_version": "v1",
  "ingested_at": "timestamp"
}
```

### Embedding 策略与向量存储选型

OpenAI Embeddings 适合默认云端方案；Ollama 与 llama.cpp 则适合作为本地与离线方案。OpenAI 的 embeddings API 以文本输入换取浮点向量，按输入 token 计费；Ollama 文档提供 `/api/embed`，并说明嵌入适用于 cosine similarity 与 RAG；llama.cpp server 文档则直接声明其暴露 OpenAI-compatible embeddings routes。由此可以定义三层 embedding 策略：**默认云端、可替换本地、测试用 mock**。

| 方案 | 优点 | 缺点 | 适用阶段 |
|---|---|---|---|
| OpenAI embeddings | 接入最简单、文档成熟、语义质量稳 | 需要联网与计费 | 默认生产方案 |
| Ollama embeddings | 本地调用、可离线、统一本地 API | 向量维度与模型质量差异较大 | 本地开发/隐私优先环境 |
| llama.cpp embeddings | 本地可控、OpenAI-compatible | 运维更手工 | 高可控本地部署 |
| Mock embeddings | 零依赖、便于单测 | 不能代表语义质量 | 单元测试与 smoke test |

| 向量存储 | 核心能力 | 过滤能力 | 运维复杂度 | 推荐用途 |
|---|---|---|---|---|
| SQLite/JSON + 应用层余弦 | 最低门槛 | 由应用层负责 | 很低 | MVP 离线 demo |
| pgvector | PostgreSQL 内 exact/ANN、多种距离、与业务数据同库 | 依赖 SQL 条件、可与 RLS/表结构协同 | 低到中 | 默认首选 |
| Qdrant | 向量 + payload，强调 filtered search 与 payload index | 很强，适合复杂过滤 | 中 | 检索量和过滤复杂度上来后的升级方案 |

### 检索与权限过滤流程

检索不能是“先搜后裁剪”，而应是“先过滤后检索，检索后再二次审核”。推荐流程：

1. 根据 `user_id / room_id / role / session_id / visibility_scope / document_type / system_name` 生成 `RetrievalFilter`。
2. 在应用层构造 SQL/Qdrant filter，只允许可见文档进入候选集。
3. 执行 **hybrid retrieval**：向量召回 + Tantivy/BM25 召回。
4. 用简单的 score fusion 或 RRF 合并结果。
5. 对 top-k 结果做二次权限审计。
6. 返回结构化 evidence，不直接生成回答。

推荐的证据返回格式：

```json
{
  "query": "侦查失败是否会导致线索完全消失？",
  "filters": {
    "room_id": "uuid",
    "role": "kp",
    "system_name": "coc7e_licensed_or_user_provided",
    "document_types": ["rulebook", "module"]
  },
  "evidence": [
    {
      "chunk_id": "uuid",
      "document_title": "Core Rules SRD",
      "document_type": "rulebook",
      "section_path": ["Investigation", "Skill Checks"],
      "page_start": 94,
      "page_end": 95,
      "score": 0.873,
      "license_name": "CC-BY-4.0",
      "visibility_scope": "public_rule",
      "preview": "short preview only, max 300-600 chars",
      "reason": "命中“侦查”“失败后果”“线索获取”语义"
    }
  ]
}
```

证据返回里的 `preview` 不应用作“原文复现窗口”。版权局明确说明合理使用没有固定字数安全线，是否成立依赖具体情境；因此更稳妥的工程策略是只返回**短证据摘录 + 章节信息 + 来源**，而把最终叙事留给 Agent 自己根据规则依据进行转述。

## Agent 设计

Agent 层推荐统一走 `agent_core`，其职责是：定义请求/响应 envelope、schema 校验、provider 适配、重试、日志与审计。OpenAI Structured Outputs、Ollama Structured Outputs、llama.cpp server 的 schema-constrained JSON 都支持“先定义 schema，再约束模型输出”；这非常适合 TRPG 场景，因为骰判定、状态更新、隐藏笔记和导出故事都天然是结构化数据。

### Dice Agent

Dice Agent 的目标不是代替 KP 叙事，而是把“是否要检定、怎么检定、依据哪条规则、结果是什么、结果如何影响剧情”结构化。它接收玩家动作、角色状态、场景上下文与规则 evidence；输出检定需求、骰式、阈值、修正项、掷骰结果和简短叙事提示。若证据不足，应把 `uncertainty` 设为非空，并降级为“建议人工确认”而不是强制做高风险裁定。

**输入 schema**

```json
{
  "action_text": "我检查书架后面的墙壁。",
  "system_name": "coc7e_licensed_or_user_provided",
  "character": { "id": "char_1", "skills": { "spot_hidden": 60 } },
  "scene": { "location": "abandoned_library" },
  "rule_evidence": [],
  "rng_seed": 42
}
```

**输出 schema**

```json
{
  "needs_roll": true,
  "roll_type": "skill_check",
  "skill": "spot_hidden",
  "dice_formula": "1d100",
  "target": 60,
  "difficulty": "regular",
  "modifiers": [],
  "roll_result": 42,
  "success_level": "regular_success",
  "rule_basis": [
    {
      "source": "Core Rule SRD",
      "section": "Skill Checks",
      "reason": "该动作属于搜索隐藏信息。"
    }
  ],
  "narrative_hint": "角色发现了书架底部有擦拭痕迹。",
  "uncertainty": null
}
```

**RAG 调用模式**：只查 `public_rule` 与该规则系统公开角色规则，禁止触达 `kp_only_module`。
**测试样例**：
- “攻击 goblin” 命中战斗规则，返回需要检定。
- “我翻看纸条内容”如果不涉及难度，返回 `needs_roll=false`。
- evidence 为空时，`uncertainty` 非空且不伪造具体条文。

### Character Agent

Character Agent 面向两个对象：PL 与 KP。对 PL，它是“创建/修正助手”；对 KP，它是“审核助手”。它读取规则书 RAG、职业/种族/技能点规则以及当前团的 house rules，输出合法性检查和修改建议。

**输入 schema**

```json
{
  "system_name": "dnd5e_srd_5_2_1",
  "mode": "player_assist",
  "character_draft": {
    "name": "Aria",
    "class": "wizard",
    "level": 1,
    "abilities": { "str": 8, "dex": 14, "con": 12, "int": 16, "wis": 10, "cha": 13 },
    "skills": ["arcana", "investigation"]
  },
  "rule_evidence": [],
  "house_rules": []
}
```

**输出 schema**

```json
{
  "is_valid": true,
  "errors": [],
  "warnings": [],
  "suggestions_for_player": [
    "若你偏向调查型玩法，可优先补足 Investigation 与工具熟练。"
  ],
  "kp_review_notes": [
    "该角色偏后排施法，前期生存能力一般。"
  ],
  "uncertainty": null
}
```

**RAG 调用模式**：只查规则书与房间内公开 house rules；不查模组书。
**测试样例**：
- 技能点超限时返回 `errors`。
- 缺少必填字段时返回 `warnings` 与创建建议。
- 非法职业特性组合时返回 `kp_review_notes` 和 rule basis。

### KP Agent

KP Agent 是全平台最敏感的 Agent。它可在人类 KP 在场时作为辅助，也可在 KP 缺席时运行，但必须受 **规则 RAG、模组 RAG、可见性策略、当前战况与长期记忆**共同约束。它应生成 **可见叙事** 和 **私有注记** 两层结果，而且私有注记绝不能发给 PL。

**输入 schema**

```json
{
  "mode": "assist_or_autorun",
  "player_action": "我撬开上锁的抽屉。",
  "room_context": {
    "room_id": "uuid",
    "session_id": "uuid",
    "role_of_requester": "pl"
  },
  "scene_state": {},
  "character_state": {},
  "combat_state": {},
  "public_evidence": [],
  "kp_only_evidence": [],
  "memory_evidence": [],
  "recent_log_summary": "..."
}
```

**输出 schema**

```json
{
  "visible_narration": "你把细长金属片插入锁孔，锁芯发出轻微咔嗒声。",
  "private_kp_notes": "抽屉里有第二层假底；尚未被玩家发现。",
  "rule_basis": [
    {
      "source": "Core Rules",
      "section": "Ability Checks",
      "reason": "撬锁属于工具/技能检定。"
    }
  ],
  "required_rolls": [
    {
      "actor": "player_1",
      "dice_formula": "1d20+5",
      "skill": "thieves_tools",
      "difficulty": 15
    }
  ],
  "state_updates": [
    {
      "path": "scene.flags.drawer_unlocked",
      "op": "set",
      "value": false
    }
  ],
  "new_public_clues": [],
  "new_private_clues": [
    {
      "title": "抽屉有假底",
      "visibility": "kp_secret"
    }
  ],
  "uncertainty": null
}
```

**RAG 调用模式**：
- 若请求者是 PL，模型输入上下文中可包含 KP-only 证据，但输出前必须经过“可见性投影”层。
- 若请求者是 KP，则可返回完整版本。
- 若处于 auto-KP 模式，系统也必须分别维护“内部真实状态”和“对 PL 的可见状态”。

**uncertainty 处理**：
- 规则库无依据：`uncertainty="rules_missing"`。
- 模组没命中：`uncertainty="module_missing"`。
- 规则与 house rules 冲突：`uncertainty="conflict_needs_human_review"`。

**测试样例**：
- PL 查询时，不得泄露 `kp_secret` clue 标题。
- evidence 里只有公开规则时，允许给出保守裁定。
- memory 里有“玩家不喜欢冗长描写”，应缩短 narration。

### Creator Agent

Creator Agent 的价值不是“再写一遍日志”，而是把 TRPG 过程转成**可读故事**与**可追溯复盘**。它必须支持同一份 session 生成两个版本：PL 无剧透版、KP 完整版。

**输入 schema**

```json
{
  "session_id": "uuid",
  "scope": "pl_safe|kp_full",
  "session_log": [],
  "visible_clues": [],
  "hidden_clues": [],
  "story_style": "novella"
}
```

**输出 schema**

```json
{
  "title": "失落灯火",
  "chapters": [
    { "heading": "雨夜入馆", "content_markdown": "..." }
  ],
  "appendix": {
    "missed_clues": [],
    "rules_disputes": []
  },
  "uncertainty": null
}
```

**RAG 调用模式**：
- `pl_safe` 只可见公开日志和公开线索。
- `kp_full` 可使用完整日志与隐藏线索。
**测试样例**：
- PL 版不得出现隐藏 NPC 动机。
- KP 版必须包含错过线索与分支说明。

### Memory Agent

Memory Agent 不应自动在线微调底层模型，而应把“学习”限制为**可控长期记忆与人工反馈系统**。这是更符合当前工程实践与合规边界的做法：OpenAI、Ollama、llama.cpp 都能提供结构化输出或本地/云端模型服务，但“自动学习”最稳妥的实现并不是擅自改模型权重，而是将复盘提炼成结构化偏好文档，再在后续检索与 prompt 组装时使用。

**输入 schema**

```json
{
  "session_id": "uuid",
  "session_log": [],
  "feedback": {
    "fun_score": 4,
    "pacing_score": 3,
    "fairness_score": 5,
    "story_score": 4,
    "notes": "希望线索提示再明显一点。"
  }
}
```

**输出 schema**

```json
{
  "memory_summary": {
    "highlights": ["玩家在图书馆推理段体验较好"],
    "player_preferences": ["偏好更明确的线索提示"],
    "pacing_notes": ["中段搜索时间略长"],
    "rule_disputes": [],
    "successful_scenes": ["雨夜入馆"],
    "failed_scenes": [],
    "kp_style_adjustments": ["减少过长环境描写", "提高线索明示度"]
  },
  "write_to_scopes": ["campaign", "player", "room"],
  "requires_human_approval": true
}
```

**测试样例**：
- 没有反馈时，也能只从日志提取非敏感风格记忆。
- 负反馈不应直接变成不可审计的永久规则，而应进入待批准状态。
- 输出必须不包含整段原始日志复写。

## KP 缺席模式、本地模型接入与安全边界

本项目的模型接入层应统一暴露类似如下环境变量：

```bash
OPENAI_API_KEY=
OPENAI_MODEL=
OPENAI_BASE_URL=

# 本地提供商可选
OLLAMA_BASE_URL=http://127.0.0.1:11434
LLAMA_CPP_BASE_URL=http://127.0.0.1:8080/v1
```

OpenAI 官方文档建议把 API key 存为环境变量而不是写进源码，并通过 Bearer 凭证从服务端调用；Ollama 官方文档说明其本地 API 默认在 `http://localhost:11434/api` 提供；Ollama 也提供部分 OpenAI API 兼容层；llama.cpp server README 则说明它暴露 OpenAI-compatible chat completions、responses 和 embeddings routes。基于这些官方能力，`llm_client` 应只处理一个统一的 provider trait：`chat_json() / chat_text() / embed()`，由后端在运行时决定调用 OpenAI、Ollama 还是 llama.cpp。

KP 缺席模式建议采用“双层约束”：

- **第一层是 RAG 约束**：KP Agent 只能看到被授权给它的规则/模组/记忆/日志上下文。
- **第二层是输出投影**：即使模型内部使用了 KP-only 证据，发给 PL 的结果仍需经过 server 端投影，只保留 `visible_narration`、公开检定请求和公开状态更新。

这能降低“模型一时失误把真相讲出来”的风险。数据库层可进一步使用 PostgreSQL RLS 保护敏感记录；PostgreSQL 官方文档说明启用行级安全后，若没有匹配策略，访问将默认拒绝。

安全与隐私上，还需要注意三件事。第一，OpenAI key 必须只在服务端可见，不能下发到浏览器代码。第二，Ollama 本地 API 在本机访问时默认不要求认证，因此开发机或家用服务器应绑定回环地址、反向代理鉴权或置于内网；不要把未鉴权的本地模型端口直接暴露到公网。第三，Creator Agent 与 Memory Agent 处理的是完整会话材料，导出与记忆写入都必须记录审计日志，以便回溯是谁导出了什么、批准了哪些长期偏好。

## 数据库、API 与前端交互

PostgreSQL 适合作为主数据库，不只是因为 SQLx 对 PostgreSQL/SQLite 都有官方支持，更因为本项目的数据天然是**强关系 + 半结构化 + 向量 + 审计**的混合体。PostgreSQL 官方文档说明 `jsonb` 可配合 GIN 索引高效搜索 key 或 key/value 对；这很适合存放规则系统特有扩展字段、角色卡扩展、Agent 输出快照与导出元数据。

### 数据库 schema 概要

| 表 | 关键字段 | 索引建议 | 说明 |
|---|---|---|---|
| `users` | `id`, `email`, `display_name`, `created_at` | `email unique` | 用户 |
| `rooms` | `id`, `title`, `system_name`, `owner_id` | `owner_id`, `system_name` | 房间/团 |
| `room_members` | `room_id`, `user_id`, `role` | `(room_id,user_id) unique`, `role` | 房间角色 |
| `campaigns` | `id`, `room_id`, `title`, `status` | `room_id` | 长线团 |
| `sessions` | `id`, `room_id`, `campaign_id`, `status`, `started_at` | `room_id`, `campaign_id`, `status` | 单场 session |
| `messages` | `id`, `session_id`, `author_id`, `visibility`, `content` | `session_id, created_at`, `visibility` | 聊天与系统消息 |
| `documents` | `id`, `room_id`, `document_type`, `visibility_scope`, `license_name`, `license_url`, `source_url`, `content_hash`, `status` | `room_id`, `document_type`, `visibility_scope`, `status` | 文档元数据 |
| `document_chunks` | `id`, `document_id`, `section_path`, `page_start`, `page_end`, `visibility_scope`, `content` | `document_id`, `visibility_scope`, `GIN(section_path jsonb)` | chunk 存储 |
| `embeddings` | `chunk_id`, `provider`, `model`, `vector` | 向量索引 | 向量层 |
| `characters` | `id`, `room_id`, `owner_id`, `system_name`, `visibility` | `room_id`, `owner_id` | 角色 |
| `character_sheets` | `character_id`, `sheet_json`, `version` | `character_id`, `GIN(sheet_json)` | 角色构建与版本 |
| `dice_rolls` | `id`, `session_id`, `actor_id`, `formula`, `result_json` | `session_id, created_at` | 掷骰审计 |
| `combat_states` | `id`, `session_id`, `state_json` | `session_id` | 战况 |
| `clues` | `id`, `session_id`, `title`, `visibility`, `state`, `source_chunk_id` | `session_id`, `visibility` | 线索 |
| `clue_edges` | `from_clue_id`, `to_clue_id`, `edge_type` | `(from_clue_id,to_clue_id)` | 线索关系 |
| `npc_profiles` | `id`, `session_id`, `visibility`, `profile_json` | `session_id`, `visibility` | NPC |
| `locations` | `id`, `session_id`, `visibility`, `location_json` | `session_id`, `visibility` | 地点 |
| `session_exports` | `id`, `session_id`, `scope`, `format`, `object_key` | `session_id`, `scope`, `format` | 导出物 |
| `agent_memories` | `id`, `scope`, `room_id`, `campaign_id`, `player_id`, `approved`, `memory_json` | `scope`, `room_id`, `approved`, `GIN(memory_json)` | 长期记忆 |
| `agent_feedback` | `id`, `session_id`, `author_id`, `fun_score`, `notes` | `session_id` | 反馈 |
| `audit_logs` | `id`, `actor_id`, `action`, `target_type`, `target_id`, `payload_json` | `actor_id`, `action`, `created_at` | 审计 |

建议对 `documents.visibility_scope`、`messages.visibility`、`clues.visibility`、`agent_memories.scope` 做枚举约束；对 `document_chunks` 与 `embeddings` 做分表或分区预留。若采用 PostgreSQL，`character_sheets.sheet_json`、`agent_memories.memory_json` 与 `documents.extra_metadata` 等 JSONB 字段建议配 GIN 索引。

### API 设计

Axum 的 extractor 模型很适合把 `Path / Query / Json / State / AuthContext` 清晰分开，且 WebSocketUpgrade 可直接建立 WS 会话。

**REST 端点建议**

| 方法 | 路径 | 权限 | 用途 |
|---|---|---|---|
| `GET` | `/health` | public | 健康检查 |
| `POST` | `/api/rooms` | user | 创建房间 |
| `GET` | `/api/rooms/:id` | member | 房间详情 |
| `POST` | `/api/documents/upload` | KP/owner | 上传规则书/模组 |
| `POST` | `/api/documents/:id/ingest` | KP/owner | 执行导入 |
| `POST` | `/api/rag/search` | member | 通用检索 |
| `POST` | `/api/rag/query-rules` | member | 规则查询 |
| `POST` | `/api/rag/query-module` | KP/assistant KP | 模组查询 |
| `POST` | `/api/characters` | member | 新建角色 |
| `POST` | `/api/characters/:id/validate` | owner/KP | 车卡校验 |
| `POST` | `/api/characters/:id/kp-review` | KP | KP 审核 |
| `POST` | `/api/sessions` | KP | 开新 session |
| `POST` | `/api/sessions/:id/action` | member | 玩家行动 |
| `POST` | `/api/sessions/:id/roll` | member/system | 掷骰 |
| `POST` | `/api/agents/kp/act` | KP/system | 调 KP Agent |
| `POST` | `/api/agents/dice/resolve` | member/system | 调 Dice Agent |
| `POST` | `/api/agents/creator/write-story` | KP/system | 导出故事 |
| `POST` | `/api/agents/memory/extract` | KP/system | 复盘写记忆 |

**REST 示例**

```json
POST /api/rag/query-rules
{
  "room_id": "room_123",
  "system_name": "coc7e_licensed_or_user_provided",
  "query": "侦查检定失败后，线索是否必须完全丢失？",
  "top_k": 5
}
```

```json
200 OK
{
  "evidence": [
    {
      "chunk_id": "chk_1",
      "document_title": "Core Rules SRD",
      "section_path": ["Investigation", "Skill Checks"],
      "visibility_scope": "public_rule",
      "score": 0.88,
      "preview": "..."
    }
  ]
}
```

### 前端页面与交互流程草图

Next.js App Router 是文件系统路由，适合把房间页、导入页、导出页和设置页组织成清晰的层级；React Flow 则非常适合表示节点—边结构的线索图、NPC 关系图和地点关系图。

建议页面如下：

| 页面 | 核心组件 | 关键交互 |
|---|---|---|
| 登录页 | 登录表单 | 登录/切换账户 |
| 房间列表 | room cards | 进入房间、创建房间 |
| 房间详情 | 成员列表、规则系统标签 | 邀请、角色入口、资料入口 |
| 规则库管理 | 上传器、license badge、ingest status | 上传、复审、重新导入 |
| 模组库管理 | 文档树、可见性设置 | 设为 KP-only、章节切分审查 |
| 角色创建页 | 表单、规则提示、修正建议面板 | Character Agent 辅助 |
| 房间游玩页 | 聊天区、Dice 面板、角色面板、线索面板、战况面板 | 实时跑团 |
| 线索可视化页 | React Flow 画布 | 节点筛选、边展开、时间轴联动 |
| 导出页 | Markdown/HTML/JSON 导出选项 | 下载、预览 |
| Agent 设置页 | provider/config form | 切换 OpenAI/Ollama/llama.cpp |
| 复盘页 | 反馈表、长期记忆审核表 | 批准/拒绝记忆 |

**WebSocket 事件示例**

```json
{
  "type": "player_action",
  "room_id": "room_123",
  "session_id": "sess_456",
  "player_id": "user_789",
  "content": "我检查书架后面的墙壁。"
}
```

```json
{
  "type": "dice_resolved",
  "session_id": "sess_456",
  "roll": {
    "formula": "1d100",
    "result": 42,
    "target": 60,
    "success_level": "regular_success"
  }
}
```

## 最终范围增补：规则适配器、地图与 Creator 插图

### 规则系统注册表

MVP 必须提供以下 `RuleSystem` 注册项：

```text
generic_percentile
  通用百分骰、成功等级、奖惩骰与可扩展角色字段。

dnd5e_srd_5_2_1
  使用允许分发的 SRD 内容与明确署名信息。

coc7e_compatible
  提供字段映射、百分骰流程、角色卡 schema 和插件接口；不在仓库或镜像中捆绑未授权商业规则正文。

licensed_commercial
  由官方授权包或用户声明有权使用的私有规则包驱动。
```

`RuleSystem` 代码适配器与“规则正文内容包”必须解耦。适配器可以定义数据结构和行为接口；规则正文只有在来源、许可证和可见性审核通过后才能进入 RAG。

### 地图能力

MVP 支持：

```text
scene_board
square_grid
hex_grid_flat_top
hex_grid_pointy_top
```

服务端保存统一世界坐标，并通过 `GridGeometry` 适配方格和六角格。路径、距离、范围模板和 Token 锚点不得写死为方格算法。等距与 3D 仅保留插件边界。

### Creator 插图插件

Creator Agent 可以调用 `ImageProvider` 生成章节封面、场景插图或角色纪念图，但必须满足：

- 默认关闭，必须由 KP/房主主动启用。
- PL 安全版与 KP 完整版使用不同上下文投影。
- 每张图进入 `draft`，人工批准后才能发布或导出。
- 独立记录 provider、模型、提示版本、费用、来源素材和许可声明。
- 不允许自动使用未公开 KP 内容生成 PL 可见图片。
- 图像生成失败不得阻塞文字故事导出。

### 并发与冲突处理

战斗、回合与角色卡的每次写入必须携带 `expected_version`，并以 compare-and-swap 更新。数据库事务捕获 PostgreSQL `40P01`（deadlock detected）与 `40001`（serialization failure）；只有带幂等键的命令才能进行最多三次指数退避重试。连续失败后返回 `409 concurrent_update`，同时附带最新版本或快照 URI。协作笔记和线索布局可以使用 CRDT，但不得用 CRDT 决定 HP、先攻、回合阶段、线索可见性或角色合法性。

## 合规、测试、部署与 Codex 任务包

### 法律与合规策略

合规策略的原则应尽量简单、可程序化、可审计：

1. **只接受明确许可来源**：官方 SRD、Creative Commons、ORC、用户声明有权上传的本地文件。
2. **许可证字段强制落库**：`license_name / license_url / source_url / uploaded_by / content_hash / imported_at / visibility_scope`。
3. **许可证不明即 `pending_review`**。
4. **模组默认 KP-only**，公开线索单独派生为 `pl_visible_clue`。
5. **RAG 只回短证据片段**，不回长段原文。
6. **导出与记忆都审计**。

可以把许可状态设计为：

```text
approved_open
approved_user_provided
approved_official_srd
pending_review
rejected_unknown_license
rejected_copyright_risk
```

推荐保存的 license metadata 字段：

```json
{
  "license_name": "CC-BY-4.0",
  "license_url": "https://creativecommons.org/licenses/by/4.0/",
  "source_url": "https://www.dndbeyond.com/srd",
  "source_type": "official_srd",
  "copyright_holder": "Wizards of the Coast LLC",
  "attribution_text": "stored if provided by source",
  "uploaded_by": "user_id_or_system",
  "accessed_at": "timestamp",
  "content_hash": "sha256"
}
```

### CI 与测试清单

SQLx 官方文档建议在 CI 中运行 `cargo sqlx prepare --check` 以保证查询元数据与 schema 同步。结合 Rust 与前端栈，建议最小测试矩阵如下。

| 测试 | 目标 |
|---|---|
| dice parser tests | 骰式解析正确 |
| dice seeded RNG tests | 可复现结果 |
| chunking tests | 按章节切块、页码映射、overlap 正确 |
| license checker tests | 许可证明确/不明确分流 |
| RAG permission filter tests | PL 不可命中 KP-only |
| Agent JSON schema tests | 所有 Agent 输出可校验 |
| KP non-disclosure tests | KP Agent 不泄露隐藏线索 |
| Character validation tests | 非法角色卡能报错 |
| Creator spoiler tests | PL 版无隐藏真相 |
| API smoke tests | 主要端点可通 |
| WS room tests | 基本实时消息链路可通 |

推荐 CI 命令：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo sqlx prepare --check

pnpm install
pnpm lint
pnpm test
```

### 从空仓库到 MVP 的开发与部署步骤

以下步骤可直接用于 Codex 的第一轮执行：

```bash
mkdir trpg-platform && cd trpg-platform
git init

# Rust workspace
cargo new crates/server --bin
cargo new crates/auth --lib
cargo new crates/rag_core --lib
cargo new crates/document_ingestor --lib
cargo new crates/trpg_rules --lib
cargo new crates/dice_engine --lib
cargo new crates/agent_core --lib
cargo new crates/kp_agent --lib
cargo new crates/dice_agent --lib
cargo new crates/character_agent --lib
cargo new crates/creator_agent --lib
cargo new crates/game_core --lib
cargo new crates/llm_client --lib
cargo new crates/memory_core --lib
cargo new crates/export_core --lib

# Frontend
pnpm create next-app apps/web

# Docs and config
mkdir -p docs prompts data/{raw,processed,pending_review,saves}
touch README.md AGENTS.md ARCHITECTURE.md LEGAL_POLICY.md AGENT_DESIGN.md
```

建议环境变量：

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/trpg_platform
REDIS_URL=redis://localhost:6379

OPENAI_API_KEY=
OPENAI_MODEL=
OPENAI_BASE_URL=

OLLAMA_BASE_URL=http://127.0.0.1:11434
LLAMA_CPP_BASE_URL=http://127.0.0.1:8080/v1

OBJECT_STORAGE_MODE=local
LOCAL_STORAGE_PATH=./data/blob
```

建议分阶段让 Codex 实现：

| 阶段 | 目标 | 交付物 |
|---|---|---|
| Phase 1 | 工作区、DB、基础 REST/WS、骰子与角色基础结构 | 可编译 monorepo |
| Phase 2 | 文档导入、切块、embedding provider trait、混合检索 | `ingest` 与 `query` 跑通 |
| Phase 3 | Dice/Character/KP Agent 与结构化出参 | 基础跑团循环 |
| Phase 4 | 前端房间页、线索图、战况面板、导出 | 可玩 demo |
| Phase 5 | Memory Agent、反馈审核、长期记忆检索 | 可控“学习”闭环 |

### 可直接交给 Codex 的任务清单

下面这部分可以直接复制到 Codex 任务里。由于 Codex 会读取 `AGENTS.md`，建议把“测试命令、禁止事项、合规边界”也一并写进仓库。

#### 首要二十个任务

1. 初始化 Rust workspace 与 Next.js monorepo。
2. 新建 `AGENTS.md`，写明许可证边界、测试命令、禁止抓取盗版规则书。
3. 写 PostgreSQL migration：`users / rooms / room_members / sessions / documents / document_chunks / characters / clues / agent_memories / audit_logs`。
4. 在 `server` 中搭建 Axum 路由与基础错误模型。
5. 在 `auth` 中实现 `AuthContext`、房间角色与可见性枚举。
6. 在 `dice_engine` 中实现骰式解析与 seedable RNG。
7. 在 `rag_core` 中定义 `EmbeddingProvider / VectorStore / SearchIndex / RetrievalFilter / Evidence`。
8. 在 `document_ingestor` 中实现 `txt/md/html` 解析与结构化 chunking。
9. 在 `document_ingestor` 中实现 license checker 与 `pending_review` 流程。
10. 在 `rag_core` 中实现本地 SQLite/JSON vector store 的 MVP 版本。
11. 接入 Tantivy 或最小 BM25 关键词检索。
12. 实现 hybrid retrieval 与权限过滤。
13. 在 `llm_client` 中实现 OpenAI-compatible `chat_json()` 与 `embed()`。
14. 接入 OpenAI Structured Outputs；为本地 provider 适配 Ollama/llama.cpp。
15. 在 `dice_agent` 中实现“动作→规则检索→判定→掷骰”。
16. 在 `character_agent` 中实现车卡验证与建议。
17. 在 `kp_agent` 中实现可见/私有双通道输出。
18. 在 `game_core` 中实现 session 主循环与状态更新。
19. 在前端完成房间页、角色创建页、线索图页、导出页 skeleton。
20. 补齐测试、CI、README 与架构文档。

### 可复制的 README 初稿

```md
# TRPG Online Platform

一个面向 TRPG 在线游玩的平台，支持：

- 合法规则书 / 模组书导入与 RAG
- Dice Agent 自动判定与掷骰
- Character Agent 车卡辅助与 KP 审核
- KP Agent 在缺席模式下辅助或代行 KP
- 线索图 / NPC 关系图 / 战况面板
- Creator Agent 生成 PL 无剧透版与 KP 完整复盘
- Memory Agent 基于可控长期记忆改善体验

## 技术栈

- Backend: Rust, Axum, Tokio, SQLx
- Database: PostgreSQL
- Vector: pgvector / local store / Qdrant
- Frontend: Next.js, TypeScript, React Flow

## 快速开始

```bash
pnpm install
cargo build
docker compose up -d postgres redis
cargo sqlx database create
cargo sqlx migrate run
cargo run -p server
pnpm --dir apps/web dev
```

## 环境变量

```bash
DATABASE_URL=
OPENAI_API_KEY=
OPENAI_MODEL=
OPENAI_BASE_URL=
OLLAMA_BASE_URL=
LLAMA_CPP_BASE_URL=
```

## 合规说明

本项目不抓取、不存储盗版规则书或版权不明模组。只支持：

- 官方开放授权 SRD
- CC / ORC / 其他明确许可文本
- 用户声明有权使用的本地上传文件

许可证不明的文件会进入 `pending_review`。

## 主要目录

- `apps/web` 前端
- `crates/server` API
- `crates/rag_core` 检索核心
- `crates/document_ingestor` 文档导入
- `crates/kp_agent` KP Agent
- `crates/dice_agent` Dice Agent
- `crates/character_agent` Character Agent
- `crates/creator_agent` Creator Agent
- `crates/memory_core` 长期记忆

## 测试

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo sqlx prepare --check
pnpm lint
pnpm test
```
```

### 可复制的 AGENTS.md 初稿

```md
# AGENTS.md

本仓库用于实现 TRPG 在线游玩平台。

## 必须遵守

- 主要后端实现使用 Rust。
- 不下载、不抓取、不存储盗版规则书或版权不明模组。
- 只允许导入官方开放授权 SRD、CC/ORC/明确开放许可文本，或用户声明有权使用的本地文件。
- 模组书默认 KP-only。
- RAG 输出不得复现大段版权文本。
- KP Agent 不得向 PL 泄露 KP-only 内容。
- API keys 只能通过环境变量读取，不得写入源码或前端。
- 长期学习只允许通过长期记忆 / 复盘 RAG / 人工反馈实现，不自动在线微调底层模型。

## 优先完成顺序

- workspace 与 migrations
- auth / visibility
- dice engine
- rag_core
- document ingestor
- llm_client
- Dice Agent
- Character Agent
- KP Agent
- game loop
- frontend skeleton
- tests and docs

## 常用命令

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo sqlx prepare --check
pnpm lint
pnpm test
```

## 代码风格

- 使用强类型 struct，避免裸 JSON 到处传递。
- 错误处理优先 `thiserror` / `anyhow`。
- 避免 `unwrap()`。
- 新增功能必须补测试。
- 模型输出必须经过 schema 校验。
```

### 可复制的 ARCHITECTURE.md 初稿

```md
# Architecture

## 总体原则

本项目采用“多知识库 RAG + 多 Agent + 严格权限过滤”的架构。

知识域至少包括：

- 规则书 RAG
- 模组 RAG
- 已公开线索 RAG
- 会话日志 RAG
- 长期记忆 RAG

## 后端

- Rust
- Axum
- Tokio
- SQLx
- PostgreSQL
- pgvector（默认）
- Tantivy（关键词检索）

## 前端

- Next.js App Router
- React Flow
- WebSocket 实时房间

## 模型接入

统一通过 OpenAI-compatible provider：

- OpenAI API
- Ollama
- llama.cpp server

## 关键原则

- 先权限过滤，后检索
- Agent 输出必须结构化
- KP 输出分为 visible / private 两层
- 长期学习不直接微调模型，只写入可审计记忆
```

### 可复制的 LEGAL_POLICY.md 初稿

```md
# Legal Policy

## 允许导入

- 官方开放授权 SRD
- Creative Commons 许可文本
- ORC 许可文本
- public domain 资料
- 用户声明有权使用的本地上传文件

## 不允许

- 盗版规则书
- 版权不明模组
- 无法确认来源的抓取文本
- 默认抓取商业模组全文

## 流程

1. 检查来源
2. 检查许可证
3. 保存 license metadata
4. 许可证明确则入库
5. 许可证不明则 `pending_review`

## RAG 输出规则

- 仅返回短证据片段
- 不返回整章、整页、长表格
- 必须返回来源、章节、许可信息
```

### 可复制的 AGENT_DESIGN.md 初稿

```md
# Agent Design

## Dice Agent

职责：
- 判断是否需要检定
- 查询规则 RAG
- 生成骰式
- 执行掷骰
- 返回结构化结果

## Character Agent

职责：
- 指导 PL 车卡
- 检查合法性
- 给 KP 审核辅助
- 输出修正建议

## KP Agent

职责：
- 剧情推进
- 应用规则
- 读取模组信息
- 输出 visible_narration 与 private_kp_notes
- 绝不向 PL 泄露 KP-only 内容

## Creator Agent

职责：
- 将会话日志改写为故事
- 生成 PL 无剧透版
- 生成 KP 完整复盘版

## Memory Agent

职责：
- 从完整 session 中提取长期记忆
- 记录玩家偏好与节奏偏好
- 必须支持人工审核
- 不能自动在线微调底层模型
```

### 可复制的 Codex 主任务提示词

```md
请在当前仓库中实现一个 TRPG 在线游玩平台 MVP，满足以下要求：

- Rust 后端（Axum/Tokio/SQLx）
- Next.js 前端
- 多知识域 RAG：规则书 / 模组 / 线索 / 会话日志 / 长期记忆
- Dice Agent / Character Agent / KP Agent / Creator Agent / Memory Agent
- 权限模型：PL 不能命中 KP-only 内容
- KP 缺席时支持 OpenAI-compatible 本地/远端模型
- 结构化输出与 schema 校验
- 严格合规：不导入版权不明资料

请按以下阶段执行：

Phase 1:
- workspace
- migrations
- auth
- dice engine
- server skeleton
- frontend skeleton

Phase 2:
- document ingestor
- chunking
- license checker
- local vector store
- hybrid retrieval

Phase 3:
- llm_client
- Dice Agent
- Character Agent
- KP Agent
- game loop

Phase 4:
- Creator Agent
- export_core
- React Flow clue graph
- combat dashboard

Phase 5:
- Memory Agent
- feedback review UI
- long-term memory retrieval

完成每一阶段后：
- 运行格式化、lint、测试
- 修复可修复问题
- 更新 README / ARCHITECTURE / LEGAL_POLICY / AGENT_DESIGN
- 汇报已完成、待完成、需要真实 API key 的部分
```

## 供 Codex 直接执行的首批任务优先序列

1. 创建 monorepo 目录、Rust workspace、Next.js 应用与顶层文档。
2. 写 `AGENTS.md`，锁定许可证边界、禁止事项、测试命令。
3. 建立 PostgreSQL migrations 与 `sqlx` 配置。
4. 在 `auth` 中定义用户、房间、角色、可见性与授权决策。
5. 在 `server` 中搭建 Axum REST 路由与统一错误处理中间件。
6. 加入 Axum WebSocket 房间总线，跑通基础聊天。
7. 在 `dice_engine` 实现骰式 parser、seedable RNG、优势/劣势/奖惩支持。
8. 在 `trpg_rules` 设计通用 `RuleSystem` trait 与角色卡扩展字段接口。
9. 在 `rag_core` 定义 `DocumentChunk / Evidence / RetrievalFilter / EmbeddingProvider / VectorStore / SearchIndex`。
10. 在 `document_ingestor` 实现 `txt/md/html` 解析器与章节化 chunking。
11. 在 `document_ingestor` 实现 `license checker` 与 `pending_review` 状态机。
12. 在 `rag_core` 实现本地 SQLite/JSON 向量存储 MVP。
13. 接入 Tantivy 或最小 BM25 关键词索引，实现 hybrid retrieval。
14. 在 `llm_client` 中实现 OpenAI-compatible `chat_json`、`chat_text`、`embed`。
15. 通过环境变量支持 OpenAI / Ollama / llama.cpp provider 切换。
16. 在 `dice_agent` 实现行动解析、规则检索、掷骰与结构化结果。
17. 在 `character_agent` 实现车卡创建辅助、合法性检查与 KP 审核结果。
18. 在 `kp_agent` 实现双通道输出与非泄露测试。
19. 在 `game_core` 实现会话主循环、状态更新、线索增量写入与存档。
20. 在前端实现房间页、角色页、线索图页、导出页，并补齐 CI、测试与文档。
