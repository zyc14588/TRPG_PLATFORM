# P03 最终修复状态

```text
BATCH_ID = P03
BATCH_STATUS = COMPLETE
BASE_HEAD = 4c090988212d4024b030067312130178cd596978
WORKTREE = STAGED_AND_UNCOMMITTED
VERIFIED_AT_UTC = 2026-07-16T17:51:53Z
LOCAL_P03_TECHNICAL_REPAIR = PASS
FINAL_CODERABBIT_REVIEW = PASS_AFTER_1_VALID_MINOR_FIX_SECOND_PASS_0_FINDINGS
HOSTED_CI_FOR_THIS_UNCOMMITTED_TREE = NOT_RUN
PRODUCT_RELEASE_READY = NO
P04_EXECUTED = NO
```

该结论只覆盖 P03“Migration 历史不可变、唯一 Schema 来源、真实升级与机器
断言”范围，不代表整个产品可以发布。P00、P01 的完成证据分别位于
`docs/audit/p00/P00_FINAL_STATUS.md` 与 `docs/audit/p01/P01_FINAL_STATUS.md`；
两者均明确允许进入后续批次。

## 本轮复核问题的关闭结果

1. P03 前向迁移现在在任何 DDL 和成功账本写入之前，精确核验
   `reject_canonical_append_mutation()` 与
   `enforce_canonical_audit_chain()` 的语言、返回类型、执行属性和完整函数体。
   `CREATE OR REPLACE FUNCTION` 保持相同 OID 的绕过已由真实 PostgreSQL 负向测试覆盖。
2. 前置校验和最终机器签名都纳入 `canonical_audit_log` 的列、全部约束、
   append-only/no-truncate/chain triggers 和函数体。缺失审计触发器或链函数漂移时，
   P03 明确失败，且 `_sqlx_migrations` 不出现 P03 成功记录。
3. 审计 HMAC 采用向前兼容版本：迁移前记录保留 `integrity_version=1` 和原 HMAC
   输入，不伪造重签；迁移后只能写 `integrity_version=2`，其 HMAC 同时绑定
   `occurred_at` 的 PostgreSQL 微秒表示。普通时间戳修改被 trigger 拒绝；专用测试
   即使使用 replication restore 模式绕过 trigger，只修改 1 微秒也会触发
   `canonical_audit_hmac_mismatch`。
4. 所有动态 NATS header value 先经 `HeaderValue::from_str` 可失败解析。历史
   `idempotency_key`、`correlation_id` 或 `commit_id` 中的 CR/LF 不再触发
   async-nats assertion/panic，而是只让该 outbox 行进入既有 retry/dead-letter 路径。
   真实 JetStream 测试证明同批合法行继续发布、claim 正常释放。
5. 已用 PostgreSQL 16.14 原生 `pg_dump -F c` 为四个未写 SQLx 账本的 P02
   回滚源库创建备份，并用 PostgreSQL 16.14 `pg_restore --exit-on-error` 恢复到四个
   全新数据库；所有表的行数与内容摘要逐表一致。PostgreSQL 18 创建的旧失败归档
   不再作为成功证据。
6. 已完成本机 migration ledger 库存。所有实际观察到的
   `20260705000100` 成功账本均为冻结 b-24 SHA-384；没有观察到 b-25 rewrite
   SHA-384。旧 P03 forward checksum 仅存在于本轮之前创建的本机测试库，未被手工
   修补，且不作为部署证据。
7. Schema 精确断言不再只比较 trigger type/function OID 或函数 `prosrc`。最终签名
   覆盖 trigger 的 `WHEN` predicate、enabled/constraint/deferrable/args 和完整定义，
   以及函数的 security-definer、`search_path`/其他 `proconfig`、执行属性、owner 关系
   和完整定义。`WHEN(false)` 与 `SECURITY DEFINER` 绕过均由真实 PostgreSQL 负向
   探针拒绝并自动回滚。
8. Canonical commit 不再把 `stream_id` 强制折叠成 `campaign_id`。授权资源 ID 被
   无损映射为数据库 Stream，锁、版本、幂等查找、Event、Outbox、Formal Commit、
   replay API 与 NATS envelope 都保留独立 Stream；同 Campaign 两个 Stream 使用相同
   客户端幂等键仍各自从版本 1 提交。JetStream 去重 ID 绑定完整持久化幂等作用域，
   不再跨 Stream 静默吞消息。
