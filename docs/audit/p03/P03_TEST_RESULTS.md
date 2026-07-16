# P03 最终修复测试与证据

除明确标注的预期失败/中止外，本文件中的 PASS 命令均在
2026-07-17 AEST 实际执行并返回退出码 `0`。数据库、pgvector、独立 witness、
NATS JetStream 与 Redis 均为真实本地服务；没有把条件跳过计为通过。

## 开始门禁与事后重建基线

```text
git rev-parse HEAD
4c090988212d4024b030067312130178cd596978

git status --porcelain=v1
复核开始时 32 个 P03 文件已暂存、未提交；没有把 git diff --name-only 的空输出误当作无差异。

git diff --cached --check
exit 0
```

上述 `32` 是本轮修复开始前的历史快照，不是最终文件数。本轮根据复核发现新增了
`apps/api-server/src/lib.rs` 与
`apps/api-server/tests/canonical_replay_integration.rs` 两个最小 replay 接口适配，
因此最终差异为 34 个文件。三份生成式 manifest 已包含在原 32 个文件中，不额外
增加最终路径数；最终 34 个文件全部暂存，没有未暂存或未跟踪的修复产物。

原始 P03 执行没有在修改前保存完整测试日志，该历史动作不能被伪造。补救方式是
`git archive HEAD` 到 `/tmp/p03-baseline-4c090988`，在四个全新专用数据库上事后
重建基线：

```text
cargo test -p trpg-data-eventing --all-features --locked
42 passed; 0 failed; 0 ignored
```

该结果只证明 `4c090988…` 当时已有测试通过；它不证明 AUD-048/066/072 等 P03
缺陷不存在。新增负向测试正是为了覆盖旧测试未发现的漂移和 panic 路径。

## 必需 P03 命令

| 命令 | 结果 |
|---|---|
| `sqlx migrate info --source migrations` | PASS；七个 migration 均为 `installed`。 |
| `sqlx migrate run --source migrations` | PASS；已安装库无输出、ledger 行数不变，是真正 no-op。 |
| `cargo test -p trpg-data-eventing --test migration_upgrade --all-features --locked` | PASS；1/1。 |
| `psql ... -f scripts/ci/assert-schema.sql` | PASS；输出 `P03_SCHEMA_ASSERTION_OK`。 |
| `cargo test -p trpg-data-eventing --all-features --locked` | PASS；52 passed，0 failed，0 ignored；使用独立 canonical/witness PostgreSQL、eventing/witness PostgreSQL、JetStream 与 Redis。 |
| `cargo test -p api-server --test canonical_replay_integration --all-features --locked -- --nocapture` | PASS；1/1；真实双 PostgreSQL 链，验证 API replay 保留 `stream_id` 并继续按实时成员关系/Visibility 过滤。 |
| `cargo clippy -p trpg-data-eventing --all-targets --all-features --locked -- -D warnings` | PASS。 |
| `cargo check --workspace --all-targets --all-features --locked` | PASS。 |
| `cargo fmt --all -- --check` | PASS。 |
| `python3 scripts/ci/validate_workflows.py` | PASS。 |
| `python3 scripts/ci/check_dependency_directions.py` 及其 3 个负向测试 | PASS。 |
| `python3 scripts/ci/check_product_boundaries.py` 及其 5 个负向测试 | PASS。 |
| `python3 scripts/ci/discover_tests.py --check` | PASS；206 Rust targets，48 fixtures，0 orphan。 |
| `python3 scripts/ci/manifest.py --check` | PASS；三份确定性 manifest 已在最终 CodeRabbit 收口后按当前 index 再次重生成，均为 3856 lines。 |
| `bash -n scripts/ci/p02-integration-services.sh scripts/ci/test-all.sh` | PASS。 |
| `git diff --check` | PASS。 |

Hosted CI 没有在这个未提交工作树上运行，不能写成 PASS。CodeRabbit CLI `0.6.5`
已完成两轮 `uncommitted` review：第一轮 1 个有效 minor（32/34 文件计数缺少解释），
修复并重生成 manifest 后第二轮覆盖全部 34 个文件且 findings=0。

## 关键负向与绕过回归

