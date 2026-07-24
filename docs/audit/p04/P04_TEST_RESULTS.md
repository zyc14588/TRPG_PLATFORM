# P04 测试与防伪证据

验证日期：`2026-07-21`
基线 HEAD：`fb6e146612e4df66a508292245da6b995bbe64fb`
范围：当前未暂存、未提交工作树

```text
LOCAL_P04_TECHNICAL_REPAIR = PASS
MACHINE_EVIDENCE = PASS_CURRENT_WORKTREE_P00_4
CODERABBIT_FINAL_REVIEW = PRIOR_FIX_CYCLE_PASS_NOT_RERUN_AFTER_ADDITIONAL_REPAIR
HOSTED_CI = NOT_RUN
FULL_WORKSPACE_EXTERNAL_RUNTIME = NOT_CLAIMED
PRODUCT_RELEASE_READY = NO
```

本文件是 `HUMAN_AUTHORED_SUMMARY`，不是 raw log，也不以 Markdown 自述产生 PASS。预验证
manifest 已重跑完整矩阵并通过 live-context；正式冻结文件位于
`/tmp/p04-final-evidence-20260721/p04-data-eventing.json` 与
`/tmp/p04-final-evidence-20260721/p04-backup-restore.json`，同目录包含各自的 raw log、JUnit 与
SARIF。

## 数据/事件完整真实服务矩阵

服务：PostgreSQL 18.4 独立主库/见证库、NATS Server 2.10.27 JetStream、Redis 8.0.5。
测试只允许重置显式授权、名称精确匹配的本机专用数据库。

```text
env \
  P02_CANONICAL_DATABASE_URL=<primary-canonical-url> \
  P02_CANONICAL_WITNESS_DATABASE_URL=<witness-canonical-url> \
  P02_CANONICAL_ALLOW_DATABASE_RESET=1 \
  P02_CANONICAL_RESET_DATABASE=p02_canonical_p04complete \
  P02_CANONICAL_WITNESS_RESET_DATABASE=p02_canonical_p04complete_witness \
  P02_EVENTING_DATABASE_URL=<eventing-primary-url> \
  P02_EVENTING_WITNESS_DATABASE_URL=<eventing-witness-url> \
  P02_EVENTING_ALLOW_DATABASE_RESET=1 \
  P02_EVENTING_RESET_DATABASE=p04_eventing \
  P02_EVENTING_WITNESS_RESET_DATABASE=p04_eventing_witness \
  P02_NATS_URL=nats://127.0.0.1:24222 \
  P02_REDIS_URL=redis://127.0.0.1:26379 \
  P03_ALLOW_DATABASE_RESET=1 \
  P03_DATABASE_URL=<migration-upgrade-url> \
  P04_ALLOW_DATABASE_RESET=1 \
  P04_DATABASE_URL=<p04-primary-url> \
  P04_WITNESS_DATABASE_URL=<p04-witness-url> \
  P04_ADMIN_DATABASE_URL=<postgres-admin-url> \
  P04_RECOVERY_DATABASE_URL=<dedicated-recovery-url> \
  P04_PG_DUMP=/usr/lib/postgresql/18/bin/pg_dump \
  P04_PG_RESTORE=/usr/lib/postgresql/18/bin/pg_restore \
  cargo test --locked -p trpg-data-eventing --all-features -- --test-threads=1
```

最终重跑 exit `0`：17 个 test binaries，**66 passed、0 failed、0 ignored**；doc-tests 0。

```text
20+3+3+8+5+5+4+1+4+5+1+1+1+1+2+1+1 = 66
```

覆盖真实 canonical transaction/independent witness、JetStream ACK、Redis cache、旧库升级、
Event Store 与 leased Outbox、Projection checkpoint、pgvector RAG，以及嵌入
`postgres_event_store_integration` 的 PostgreSQL custom-format backup—destroy—restore—rebuild
drill。Canonical integration 在新增 reset guard 后连续两次 targeted PASS，再进入完整矩阵 PASS，
证明结果不依赖空库偶然性。