9. `scripts/ci/test-all.sh` 的最终 schema assertion 直接使用受保护的
   `P03_DATABASE_URL`，不再硬编码 Docker 容器名和另一个数据库目标。

## AUD 验收矩阵

| ID | 状态 | 最终证据 |
|---|---|---|
| AUD-048 | PASS | 空库、b-24、漂移库和重复执行均在 PostgreSQL 16/pgvector 上真实运行；最终 schema SQL 含负向写入。 |
| AUD-066 | PASS | 已发布 migration 与 b-24 fixture 字节一致；SHA-256 `89166284272c7b5e5603178362501dd484c4e536ffdfd2ba4e281e1e5785d7f7`；b-25 ledger 明确 VersionMismatch。 |
| AUD-067 | PASS | Rust migrator 只通过 `sqlx::migrate!` 读取 `migrations/**`；原 Rust 内嵌 DDL 已删除。 |
| AUD-071 | PASS | SQLx/Serde Event、Outbox、Projection DTO 真实 round-trip，无损读取升级数据；canonical write/replay/API/NATS 均保留独立 `stream_id`。 |
| AUD-072 | PASS | 类型漂移、同 OID 函数体漂移、审计 trigger 缺失、审计链函数漂移、trigger `WHEN(false)` 和函数 security-definer/search-path 漂移均在成功 ledger 前或最终 assertion 中失败；columns/constraints/triggers/functions 有精确机器签名。 |
| AUD-073 | PASS | JSONB、enum、非负/非空白、引用、metadata binding、append-only、formal set、历史分类和审计完整性版本约束均由数据库拒绝非法状态。 |
| AUD-074 | PASS | 已知旧事件逐步 upcast；未知版本 fail-closed；真实历史 replay 通过。 |
| AUD-076 | PASS | idempotency 绑定 campaign/stream/operation/request hash/首次结果；真实 canonical commit 证明同 Campaign 不同 Stream 的同键请求互不冲突，JetStream 也不会跨作用域去重。 |

## Migration 不可变性与部署决策

| 对象 | SHA-256 | SQLx SHA-384 | 决策 |
|---|---|---|---|
| 冻结 `20260705000100` 与 b-24 fixture | `89166284272c7b5e5603178362501dd484c4e536ffdfd2ba4e281e1e5785d7f7` | `40539cf7e8f2fd0a87481a7c41dc1d14b24083ceaee3dbe3ab3d6f6b38e76bbfd117942b3d20b4ef547ccb40be709379` | 唯一受支持历史 ledger。 |
| 观察到的 b-25 rewrite fixture | `d2d9a58a0a24613935c5a91772fd209e5b18434692488e80bbedd2890bedba70` | `7cd30a91cb521ba1288303287d8cac9674a65d81630c741480e708b58e799598ee6fa63509c49c064edd5ea200f376e7` | 不兼容状态；fail-closed，禁止 ledger 编辑。 |
| 当前未发布 P03 forward migration | `ce19818ad077189edd5ed222e38f10911ecddb032d9321eaf0cd83e481222ed3` | `a724ba30e6d271c0449e1ca5968ee3bdb20d4dd78b35d88d66362f80ad967b9634c936b97d4fe8a0b1a9330020edce1f` | 本工作树仍未提交/发布；最终专用测试库使用该 checksum。 |

若外部环境将来发现 b-25 ledger 或任一未发布 P03 checksum，操作步骤必须是：

1. 停止写入并保留原库；
2. 用匹配服务端主版本的工具完成并验证备份；
3. 从受支持的 b-24 备份恢复到新库，或在新空库执行当前 migration；
4. 运行 `sqlx migrate info`、重复 `sqlx migrate run` 和 `assert-schema.sql`；
5. 验证内容后切换连接。不得 UPDATE/INSERT/DELETE `_sqlx_migrations`，不得修改已发布 migration。

## 实际范围核算

P03 最终差异为 34 个文件。以下是超出提示词“预计文件”但仍属于允许的
Migration CI、测试或最小接口适配的逐项说明：

