# P07 测试、失败基线与验收证据

记录日期：2026-07-27（Australia/Brisbane）
基线 HEAD：`b2793988c5e2e021d556635d19e8d110a99ece8a`

```text
P07_REQUIRED_COMMANDS = PASS
P07_REAL_HTTP_POSTGRES_POLICY_JETSTREAM = PASS
P07_SCHEMA_ASSERTION = PASS
SCOPED_PREREQUISITE_REGRESSION = PASS
FORMAT_CHECK_CLIPPY_RELEASE_BUILD = PASS
THIRD_PARTY_LOCAL_DIFF_SCAN = PASS_0_FINDINGS
DEPENDENCY_ADVISORY_SCAN = FAIL_3_DISCLOSED_PREEXISTING
P07_IMPLEMENTATION_COMMIT = 6657d90a47110e3df4ce4f0e53f1e78e2b661a4c
P07_REVIEW_REMEDIATION_COMMIT = eb02b24d8d2a422dc70d0d2e5052b3f0267431c1
HOSTED_CI_INITIAL_IMPLEMENTATION_COMMIT = FAIL_PG_DUMP_16_SERVER_18
CI_REPAIR_LOCAL_BACKUP_RESTORE = PASS_POSTGRESQL_18_4
CI_REPAIR_THIRD_PARTY_SCAN = PASS_0_FINDINGS
HOSTED_CI_CI_REPAIR_COMMIT = PASS_ALL_REQUIRED_CHECKS
GITHUB_CODEX_REVIEW = 5_P1_FIXED_REPLIED_RESOLVED
FINAL_FULL_WORKSPACE_TEST = PASS
FINAL_THIRD_PARTY_SCAN = PASS_0_NEW_FINDINGS_4_PREEXISTING_INFO
HOSTED_CI_FINAL_HEAD_POLICY = MUST_PASS_BEFORE_MERGE
```

## 修改前真实基线

在实现前逐一运行 P07 提示词规定的五条命令，五个命令均因测试目标不存在而返回
Cargo exit `101`，分别缺少：

- `player_action_http_integration`
- `human_kp_investigation_flow`
- `server_dice_and_sanity_sequence`
- `decision_state_outbox_atomicity`
- `vertical_human_kp_tutorial_slice`

这些失败用于确认 `AUD-006/008/013/030/042` 尚未闭合，没有被计为通过或从证据中删除。

## 五条强制命令

最终在固定摘要、隔离命名的 primary/witness PostgreSQL、OpenFGA、OPA 与 NATS
JetStream 上按提示词原样执行：

| 命令 | 结果 |
| --- | --- |
| `cargo test -p trpg-api --test player_action_http_integration` | PASS，`1/1`，exit `0`；真实 TCP HTTP、Identity、Authority、OpenFGA/OPA、PostgreSQL、生产 Outbox publisher 与 JetStream ACK |
| `cargo test -p trpg-runtime --test human_kp_investigation_flow` | PASS，`2/2`，exit `0` |
| `cargo test -p trpg-ruleset-coc7 --test server_dice_and_sanity_sequence` | PASS，`4/4`，exit `0` |
| `cargo test -p trpg-data-eventing --test decision_state_outbox_atomicity` | PASS，`1/1`，exit `0`；含 investigation、SAN 与 fault injection |
| `cargo test -p trpg-testing --test vertical_human_kp_tutorial_slice` | PASS，`2/2`，exit `0` |

HTTP 测试验证返回的 `realtime_delta_id` 精确映射到末 event sequence；同一 sequence 的
Outbox 行携带 `party_visible`、Correlation、Trace 与 Fact Provenance，生产 publisher
收到 JetStream ACK 后标记 `published_at`，订阅端收到对应 `DecisionCommitted` canonical
envelope。它不是只断言内存 helper 或合成字符串。

## 前置不变量回归