| 缺陷/绕过 | 证据 |
|---|---|
| 错误 pre-P03 列类型 | migration 明确失败，P03 success ledger count 为 0。 |
| 同 signature/OID 的 mutation function 被 `CREATE OR REPLACE` 改成 `RETURN NEW` | OID 前后相同；P03 以 `mutation function drift` 失败，success ledger 为 0。 |
| 删除 `canonical_audit_log_append_only` | P03 以 `mutation trigger drift` 失败，success ledger 为 0。 |
| audit-chain function 同 OID 改成 `RETURN NEW` | P03 以 `chain function drift` 失败，success ledger 为 0。 |
| trigger 保留名称/事件掩码/函数但增加 `WHEN(false)` | migration preflight 与最终 `assert-schema.sql` 都明确失败；退出码 3，事务自动回滚。 |
| `enforce_event_outbox_binding()` 改为 `SECURITY DEFINER` 并设置 `search_path` | 最终函数执行属性/完整定义签名明确失败；退出码 3，事务自动回滚。 |
| 升级后 UPDATE/DELETE/TRUNCATE 审计记录 | append-only/no-truncate guards 拒绝；`occurred_at` 保持不变。 |
| 新写 `integrity_version=1` audit | `historical audit integrity version is migration-only`。 |
| v1 历史 HMAC 兼容 | 使用真实 P02 已签名向量得到原 hash `c8222f...bbbae`；改变 v1 timestamp 不冒充重签。 |
| v2 timestamp-only tamper | replication restore 模式绕过 trigger 后仅加 1 微秒，`verify_integrity()` 返回 `canonical_audit_hmac_mismatch`。 |
| b-25 checksum | SQLx 返回 `VersionMismatch(20260705000100)`，没有 ledger 修补。 |
| 同 Campaign 两个 Stream 使用相同客户端幂等键 | 两个 commit 均成功且各自 `stream_version=1`；数据库保存 `scene_alpha`/`scene_beta`，没有被 Campaign 级锁或全局幂等错误合并。 |
| JetStream 跨 Stream 去重 | `Nats-Msg-Id` 绑定 campaign/stream/operation/outbox idempotency key；真实批次两条同键不同 Stream 消息都收到 ACK。 |
| 历史 CR/LF NATS headers | unit test 返回 `InvalidOutboxPayload` 而非 panic；扩展真实批次 claimed 6、published 4、failed 2、dead-lettered 0，两个失败行 retry=1 且 claim_owner 释放。 |
| corrupt Outbox 与合法行同批 | 损坏行独立失败，两个合法行收到 JetStream ack 后标记 published。 |
| 未知 payload schema version | upcaster fail-closed。 |

首次扩充 migration 负向测试时，失败 migration 留在池连接上的 SQLx advisory lock
导致后续探针等待；该次运行被主动中止（exit 130），没有计为 PASS。测试随后在每个
预期失败后关闭并重建连接池，复跑通过。首次生成 schema MD5 时还发现一次人工分隔
错误以及一次 audit `TRUNCATE` 先被 FK 拒绝；两者都未作为通过证据，最终签名来自
真实迁移库查询，truncate 保护由精确 trigger signature 验证。

## 最终机器签名

```text
constraint_signature       1289e4f2857a305fc7283fd02319db11
trigger_signature          5b2eddf13c822cb4a220f7f9c790cc73
trigger_function_signature 6db05b2875333b14d2962a4f56d433e8
```

签名范围包含 `event_store`、`event_outbox`、`projection_checkpoint`、
`formal_commits`、`canonical_audit_log`，以及所有 P03 依赖的 trigger functions。
Trigger 签名额外覆盖 relation、enabled/type、constraint/deferrable、args、`WHEN`
predicate 与完整 `pg_get_triggerdef`；函数签名额外覆盖 language/kind、volatility、
parallel/strict、security-definer/leakproof、cost/rows、`proconfig`、owner 关系、support
function 与完整 `pg_get_functiondef`。

## PostgreSQL 16 原生备份与恢复

生成工具：`pg_dump (PostgreSQL) 16.14`，格式 `-F c`。四个 archive 均由 PG16
`pg_restore --exit-on-error` 恢复到全新专用库。

| Archive | SHA-256 |
|---|---|
| `/tmp/p03-backup/pg16-prechange/p02_complete4_canonical.pg16.dump` | `8ccd1f064cfb6d4c479e1360c9ee004e43234bac155ef2106566efa7932917c9` |
| `/tmp/p03-backup/pg16-prechange/p02_complete4_canonical_witness.pg16.dump` | `520dec7729207ba6528647bb651d34e24cf70b6272eca252987bc4201b6d2e76` |
| `/tmp/p03-backup/pg16-prechange/p02_complete4_eventing.pg16.dump` | `e4e1ff1bba4716efbd70de4c2708431ef4296abb670be25939fe1e5caddb53b1` |
| `/tmp/p03-backup/pg16-prechange/p02_complete4_eventing_witness.pg16.dump` | `b7fa6552e156d68cb5f557b3ed925593db0ab3d6e1da057fc4c11e52c219fd20` |

源库与恢复库逐表 `row_count/content_md5` 完全相同：

```text
canonical primary:
canonical_audit_log 2 18b5cc4f4bb39ecffac514d58aeb9e5d
event_outbox        3 dc7141cec3d2cf70b044e59d7a91972d
event_store         3 6dbee771141db2e749ebca3f488d7be0
formal_commits      2 9a3e70db0fb734f6da0e19ae75a3efe3
projection/workflow tables 0 d41d8cd98f00b204e9800998ecf8427e
canonical witness: external_audit_witness 6 b2fba6bbf2a9706aefaaa8472520b5ce

eventing primary:
canonical_audit_log 1 a84727f7f9edee23ba103d6e255e900e
event_outbox        1 985b64bb29954a1e9dd26ae780dafc46
event_store         1 edd06302c966b17debbb955f02f67b04
formal_commits      1 615d4dff6ad21482ee634d8f87fc0ca7
projection/workflow tables 0 d41d8cd98f00b204e9800998ecf8427e
eventing witness: external_audit_witness 2 c7450c3eccef20c5cac69749c875f7c8
```