| 文件 | 最小必要性 |
|---|---|
| `.github/workflows/ci.yml` | 让既有 CI 启动同一组 P02/P03 真实服务；仅修改步骤名称。 |
| `.github/workflows/release.yml` | 与 CI 使用同一 migration gate；仅修改步骤名称。 |
| `Cargo.lock` | 锁定 P03 新增的 SQLx/Serde/chrono 等持久化依赖解析。 |
| `MANIFEST.md` | 记录新增 migration、fixture、DTO、测试和 evidence。 |
| `apps/api-server/src/lib.rs` | 最小接口适配：canonical replay JSON 保留已持久化且已授权的 `stream_id`，没有新增 handler、写路径或业务能力。 |
| `apps/api-server/tests/canonical_replay_integration.rs` | 在真实双 PostgreSQL replay 测试中证明 `stream_id` 无损输出且既有成员关系/Visibility 过滤不变。 |
| `apps/migration-runner/src/main.rs` | 删除对已移除 Rust 内嵌 SQL 列表的调用，改为统计同一 SQLx migrator；无 API/业务变化。 |
| `crates/trpg-data-eventing/src/event_bus_nats_impl.rs` | 适配历史 Outbox integrity 分类并关闭 CR/LF header panic；同时把 Stream/operation 写入 envelope/header，并将 JetStream 去重 ID 绑定完整幂等作用域。 |
| `crates/trpg-data-eventing/src/event_store_sqlx_outbox_projection.rs` | 现有 SQLx persistence adapter 对新 schema、历史分类和 audit HMAC v1/v2 的最小兼容实现；Campaign/Stream 锁、版本、幂等与落库不再折叠。 |
| `crates/trpg-data-eventing/src/lib.rs` | 仅导出新增 `persistence` DTO/upcaster module。 |
| `crates/trpg-data-eventing/src/persistence_migrations.rs` | 删除第二套 Rust DDL，改为 canonical `sqlx::migrate!`。 |
| `crates/trpg-data-eventing/src/sqlx_migrations.rs` | 保留旧调用面，但只转发 canonical migrator。 |
| `crates/trpg-data-eventing/src/sqlx_migrations_contract.rs` | 暴露冻结版本/hash 与当前必需列供测试使用。 |
| `crates/trpg-data-eventing/tests/batch_024_data_eventing_contract_tests.rs` | 将旧内嵌 SQL 断言改为 canonical migrator/hash 断言。 |
| `crates/trpg-data-eventing/tests/batch_025_data_eventing_contract_tests.rs` | 同步唯一 schema source 契约。 |
| `crates/trpg-data-eventing/tests/canonical_commit_postgres.rs` | 真实事务/idempotency/audit v2 时间戳篡改测试，并覆盖同 Campaign 多 Stream 独立版本/幂等及授权资源不匹配拒绝。 |
| `crates/trpg-data-eventing/tests/event_store_contract.rs` | 同步 migration source 与事件版本契约。 |
| `crates/trpg-data-eventing/tests/jetstream_redis_integration.rs` | 真实 b-24 pending row、损坏行隔离、CR/LF header 非 panic，以及同键不同 Stream 均获 JetStream ACK 的回归。 |
| `manifests/CURRENT_PACKAGE_MANIFEST.md` | 当前包文件库存同步。 |
| `manifests/SELF_CONTAINED_PACKAGE_MANIFEST.md` | 自包含包文件库存同步。 |
| `scripts/ci/p02-integration-services.sh` | 主库切换到 digest-pinned pgvector PG16，并创建名称受保护的 P03 migration DB。 |
| `scripts/ci/test-all.sh` | 强制真实服务变量并执行 migration/schema gate；无 `continue-on-error`。 |

其余差异全部位于提示词明确允许的 `migrations/**`、
`crates/trpg-data-eventing/Cargo.toml`、`src/persistence/**`、历史 fixtures、
`migration_upgrade` 与 `scripts/ci/assert-schema.sql`。除上述两处 replay 字段最小适配外，
没有新增 API handler 或写路径；没有 P04 表、P04 worker、UI 或模型路由变更。

## 回滚边界

- 数据回滚只使用 `/tmp/p03-backup/pg16-prechange/**` 的已验证 PG16 archives 恢复到新库；
  不执行删除 Event Store 正史的 down migration。
- 代码回滚只能在提交后创建普通 revert；不得改写 Git 历史或 force push。
- 本轮未修改任何密码，也未修改 `_sqlx_migrations` 内容。

完整命令、52 项 package 测试、1 项真实 API replay 测试、备份摘要和本机 ledger
库存见 `P03_TEST_RESULTS.md`。CodeRabbit 第一轮唯一有效 minor 已修复，第二轮覆盖
全部 34 个文件且 findings=0；三份 manifest 随后按最终 index 重生成并通过检查。
