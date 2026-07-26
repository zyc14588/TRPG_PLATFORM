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
HOSTED_CI_CURRENT_PATCH = NOT_RUN
COMMIT_BOUND_EVIDENCE = NOT_GENERATED
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
最终 Realtime 强化后重新执行了五条强制命令、上述回归及编译门禁；未把未执行的 Hosted
CI 或单次 monolithic workspace test 冒充为当前 patch 的发布证明。

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

提示词要求的 `git diff --name-only` 与 `git status --porcelain=v1` 也已执行；输出包含进入
P07 前必须保留的未提交 P06 patch，以及当前 P07 代码/测试/证据。范围审计确认没有 P08
Combat/Chase/Fork/Reconsideration/Ending/Growth 的新实现；真实暂存区为空。

三份 generated source manifest 使用隔离临时 Git index/object database 绑定当前完整
P06+P07 patch，真实用户 index 未改变；`manifest.py --check` 验证为 `3945` 行，三份文件
SHA-256 相同。该 snapshot 仍不等于 commit-bound 或 Hosted CI evidence。

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
`590d03cd0df1df8ff07a3fc5bd5c28b30027d526f662c0a31ec7c7eae0056c1b283c59855a482cb28bf6cdd7c25a3427`。