## 本机 ledger 库存（只读调查）

调查了端口 15432 与 15436 上全部 50 个 `p03_%` 数据库：42 个存在相关
ledger，8 个没有 ledger。42/42 的 `20260705000100` checksum 都是冻结 b-24
`40539c...09379`；b-25 `7cd30a...376e7` 出现次数为 0。

| P03 forward SHA-384 | 数量 | 本机数据库 |
|---|---:|---|
| 本轮开始时的未发布 `6575c3...67eb7` | 6 | 这是修复前库存快照；不再是当前源码 checksum。 |
| absent（只有冻结 b-24） | 1 | `15436/p03_pgvector_final2` |
| `0f14c9...1b528` | 7 | `15432/p03_baseline_b24`, `p03_canonical_commit`, `p03_eventing`, `p03_final_canonical`, `p03_final_eventing`, `p03_upgrade_current`; `15436/p03_pgvector` |
| `303d98...325d5` | 2 | `15436/p03_canonical_verify`, `p03_eventing_verify` |
| `5436d5...3a84` | 2 | `15436/p03_repair_restore_canonical`, `p03_repair_restore_upgrade` |
| `7a4a76...f02e6` | 4 | `15436/p03_repair2_restore_canonical`, `p03_repair2_restore_upgrade`, `p03_review_final_canonical`, `p03_review_final_eventing` |
| `8ec3dd...fcd0` | 3 | `15436/p03_fix_review_canonical`, `p03_fix_review_eventing`, `p03_fix_signature3` |
| `cd2cd4...a550` | 4 | `15432/p03_final2_canonical`, `p03_final2_eventing`, `p03_mutated_upgrade_final`; `15436/p03_pgvector_final` |
| `d4519c...2f22` | 4 | `15432/p03_final3_canonical`, `p03_final4_eventing`, `p03_migration_upgrade`, `p03_mutated_upgrade_final2` |
| `fbb5e7...09ca` | 9 | `15436/p03_audit_review2_canonical`, `p03_audit_review2_eventing`, `p03_full_api_replay`, `p03_full_canonical`, `p03_full_eventing`, `p03_full_formal_commit`, `p03_hash_probe`, `p03_postreview_canonical`, `p03_postreview_eventing` |
| no ledger | 8 | `15432/p03_restore_verify_canonical_20260717`, `p03_restore_verify_eventing_20260717`; `15436/p03_baseline_reconstructed_canonical_20260717`, `p03_baseline_reconstructed_eventing_20260717`, `p03_full_identity`, `p03_full_workflow`, `p03_review_restore_canonical`, `p03_review_restore_eventing` |

这些旧 forward checksum 均是未发布修复过程中创建的本机测试库，不是远程部署
证明。本轮没有修改其 ledger；需要复用时必须删建专用测试库或从已验证备份恢复，
不能把 checksum 改成当前值。当前源码 P03 forward SQLx SHA-384 为
`a724ba30e6d271c0449e1ca5968ee3bdb20d4dd78b35d88d66362f80ad967b9634c936b97d4fe8a0b1a9330020edce1f`；
本轮专用 `15436/p03_migration_upgrade` 由测试重建后记录该 checksum，七个 migration
均由 `sqlx migrate info` 报告为 `installed`。

## 本轮修复验证中的非通过记录

- 第一次完整 package 测试命令缺少测试自身要求的
  `P02_EVENTING_RESET_DATABASE`/`P02_EVENTING_WITNESS_RESET_DATABASE`，在真实 eventing
  测试开始前 fail-closed（exit 101）；该运行未计为 PASS。补齐两个专用数据库名并换用
  全新 canonical/witness 数据库后，同一完整命令 52/52 通过。
- 本机连接在默认沙箱内会返回 `Operation not permitted`。所有声称为真实 PostgreSQL/
  NATS/Redis 的通过结果均在获批的沙箱外本机连接上重新执行；沙箱连接失败未计为 PASS。
- `WHEN(false)` 与 `SECURITY DEFINER/search_path` 两个显式漂移探针预期返回 exit 3；
  两次连接结束均回滚未提交事务，随后 canonical assertion exit 0。
- 额外执行的发布级 `python3 scripts/ci/repo_truth.py --check` 按设计因 P03 仍为
  `STAGED_AND_UNCOMMITTED` 返回 exit 1；该结果未计为 PASS。P03 提示词要求报告 Git
  状态但未授权以提交伪造 clean-tree，故 `PRODUCT_RELEASE_READY` 继续为 `NO`。

## 防伪结论

- 没有 `_sqlx_migrations` INSERT/UPDATE/DELETE。
- 没有 `#[ignore]`、测试删除、断言弱化或 `continue-on-error`。
- 冻结 migration 与 b-24 fixture `cmp` 字节一致。
- 失败、中止和 Hosted CI 未运行均被明确披露；CodeRabbit 第一轮 finding 与第二轮
  零 finding 均如实记录。
- 没有开始 P04，也没有修改任何密码。