| 命令 | 覆盖 | 结果 |
| --- | --- | --- |
| `cargo test -p trpg-domain-core --test authority_owner_integration` | Authority owner、mode immutable、跨 Campaign 与 stale version | PASS，`3/3` |
| `cargo test -p trpg-domain-core --test core_entities` | Character 生命周期、版本事件、Reconsideration append-only | PASS，`4/4` |
| `cargo test -p trpg-security-governance --test derived_visibility_matrix` | Visibility audience intersection、禁止放宽 | PASS，`4/4` |
| `cargo test -p trpg-data-eventing --test canonical_commit_postgres` | Event/Audit/Outbox 原子性、幂等、独立 witness/recovery | PASS，`4/4` |
| `cargo test -p trpg-api --test campaign_character_api_integration` | 真实 Repository、OpenFGA/OPA、角色卡版本历史 | PASS，`1/1` |
| `cargo test -p api-server --bin api-server --locked` | Player Action 写入开关默认/启用/停写/非法值 fail-closed | PASS，`1/1` |

此外，本轮对受影响 crate 的完整测试集合分组执行并通过，包括
`trpg-agent-runtime`、`trpg-runtime`、`trpg-ruleset-coc7`、`trpg-domain-core`、
`trpg-api`、`trpg-data-eventing`、`trpg-testing` 与 `api-server` 的相关 target。
最终 Realtime 强化后重新执行了五条强制命令、上述回归及编译门禁；没有把局部测试或
第三方扫描冒充为 Hosted CI/发布证明。

## 发布后托管 CI 修复

P06/P07 实现发布为 commit
`6657d90a47110e3df4ce4f0e53f1e78e2b661a4c` 后，PR #8 的 `workspace-ci` 与
`release-readiness-evidence` 均在“Start pinned integration services”失败。作业日志的
共同根因是 runner 的 `pg_dump 16` 连接固定 PostgreSQL 18 主服务；产品测试尚未开始，
不是业务断言失败。

修复不改变数据库服务版本，也不放松版本门禁：宿主 `pg_dump`/`pg_restore` 只有均与主库
同主版本时才使用，否则以主库同一 digest-pinned `pgvector` 镜像执行客户端。封装容器为
只读 root filesystem、drop all capabilities、`no-new-privileges`、受限 PID，并只挂载
每次运行新建的专用临时目录。提交前已实际验证：

| 命令/场景 | 结果 |
| --- | --- |
| 容器封装 `pg_dump --version` / `pg_restore --version` | PASS，均为 PostgreSQL `18.4` |
| 对 P07 真实数据库执行 custom dump 与 `pg_restore --list` | PASS |
| 路径逃逸、父目录逃逸、未固定镜像负例 | PASS，均 exit `2` |
| `cargo test --locked -p trpg-ops --test postgres_backup_restore_integration` | PASS，`1/1`；独立恢复与重复恢复 |
| `cargo test --locked -p trpg-data-eventing --test postgres_event_store_integration` | PASS，`1/1`；两次灾备重建与 hash 一致 |
| ShellCheck `0.10.0`（仓库固定校验和） | PASS |
| Semgrep `1.171.0`，4 个 CI 差异文件、3 条适用规则 | PASS，0 finding/0 error |

该 CI repair commit 的五项受保护 Hosted CI 随后全部通过；最终合并仍以包含后续 P1
修复、审计文档与 manifests 的最新 PR head 为准。

## PR 审查阻断修复

CI repair commit `3fa341f988171b15cbfb13dc345e95622ce07882` 的五项受保护检查
全部通过后，GitHub Codex 对 PR #8 提出五个 P1 阻断。它们已由
`eb02b24d8d2a422dc70d0d2e5052b3f0267431c1` 修复，并在远端线程逐条附证据后全部
标记为 resolved：

| P1 | 修复 | 关键验证 |
| --- | --- | --- |
| 正式授权晚于 Agent 工具执行 | OpenFGA/OPA formal authorization 完成后才允许 canonical lookup 或 executor | denial regression 证明 executor 调用数为 `0` 且 Event 为 `0`；Agent suite `6/6` |
| exact/cold retry 先重复执行非幂等工具 | Runtime/Agent 在 executor 前读取受 canonical custody 验证的 receipt；未命中时 PostgreSQL 先预检 expected version | exact/cold retry executor 始终为 `1` 并复用原 durable identities；真实 `canonical_commit_postgres` `4/4` |
| 邀请过期时间由客户端控制 | 删除 issue/accept 客户端时钟字段，DTO 拒绝未知字段，Repository 使用注入的可信服务端时钟 | 伪造旧字段反序列化失败；过期首次接受失败；真实 API/DB 集成通过 |
| 新 request hash 破坏旧零 projection retry | 只对所有 projection target 为空的历史请求接受 legacy hash，并在 receipt/race/witness 路径使用已接受的 stored hash | legacy 零 target 接受、有 target 拒绝的单测通过；完整工作区通过 |
| 邀请 Event 先提交、membership 冲突后失败 | `CampaignInviteAccepted` 与 membership INSERT 经 capability/policy/event 绑定的 `SECURITY DEFINER` function 在同一 canonical transaction 提交 | revoked/不同角色冲突时 Event=`0`、formal commit=`0`、旧 membership 不变；schema 最小权限断言通过 |