## 独立备份恢复门禁

除 66 项矩阵内嵌的恢复 drill 外，另有一个属于 `trpg-ops` 的独立 runbook 门禁。最终使用
PostgreSQL 18.4 的真实 `pg_dump`/`pg_restore`、libpq service file、源库和独立空目标库执行：

```text
env \
  P02_PG_DUMP=/usr/lib/postgresql/18/bin/pg_dump \
  P02_PG_RESTORE=/usr/lib/postgresql/18/bin/pg_restore \
  P02_LIBPQ_SERVICE_FILE=<dedicated-service-file> \
  P02_BACKUP_SOURCE_SERVICE=p04_source \
  P02_BACKUP_TARGET_SERVICE=p04_target \
  P02_BACKUP_SOURCE_URL=<source-url> \
  P02_BACKUP_TARGET_URL=<independent-target-url> \
  P02_BACKUP_DIR=<private-output-directory> \
  cargo test --locked -p trpg-ops \
    --test postgres_backup_restore_integration --all-features -- --test-threads=1
```

结果 **1 passed、0 failed、0 ignored**：生成 custom-format archive 与 SHA-256 manifest，恢复到
独立数据库，核对 `event_store`、`event_outbox`、`canonical_audit_log`、`formal_commits` 行数，并
拒绝被篡改的 manifest。

## 关联修复门禁

| 门禁 | 当前结果 |
| --- | --- |
| Agent Worker | PASS 4/4；pending、stale、stopped/panic、cycle error 均 fail closed |
| Identity | PASS：15 unit、1 PostgreSQL persistence、1 Redis distributed limit；TLS NOT_RUN |
| API canonical replay | PASS 1/1；7 个事件，含权威 group member、spectator、player、keeper filtering |
| Durable workflow PostgreSQL | PASS 1/1；process reconstruction 后 state/lease 仍可恢复 |
| Shared Kernel + Domain Core + Agent Runtime + Extension SDK + Agent Worker | PASS；关联 package tests 通过 |
| Workspace fmt/check/Clippy | PASS；Clippy 使用 `-D warnings` |
| Repository static components | PASS；210 Rust targets、48 fixtures、0 orphan，dependency/product/workflow 均通过 |
| Repository truth clean-worktree gate | EXPECTED BLOCK；当前修复工作树未提交，`repo_truth.py --check` 正确返回 `worktree is not clean` |
| Evidence schema/tests | PASS；schema 防历史 PASS，repo-truth 11/11（固定 Node 24/pnpm PATH） |
| CodeRabbit uncommitted review | PRIOR_FIX_CYCLE_ONLY；首轮 3 findings、当时修复后 0；追加修复后未重跑 |

Identity 的 `postgres_tls_integration` 因缺少 `P02_TLS_DATABASE_URL` 由既有 guard 提前返回；即使
Cargo 输出 `ok`，本报告仍将 TLS 标记为 NOT_RUN。

## Schema 与 migration

```text
20260717000100 SHA-256       730da5a67fdb64e60898dee3ad911e0758261026d64afe5d414078e02cd69024
20260717000100 SQLx SHA-384  d67991333d4d9e06b5c1c51a9f3c17855bddfbb64558873f4821e7338700981a98fe78f1c05441244c95e5010e156adc
20260721000100 SHA-256       bdf7831cbd46da473502f35b791f73a7e58ab834bc334b1d8eb37d83fdbd9f2f
20260721000100 SQLx SHA-384  89f3a6b7554f9ab392ac0d5f9c06e3d886de9e607415e71fe34585439fee3c82c3580aa8494ffae153533683546163e9
constraint_signature        74abd245d298fcbc86ffaef6ab33a216
trigger_signature           3d0d5cdb9fbaa52551125b4a0c935bcb
trigger_function_signature  55817f97d554378f6cc4bd789115ebf5
```

