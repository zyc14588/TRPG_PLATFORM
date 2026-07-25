BATCH_STATUS: COMPLETE
BATCH_ID: P04

# P04 当前验收状态

复验日期：2026-07-26（Australia/Brisbane）

```text
REVALIDATION_BASE_HEAD = 63e708afe3560a419fe66afbb6f55dc99e79e175
WORKTREE = UNCOMMITTED_INTENDED_P05_REPAIR
P04_REQUIRED_TESTS = PASS
P04_REAL_POSTGRESQL = PASS
P04_REGRESSION_IN_FULL_WORKSPACE = PASS
P04_PREREQUISITE_FOR_P06 = SATISFIED
HOSTED_CI_CURRENT_PATCH = NOT_RUN
COMMIT_BOUND_RELEASE_EVIDENCE = NOT_GENERATED
PRODUCT_RELEASE_ATTESTATION = NOT_CLAIMED
```

本状态以 2026-07-26 对当前工作树的重新执行为准，取代本目录中早期把 real
OpenFGA/OPA、TLS 或完整 workspace 标为 `NOT_RUN` 的历史结论。P04 批次验收与产品发布证明是
两个不同门禁：前者已经由当前代码和真实服务复验完成；未提交工作树、Hosted CI 和发布证据仍
如实保留为未完成，但它们不是 P04 提示词或 P06 前置检查规定的批次完成条件。

## 当前完成证据

| 验收面 | 当前结果 |
| --- | --- |
| PostgreSQL Event Store integration | `1 passed / 0 failed / 0 ignored` |
| PostgreSQL leased Outbox integration | `1 passed / 0 failed / 0 ignored` |
| Projection checkpoint/resume | `1 passed / 0 failed / 0 ignored` |
| 全 workspace、all features 回归 | exit `0`；真实 PostgreSQL、Redis、NATS JetStream、MinIO、OpenFGA、OPA |
| PostgreSQL 16 schema | `P05_SCHEMA_ASSERTION_OK`；同时覆盖既有 P02–P04 schema 契约 |
| PostgreSQL 18 upgrade/schema | migration upgrade `1/1`；`P05_SCHEMA_ASSERTION_OK` |
| 生产服务图 | API、Realtime、Agent Worker、Web、PostgreSQL、Redis、NATS、MinIO 与策略服务健康 |
| 格式、check、Clippy | 全部 exit `0`；Clippy 使用 `-D warnings` |

三个 P04 必需测试使用真实 PostgreSQL 连接，不依赖环境缺失时的 early return。完整命令和首次
失败处置记录见 `P04_TEST_RESULTS.md`。

## 防伪边界

- 未删除或忽略测试，未关闭 Event Store、Outbox、Projection、Visibility 或权限断言。
- 沙箱内本地 socket/network 的 `EPERM` 运行不计 PASS；同一完整命令在允许本地服务访问的环境
  从头重跑并 exit `0`。
- 未把 `repo_truth.py --check` 的预期失败改写为通过；当前分支不是 canonical `master`，且修复
  尚未提交。
- 未把 session-local 日志冒充 commit-bound 发布证据或 Hosted CI 结果。

因此，P04 作为 P06 的前置批次已满足；本文件不声明当前工作树已经可发布。
