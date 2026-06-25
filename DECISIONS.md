# DECISIONS.md — 最终产品与技术决策

> 状态：**Accepted / Final**  
> 生效日期：2026-06-25  
> 本文件优先级高于旧研究文档中的“建议”“默认假设”“待确认”和“未指定”。

## 产品决策

| ID | 选择 | 最终解释 |
|---|---|---|
| P01 | C | 同时提供平台云服务与 Docker Compose 自托管版本 |
| P02 | A | 20 活跃房间、单房最多 8 人、200 WS 并发；压测 1000 WS |
| P03 | A | 首发单区域，数据模型和配置保留 `region_id` |
| P04 | B | Magic Link + OIDC；Passkey 和企业 SSO 后续 |
| P05 | B | Standard / Private Hybrid / Local Only 三档隐私模式 |
| P06 | B + C | Generic Percentile + D&D SRD 5.2.1 + COC/商业规则适配器及合法授权规则包。不得捆绑或抓取未授权商业正文 |
| P07 | A | MVP 使用外部语音链接；Beta 采用托管 LiveKit |
| P08 | B | TTS 插件化、默认关闭 |
| P09 | A + B | 场景板、正方形网格、六角形网格首发；等距/3D 延后 |
| P10 | B | 公共大屏为可选设备角色 |
| P11 | B | Creator 可选插图插件，默认关闭、人工审批 |
| P12 | B | 关键电影化过场默认 1200ms；可选 900/1200/1600，最大 1800 |
| P13 | B | 七个用户音频通道映射四条内部总线 |
| P14 | B | 自有、CC0、CC-BY、明确授权或用户权利声明；商业发行不接受 NC |
| P15 | B | 模型原始输入/输出加密保存 30 天，之后只保留摘要和用量 |
| P16 | B | 长期记忆人工批准，战役结束或 180 天复审 |
| P17 | A | MVP RPO 24h/RTO 4h；生产升级 RPO 15m/RTO 1h |
| P18 | B | 超预算时优雅降级，并执行月度硬上限 |

## 技术决策

| ID | 最终方案 |
|---|---|
| T01 | 模块化单体 Monorepo：多 Rust crate、一个 API、一个 Worker；按瓶颈再拆服务 |
| T02 | REST + OpenAPI，实时使用 WebSocket |
| T03 | PostgreSQL 权威状态；Redis Streams 可恢复事件、Pub/Sub 低延迟广播；S3/MinIO 对象存储 |
| T04 | 关系当前状态 + append-only 事件 + 周期快照 |
| T05 | 应用层 ABAC + PostgreSQL RLS，默认拒绝 |
| T06 | 生产默认 pgvector HNSW；保留 Qdrant Adapter |
| T07 | 本地 SQLite + FTS5 + Flat Cosine，建议上限 50k chunks |
| T08 | 向量 + BM25/Tantivy + RRF；权限过滤在召回前 |
| T09 | 默认规则证据 6、模组 4、记忆 3，总数硬上限 15 |
| T10 | Embedding 在 ingest 阶段批量异步生成，同一索引固定模型和维度 |
| T11 | PDF 文本提取；扫描页转可选 OCR Worker 并人工审查 |
| T12 | 所有会改变状态的 Agent 输出必须通过 JSON Schema |
| T13 | Provider 能力矩阵、健康检查、超时、熔断、隐私感知路由 |
| T14 | WebSocket 使用 `server_seq/last_acked_seq/resume_token/snapshot replay` |
| T15 | 战斗、回合、角色卡使用乐观锁 + PostgreSQL 死锁检测和有界重试；CRDT 仅协作笔记与线索布局 |
| T16 | Prompt 固定前缀版本化，动态上下文置尾，滚动摘要代替全量日志 |
| T17 | OpenTelemetry + Prometheus + Grafana；默认不记录完整 Prompt |
| T18 | Docker Compose 一键部署；Helm 为生产扩展 |
| T19 | TLS 1.3；短期 Access Token + HttpOnly 旋转 Refresh Session；签名上传和文件扫描 |
| T20 | Caddy/Traefik 处理 TLS 与边缘入口，Axum 为应用网关 |

## 解释性约束

### P06：商业规则支持

“支持商业规则”指：

1. 平台提供规则系统适配器、角色卡 schema、骰制接口和规则包安装机制。
2. 官方授权包可以作为单独发行物安装。
3. 用户可以上传其声明有权使用的私有规则文档。
4. 未获许可的商业规则正文不得进入仓库、镜像、演示数据或自动联网抓取流程。

### T15：并发与死锁

- `expected_version` 是所有战斗、回合、角色卡写命令的必填字段。
- `idempotency_key` 是所有可能重试命令的必填字段。
- PostgreSQL SQLSTATE `40P01`、`40001`、`55P03` 必须被分类处理。
- 有界重试最多三次；外部副作用通过 Transactional Outbox 在提交后触发。
- 重试耗尽返回 409 并强制客户端同步最新快照。
- CRDT 不得决定权威游戏状态或权限。

## 变更管理

任何对本文件的修改必须：

1. 新增或更新 ADR。
2. 更新相应设计文档、配置、API Schema 和测试。
3. 说明数据迁移、向后兼容和安全影响。
4. 通过人类项目负责人批准。