- `sqlx migrate info`: 9 个 migration installed。
- 重复 `sqlx migrate run`: exit `0`，无输出，真实 no-op。
- `assert-schema.sql`: `P04_SCHEMA_ASSERTION_OK`。
- 当前正史/安全 migration 不执行 down；应用回滚保留 schema/history，并分别用 forward apply、
  replay 与独立 `trpg-ops` backup/restore 门禁验证恢复。

## 关键负向证明

| 原风险 | 当前证明 |
| --- | --- |
| worker 已死但 health 绿色 | 首轮未完成、30 秒 stale、线程 stopped/panic、当前 cycle failure 都返回不健康 |
| 任意 Player 自称 GroupMember | Identity 只按持久 live tuple 授权；self-grant、wrong group、revoke 后访问被拒绝 |
| Unicode 键跨 Rust/PostgreSQL 漂移 | PostgreSQL C collation canonicalizer 与 serde_json 嵌套 Unicode 字节相等 |
| Projection 接受伪 hash | 数据库从 Event Store 重算 hash v3；伪 document/hash/checkpoint 均被 trigger 拒绝 |
| RAG 并发代际混合 | repository 与 raw insert 共用 transaction advisory lock；无 mixed model/dimension |
| 测试残留数据制造不稳定结果 | canonical integration 必须显式授权并精确匹配本机专用数据库，每次重置 schema |
| 摘要型证据可冒充 raw PASS | p00-4 校验环境/服务/output byte/JUnit/worktree，并支持 live-context 重验 |

## 首次失败不计入 PASS

1. 按旧报告直接重跑时，RAG row decoder 与 contract test 访问 `RagSnapshotChunk` 私有字段，
   `trpg-data-eventing` 无法编译。生产 decoder 改为 validated persisted DTO，测试改用 accessor，
   之后才取得 66/0 与 workspace check/Clippy PASS。
2. 迁移 upgrade 先因旧 SHA-384 失败，再因旧 function signature 失败；从 PostgreSQL 实际值更新
   后才通过。
3. 完整矩阵首次暴露 canonical test 残留数据 HMAC mismatch；修复测试隔离后从头重跑。
4. 冻结最终证据时曾过度删除 `P04_*` 环境，`postgres_event_store_integration` 按设计在 reset
   guard 处 fail closed。源代码核对确认该 test 内嵌 backup—destroy—restore drill；恢复其实际
   读取的 P04 专用数据库与 PostgreSQL executable 变量后从头重跑。
5. Evidence 单测首次在 Node 22 且无 pnpm 的 PATH 下失败 2/11；使用仓库锁定的 Node 24.17.0
   与 pnpm 11.9.0 后重跑为 11/11，没有放宽版本校验。
6. 机器证据首次因 Redis 8 binary 的动态库探针环境缺失而以 integrity exit `86` 失败；显式绑定
   真实 `LD_LIBRARY_PATH` 后从头重跑并通过，没有删除 service-version probe。
7. CodeRabbit 首轮复审返回 3 项；稳定命名、distinct reset identity 与 non-empty database name
   修复并重验后，当时的第二轮审查返回 0 findings；追加修复后未把旧结论写成新审查。
8. 没有把 dirty-worktree truth gate、TLS early-return、未配置 real OpenFGA/OPA、Hosted CI 或完整 workspace 外部运行矩阵
   写成 PASS。

## 防伪边界

- 本轮未新增 `#[ignore]`、测试 early return、`continue-on-error` 或 lint allow；已删除本轮新增的
  RAG lint allow。
- 未删除测试、弱化 policy/visibility gate、编辑 SQLx ledger 或删除 Event Store 历史。
- 环境 URL 在人类报告中脱敏；最终 machine manifest 只保存被选择环境值的 SHA-256，不泄露值。
- `LOCAL_P04_TECHNICAL_REPAIR=PASS` 不等价于已提交、Hosted CI 成功或产品可发布。
