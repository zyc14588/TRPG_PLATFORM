# P05 当前测试与反伪通过证据

记录日期：2026-07-26（Australia/Brisbane）
基线 HEAD：`63e708afe3560a419fe66afbb6f55dc99e79e175`

```text
LOCAL_CODE_AND_RUNTIME_GATES = PASS
HOSTED_CI_CURRENT_PATCH = NOT_RUN
COMMIT_BOUND_RELEASE_EVIDENCE = NOT_GENERATED
REPOSITORY_TRUTH = EXPECTED_FAIL_NON_CANONICAL_DIRTY_WORKTREE
```

本文件是人工可读摘要。完整 workspace 与 production smoke 日志位于当前会话临时目录，未将
`/tmp` 文件冒充提交绑定或可长期保存的发布证据。

## 必需及核心门禁

| 命令/门禁 | 结果 |
| --- | --- |
| `cargo test --locked -p trpg-security-governance --test derived_visibility_matrix --all-features -- --test-threads=1` | PASS，`4/4`，exit `0` |
| `cargo test --locked -p trpg-domain-core --test fact_provenance -- --test-threads=1` | PASS，`6/6`，exit `0` |
| normalized deletion：`cargo test --locked -p trpg-security-governance --test data_deletion_e2e --all-features -- --test-threads=1` | PASS，`8/8`，exit `0` |
| P04 Event Store/Outbox/Projection 三个精确 integration target | 各 `1/1`，exit `0` |
| `cargo test --workspace --all-features --locked --no-fail-fast -- --test-threads=1` | PASS，exit `0` |
| `cargo check --workspace --all-targets --all-features --locked` | PASS |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo build --workspace --all-targets --release --locked` | PASS，exit `0` |
| `scripts/ci/service-process-smoke.sh` | PASS；五个 Rust service 与 Web |
| `scripts/ci/production-security-smoke.sh` | PASS；完整产品图、TLS、Redis/NATS mTLS、证书轮换、external-secret 轮换 |

完整 workspace 使用真实 PostgreSQL primary/witness/TLS、Redis、NATS JetStream、MinIO、
OpenFGA 1.15.1 和 OPA，而不是 mock 全链路。session-local 完整输出分别保存在
`/tmp/p05-workspace-test-escalated.log` 与 `/tmp/p05-production-security-final.log`。

## 数据库与运行环境

| 验证 | 结果 |
| --- | --- |
| P02–P05 integration service bootstrap | PASS；服务健康、角色拓扑与 migration 完成 |
| PostgreSQL 16.14 | 直接只读挂载 assertion SQL，精确输出 `P05_SCHEMA_ASSERTION_OK` |
| PostgreSQL 18.4 + pgvector | `migration_upgrade` `1/1`，随后精确输出 `P05_SCHEMA_ASSERTION_OK` |
| Production PostgreSQL | TLS-only + SCRAM；服务身份不拥有 migration/recovery 权限 |
| Production Redis/NATS | 认证与 mTLS handshake、轮换后重连通过 |
| Secret rotation | v1/v2 external secret 与证书轮换均由 runtime smoke 真实执行 |

## 静态、治理与前端门禁

| 门禁 | 结果 |
| --- | --- |
| workflow validation | PASS |
| inventory | PASS；219 个 Rust test target、48 个 fixture、0 orphan |
| evidence schema | PASS |
| dependency direction | PASS；3 个负向 fixture 均被拒绝 |
| product boundary | PASS；5 个负向 fixture 均被拒绝 |
| P02 boundary regression | PASS；10/10 越界样例被拒绝 |
| repository-truth negative suite | PASS，19/19；使用仓库锁定 Python/Node/pnpm |
| 三份 generated manifest | PASS；从 staged 内容重生并独立校验 3916 lines |
| Compose static security contract | PASS |
| Bash syntax、ShellCheck 0.10.0、Actionlint 1.7.7 | PASS |
| `opa test policy/opa` | PASS，16/16 |
| PowerShell governance/dev-smoke parse | PASS；PowerShell 7.6.4 |
| 根 `npm test` | PASS；S12 与 Python 防伪 19/19 |
| Web `pnpm test` | PASS；5 条 ready/degraded/unavailable 行为路径 |
| Web `pnpm build` | PASS |
| `git diff --check` | PASS |

所需临时工具均安装在 `/tmp` 并校验来源摘要，没有修改系统密码或仓库工具链约束。

## 失败与假阳性拒绝记录

以下运行均未计入 PASS：

1. 完整 workspace 在沙箱内因 local bind/connect `EPERM` 失败并被终止；同一范围在允许访问本地
   服务后从头重跑才接受 exit `0`。
2. PostgreSQL 16 的第一次 schema wrapper 找不到相对 SQL；第二次因 Docker 未开启 stdin，出现
   exit `0` 但没有 marker。这正是假绿形态，最终仅接受直接挂载 SQL 且精确输出
   `P05_SCHEMA_ASSERTION_OK` 的运行。
3. PostgreSQL 18 官方镜像缺少 `vector` extension；直接灌 SQL 的替代尝试又缺少 SQLx ledger。
   两次均拒绝，最终改为 pgvector PostgreSQL 18.4 + 真实 `migration_upgrade`。
4. repository-truth 单测早期使用错误的系统 Python/Node/pnpm 而失败；最终收口时第一次虽然把
   锁定工具加入 PATH，但未设置临时 `COREPACK_HOME`，pnpm 无法验证，仍失败 `2/19`。设置
   `/tmp` Corepack/XDG 目录后，使用 Python 3.14.6、Node 24.17.0、pnpm 11.9.0 原样重跑
   `19/19`；两次失败都未删除或改写版本断言。
5. Web 测试第一次在沙箱内因 `listen EPERM` 失败；获准本地 bind 后原样重跑 5 条行为路径。
6. pnpm 首次尝试写用户目录被权限边界拒绝；改用 `/tmp` XDG/Corepack 目录，没有提升权限或修改
   用户配置。
7. production smoke 的早期运行真实暴露 NATS metadata/config 不兼容、PostgreSQL HBA/secret
   权限、服务 migration owner 以及并发 Buildx tag 竞争；逐项修复后使用单一共享 release image
   从头运行，最终才接受完整 PASS。
8. 外部提示词的旧 package 名 `trpg-privacy` 的字面命令已执行并以“不匹配 package”退出
   `101`；没有创建空壳测试或把该结果写成通过。依据当前 normalized map，真实 deletion target 在
   `trpg-security-governance` 中运行 `8/8`。

## 反伪边界

- 未新增 `#[ignore]`、测试 early return、`continue-on-error`、marker-only assertion 或放宽
  Visibility/policy/TLS gate。
- 未删除测试、未编辑既有 SQLx 成功 ledger、未让 projection/cache 取代 Event Store。
- `repo_truth.py --check` 当前仍正确失败：canonical branch 是 `master`，当前为 agent 分支，且
  工作树包含未提交修复。该失败没有被汇总成 PASS。
- 当前 patch 的 Hosted CI 和 commit-bound 发布证据仍为 `NOT_RUN/NOT_GENERATED`，不影响 P05
  批次真实性，但禁止据此宣称产品已经发布。