修复后重新运行
`cargo test --workspace --all-features --locked --no-fail-fast -- --test-threads=1`，
所有 unit、integration 与 doc tests 通过；其中包含真实 PostgreSQL、OpenFGA/OPA、
Redis、NATS、备份恢复、迁移升级、隐私与安全门禁。最终 Hosted CI 仍以包含审计文档与
generated manifests 的 PR head 为准，不以本地结果替代。

## Schema、构建与结构门禁

| 门禁 | 结果 |
| --- | --- |
| 容器内 `psql -X -v ON_ERROR_STOP=1 ... < scripts/ci/assert-schema.sql` | PASS；事务内输出 `P06_SCHEMA_ASSERTION_OK`、`P07_SCHEMA_ASSERTION_OK` 后 `ROLLBACK` |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace --all-targets --all-features --locked` | PASS |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS |
| `cargo build --workspace --all-targets --release --locked` | PASS |
| `python3 scripts/ci/check_dependency_directions.py` | PASS，`dependency directions: ok` |
| `python3 scripts/ci/discover_tests.py --check` | PASS，发现 `230` 个 Rust test targets |
| `python3 scripts/ci/verify_test_inventory.py --report /tmp/p07-test-inventory.json` | PASS，`49` fixtures、`0` orphan |
| `git diff --check` | PASS |

提示词要求的 `git diff --name-only` 与 `git status --porcelain=v1` 也已执行。P06/P07
实现已绑定上述实现 commit；CI follow-up 的范围审计确认没有 P08
Combat/Chase/Fork/Reconsideration/Ending/Growth 的新实现。

三份 generated source manifest 绑定完整 P06/P07、CI repair 与五项 P1 修复 patch；
最终 `manifest.py --check` 验证为 `3947` 行且三份文件 SHA-256 相同。manifest 本身
不等于 Hosted CI 或发布签署。

宿主机直接执行 `psql` 曾真实返回 `127`（客户端未安装），该次未计为 PASS；随后使用固定
PostgreSQL 容器内置客户端执行完全相同的断言文件并通过。Semgrep 首次并行运行因
`io_uring_queue_init` 资源错误 exit `2`、扫描集合为空，也未计为 PASS；单并发重跑成功，
详见第三方报告。

## 依赖审计边界

`cargo audit 0.22.2 --no-fetch` 对最终 `Cargo.lock` 返回 exit `1`，报告三个已在 P06
披露的既有 advisory：

- `RUSTSEC-2026-0194`、`RUSTSEC-2026-0195`：
  `trpg-security-governance -> rust-s3 0.37.2 -> aws-creds -> quick-xml 0.38.4`
- `RUSTSEC-2023-0071`：`rsa 0.9.7` 仅残留于 lockfile，当前 Linux all-features 图不可达

P07 没有新增上述依赖，也没有把该扫描写成 PASS；它们仍是 release security gate 的既有
风险，不是本批次五个 AUD 的修复证据。

P07 migration SHA-384：

- `20260727000100_create_player_action_decision_state.sql`：
  `590d03cd0df1df8ff07a3fc5bd5c28b30027d526f662c0a31ec7c7eae0056c1b283c59855a482cb28bf6cdd7c25a3427`
- `20260727000200_harden_campaign_invite_acceptance.sql`：
  `8f796c71a4ee6c567ec54f479cd852b3095bb072ee16dd2947c0928355718787ad11c2207abdbf8450626b4414567312`
