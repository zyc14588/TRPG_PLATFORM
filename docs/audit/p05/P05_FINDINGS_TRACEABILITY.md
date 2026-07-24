# P05 深度检查问题修复追溯

```text
SOURCE_REVIEW = /tmp/codex-security-scans/TRPG_PLATFORM/fb6e1466_20260723T132131Z/report.md
BASE_HEAD = fb6e146612e4df66a508292245da6b995bbe64fb
LOCAL_IMPLEMENTATION_REPAIR = VERIFIED
STRICT_BATCH_ACCEPTANCE = BLOCKED | PARTIAL_NOT_ACCEPTED
```

本表以 2026-07-23 深度检查中的正式 family 为单位，不再使用旧的 `9/9`、`20/20` 或 `5/5`
关闭计数。`FIXED_LOCAL` 表示当前工作树的实现已经由负向或真实依赖测试验证；它不代表缺失的
P04 前置、干净 checkpoint、Hosted CI 或 production TLS 已通过。

## 实现与控制 family

| 检查 family | 修复 | 关键复验 | 状态 |
| --- | --- | --- | --- |
| `CANONICAL-DB-FORMAL-TUPLE-FORGERY`、`CANONICAL-EVENT-SECURITY-METADATA-BINDING`、`CANONICAL-RECEIPT-INTEGRITY-DOWNGRADE` | canonical integrity v2 绑定所有安全字段；consumer、recover、commit、witness append 均执行 keyed verification；DB roles 收紧 | canonical PostgreSQL 4/4、wrong-key/tamper regression、角色探针 | FIXED_LOCAL |
| `RAW-EVENTSTORE-CROSS-CAMPAIGN-REPLAY`、`RAW-EVENTSTORE-SELF-ISSUED-WORKFLOW-AUTHORITY` | unscoped multi-campaign replay fail closed；正式读取要求 campaign-bound live authorization，调用方不能自发 workflow authority | API canonical replay、shared-kernel unscoped replay、agent identity gate | FIXED_LOCAL |
| `DELETION-CROSS-CAMPAIGN-CONFUSED-DEPUTY`、`DELETION-JOB-EXISTENCE-ORACLE`、`DELETION-ORPHAN-PENDING-JOBS` | job 绑定 requester/campaign/subject；先完成真实 policy + canonical evidence，再原子分配；越权状态查询返回不透明 404；重试幂等 | API privacy 14/14、真实 policy route、失败 commit 无 pending job | FIXED_LOCAL |
| `DELETION-DATA-SUBJECT-KEYING`、`DELETION-JETSTREAM-ABSENCE-PROOF`、`DELETION-REDIS-INDEX-TTL` | data subject 独立于 visibility；NATS deletion digest/classification、Redis AEAD metadata/index、TTL/legacy 处理均绑定 subject；未知/错绑消息使 absence proof 失败 | 真实 deletion E2E 4/4、JetStream/Redis integration | FIXED_LOCAL |
| `DELETION-OBJECTSTORE-ADAPTER-MISMATCH` | worker 和 E2E 统一使用生产 MinIO/S3 adapter 语义，并验证对象不存在而非影子目录 | MinIO + deletion E2E、真实 agent-worker smoke | FIXED_LOCAL |
| `DELETION-TERMINAL-STATUS-FORGERY`、`LEGAL-HOLD-DEACTIVATION-AUTHORIZATION`、`SUBJECT-KEY-UNAUTHORIZED-DESTRUCTION`、`ERASED-SUBJECT-TOMBSTONE-CREATION-AUTHORIZATION` | transition trigger、running-job authority、append-only evidence 与最小数据库角色阻止应用凭据直接伪造终态/hold/key/tombstone | schema assertion、6 个拒绝角色探针、retained-history negative tests | FIXED_LOCAL |
| `OUTBOX-TERMINAL-STATE-AUTHORIZATION` | Outbox claim/ACK 绑定 delivery transition、claim token 和已验证 formal commit；应用角色不能直接改终态 | migration/schema assertion、JetStream integration、角色探针 | FIXED_LOCAL |
| `DB-GROUP-MEMBERSHIP-FORGERY`、`PRIVATE-GROUP-REPLAY-LIFECYCLE-ATOMICITY` | group membership 写入收紧；授权决策在同一持久连接/事务快照读取 membership 与 revocation，不再使用两次可漂移查询 | PostgreSQL identity revoke、private-group replay tests | FIXED_LOCAL |
| `CACHE-PRIVATE-METADATA-CONFIDENTIALITY` | Redis value 和敏感 metadata 同时进入 AEAD envelope，Debug 只输出脱敏引用 | cache unit tests、真实 Redis E2E | FIXED_LOCAL |
| `CALLER-MINTABLE-VISIBILITY-PRINCIPAL`、`MEMORY-RAG-PROCESSOR-AUTHORIZATION` | RAG/Memory RAG 接受 IdentityVerifier 铸造的 campaign-bound ReplayAuthorization；processor 与 target audience 分离 | Memory RAG、target audience、live replay tests | FIXED_LOCAL |
| `RAG-COPYRIGHT-USE-METADATA-BINDING`、`RAG-EMBEDDING-INTEGRITY` | snapshot 绑定来源事件、内容摘要、use/copyright metadata、embedding bytes/model、visibility/provenance 和 derivation receipt | pgvector RAG contract、schema assertion | FIXED_LOCAL |
| `PLUGIN-INPUT-MANIFEST-CLASSIFICATION`、`TOOL-GRANT-EXECUTION-TUPLE-BINDING` | host 验证输入 manifest/classification；grant 绑定 plugin/tool/schema/input/campaign/actor；ToolResult 仅在成功执行后铸造 | plugin host 6/6、tool provider contracts | FIXED_LOCAL |
| `FACT-EVIDENCE-TRUST-ROOT-AND-FACT-BINDING`、`FACTSOURCE-SEMANTIC-ATTESTATION` | 移除默认自签路径；evidence 由仓库 trust root 验证并绑定 target、canonical event、完整 command；DiceRoll 等来源要求专用 receipt | fact provenance 8/8、human confirmation 8/8、canonical integration | FIXED_LOCAL |
| `CLOUD-CONTEXT-PROVENANCE-BINDING`、`CLOUD-PROVIDER-ENDPOINT-IDENTITY`、`CLOUD-PROVIDER-CREDENTIAL-BINDING`、`CLOUD-ROUTE-POLICY-BINDING` | authorization 绑定精确最小上下文、bytes、endpoint/provider/model/credential/route/policy revision | cloud policy 6/6、cloud E2E、provider send tests | FIXED_LOCAL |
| `CLOUD-CONSENT-UNTRUSTED-CLOCK`、`CLOUD-AUTHORIZATION-REVOCATION-FRESHNESS`、`CLOUD-NOTICE-EVIDENCE-AUTHENTICITY` | 时间来自可信 clock；notice 持久化并绑定；发送边界重新检查 consent、secret 和 route 撤销 | cloud policy/E2E、provider transport tests | FIXED_LOCAL |
| `CLOUD-CONSENT-PUBLIC-WRITE-AUTHORIZATION`、`DB-CLOUD-CONSENT-FORGERY`、`POST-ERASURE-CONSENT-RESURRECTION` | 移除宽泛 public write；consent transition 要求授权来源；数据库角色收紧；erased subject trigger 禁止复活 consent | schema assertion、cloud E2E、post-erasure negative tests | FIXED_LOCAL |
| `CLOUD-MIGRATION-APPEND-ONLY-BACKFILL-CONFLICT` | migration 使用锁定、可重入的 append-only backfill 顺序，保留历史审计并拒绝冲突升级 | populated upgrade test、clean schema assertion | FIXED_LOCAL |
| `LOCAL-PROVIDER-REMOTE-BOUNDARY-MISCLASSIFICATION`、`PROVIDER-PRODUCTION-ATTESTATION` | endpoint 解析决定隐私边界，label 不能伪装；production attestation 绑定部署 snapshot 和可信 secret reference | provider secret 4/4、deployment contracts | FIXED_LOCAL |
| `LOCAL-MODEL-LEVEL4-CERTIFICATION-AUTHENTICITY` | Level 4 report 使用持久签名 receipt、模型/量化/runtime/测试集摘要，AI Keeper 每次检查真实认证 | certification、provider/runtime tests | FIXED_LOCAL |
| `SECRET-REVOCATION-DURABILITY` | mounted/KMS revocation 写入持久 catalog，manager restart 后仍拒绝已撤销版本 | secret boundary 5/5、provider send recheck | FIXED_LOCAL |
| `DEPLOYMENT-SECURITY-SNAPSHOT-EVENT-BINDING` | 部署 snapshot 绑定 provider endpoint、credential reference、环境和安全 policy，并写 governed event | deployment contracts、release service smoke | FIXED_LOCAL |
| `NATS-MTLS-CLIENT-INCOMPATIBILITY`、`REDIS-MTLS-CLIENT-INCOMPATIBILITY` | 客户端配置支持 CA/client cert/private key 并对远端 plaintext fail closed；Compose 配置与 secret path 对齐 | client negative contracts、静态 Compose checks | IMPLEMENTATION_FIXED；RUNTIME_TLS_NOT_PROVEN |
| `POSTGRES-PASSWORD-ARGV-EXPOSURE` | bootstrap 通过受限环境/文件输入密码，不再把密码放入 `psql` argv | shell/static contract、argv negative probe | FIXED_LOCAL |

## Evidence/false-green family

| 检查 family | 当前处理 | 状态 |
| --- | --- | --- |
| `PRIVACY-POLICY-PERMIT-DOUBLE` | Rust/OpenFGA/OPA 统一 `delete_personal_data`；新增真实 policy permit/deny 测试，固定 permit 只保留作内部单元隔离 | FIXED_LOCAL |
| `DELETION-VACUOUS-BRANCH-FIXTURE` | 缺少任一真实 surface 时测试失败；生产 adapter 与 E2E 共用，逐面记录真实 receipt | FIXED_LOCAL |
| `MODEL-CERTIFICATION-NO-EXECUTION` | certification 必须绑定实际执行摘要、签名和 durable store；provider/runtime 消费该 receipt | FIXED_LOCAL |
| `CLOUD-FALLBACK-FAKE-LEDGER` | production provider send 使用持久 consent/notice/route/secret，并在真实 transport boundary 消耗 authorization | FIXED_LOCAL |
| `SCHEMA-ASSERTION-SEMANTIC-DRIFT` | assertion 验证函数/触发器语义与 migration checksum；替换为 no-op 或故障注入后会失败 | FIXED_LOCAL |
| `COMPOSE-PLACEHOLDER-HEALTH` | Compose 使用真实 Rust binaries；release process smoke 已启动五服务和 web | PLACEHOLDER_FIXED；PRODUCTION_COMPOSE_TLS_NOT_RUN |
| `UNBOUND-LOCAL-PASS-ATTESTATION` | 旧 `9/9`、`20/20`、`5/5` 全部撤回；raw logs 保留在扫描目录；release readiness 保持 BLOCKED | BLOCKED_BY_INHERITED_HISTORY |

## 为什么仍不能标记 COMPLETE

- `UNBOUND-LOCAL-PASS-ATTESTATION` 的历史边界不能在当前混合 dirty worktree 中事后制造；
- P04 严格前置没有已验证完成证据；
- production Compose TLS/mTLS、external secrets、Hosted CI 和最终 CodeRabbit 未执行；
- 没有外部、不可变、绑定当前干净 commit 的完整 `scripts/ci/test-all.sh` 发布证据。

因此，所有 `FIXED_LOCAL` 只描述当前可验证实现；严格批次结论仍为
`BLOCKED | PARTIAL_NOT_ACCEPTED`。
