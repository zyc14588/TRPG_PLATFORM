# P06 测试、反伪通过与独立检查证据

记录日期：2026-07-26（Australia/Brisbane）
基线 HEAD：`b2793988c5e2e021d556635d19e8d110a99ece8a`

```text
P06_REQUIRED_COMMANDS = PASS
FULL_WORKSPACE_REAL_DEPENDENCIES = PASS
FORMAT_CHECK_CLIPPY_RELEASE_BUILD = PASS
THIRD_PARTY_PATCH_SCAN = PASS
DEPENDENCY_ADVISORY_SCAN = FAIL_DISCLOSED
HOSTED_CI_CURRENT_PATCH = NOT_RUN
COMMIT_BOUND_EVIDENCE = NOT_GENERATED
```

## P06 必需与补充命令

以下目标在固定摘要 PostgreSQL 18/pgvector、独立 witness、OpenFGA、OPA、Redis、
NATS JetStream 和 MinIO 环境中实际执行；缺少 P06 环境变量时测试 fail closed：

| 命令 | 结果 |
| --- | --- |
| `cargo test -p trpg-domain-core --test core_entities` | PASS，`4/4`，exit `0` |
| `cargo test -p trpg-data-eventing --test core_domain_schema_integration` | PASS，`1/1`，真实 primary/witness PostgreSQL，exit `0` |
| `cargo test -p trpg-runtime --test session_scene_state_machine` | PASS，`1/1`，真实并发与重连，exit `0` |
| `cargo test -p trpg-api --test campaign_character_api_integration` | PASS，`1/1`，真实 PostgreSQL/OpenFGA/OPA/audit，exit `0` |
| `cargo test -p trpg-ruleset-coc7 --test character_scenario_validation` | PASS，`3/3`，exit `0` |
| `cargo test -p trpg-domain-core --test authority_owner_integration` | PASS，`3/3`，含错误 authority mode 负例 |
| `cargo test -p api-server --test core_domain_adapter_integration` | PASS，`1/1`，生产 adapter + 真实 PostgreSQL |
| `cargo test -p trpg-data-eventing --test migration_upgrade` | PASS，`1/1` |

`psql` 对专用数据库事务式执行 `scripts/ci/assert-schema.sql`，精确输出
`P06_SCHEMA_ASSERTION_OK` 后回滚。

## 完整回归与构建

| 门禁 | 结果 |
| --- | --- |
| `cargo test --workspace --all-features --locked --no-fail-fast -- --test-threads=1` | PASS，exit `0`；包含 integration、E2E 和 doc tests |
| `cargo check --workspace --all-targets --all-features --locked` | PASS，exit `0` |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS，exit `0` |
| `cargo fmt --all -- --check` | PASS，exit `0` |
| `cargo build --workspace --all-targets --release --locked` | PASS，exit `0` |
| `python3 scripts/ci/check_dependency_directions.py` | PASS，`dependency directions: ok` |
| `git diff --check` | PASS，exit `0` |

完整 workspace 回归同时覆盖 P02 Identity/Membership、Authority immutable/fork-only、
Visibility、canonical Event/Outbox、P05 deletion/provenance/policy fail-closed 等既有不变量。
最新 capability migration 写入后，第一次在保留旧 SQLx ledger 的测试数据库上运行完整
workspace，按预期以 `MigrationChecksumMismatch` 失败，未计为 PASS、也未编辑 ledger。
随后只重建八个无 volume 的仓库测试容器（primary/witness/TLS PostgreSQL、Redis、NATS、
OpenFGA、OPA、MinIO），用固定 digest 服务从空库重跑；上表最终完整命令 exit `0`。

## 关键反伪负例

- 修复前真实复现：将合法 Character event sequence 复用于
  `character_forged_by_reused_event`，数据库接受伪造 row；该结果被记为缺陷而非 PASS。
- 修复后真实 `SET LOCAL ROLE trpg_api_service` 重放同一 event、不同 row id，trigger
  因 HMAC v3 target 不匹配而拒绝；即使攻击者知道尚未投影的精确 Campaign relation、
  row id 和 event sequence，也会因拿不到 commit-scoped secret projection capability
  而无法写入伪造 title/state/version；API role 的业务表 `DELETE` 也被 PostgreSQL 拒绝。
- projection failure injection 发生后：canonical event/outbox count 为 `1`，Campaign
  projection count 为 `0`；移除故障后同一请求恢复 projection，event count 仍为 `1`。
- Invite issue/accept、Character submit/approve、Session start/pause/resume/end、
  Scene switch 和 Reconsideration 精确重试均验证 event count 不增加；Invite token 在重试中
  稳定且不落库。
- 无新 canonical event 的 projection UPDATE、非法 Session/Scene 转换、错误
  Authority mode、用户伪造授权 context、错误 resource/type/action policy evidence 均被拒绝。
- `AuthorizedCoreApiContext` 不实现 `Deserialize`；测试 context 由真实 IdentityService
  签发的 user/workflow actors、锁定 Authority Contract 和真实 OpenFGA/OPA decision 构造。
- 生产 adapter 测试直接实例化 `api_server::core_domain::RepositoryCampaignCharacterPort`；
  lower-layer policy fixture 明确不是 policy E2E，真实 policy 证据由 `trpg-api` 测试承担。

## 真实失败与处置

以下失败均未计为 PASS：

1. 审查首先发现旧证据声称“全部同一事务”，但实现把 canonical commit 与 projection
   分成两个 transaction；证据已纠正，并增加 projection failure/exact retry 测试。
2. 复用合法 event sequence 写不同 Character row 的利用在修复前成功；仅绑定 row target
   后又发现 projection failure 窗口内，API 数据库角色可能抢占精确 target 写伪造内容。
   最终用 HMAC v3 `(relation,row_id,capability_hash)` target、commit-scoped secret
   capability、数据库 trigger 和两类真实 service-role 攻击回归关闭。
3. Invite 随机 token、expected-version 检查顺序及事件 idempotency suffix 曾导致精确重试
   失败；改为稳定派生和先识别 exact retry 后检查版本。
4. `AuthorizedCoreApiContext` 曾有 public 字段，可组合任意 actor role/mode；改为 typed
   server-side constructor，并将 mode 纳入 Authority binding。
5. P06 adapter 曾只存在于测试；新增生产 port 后用真实 PostgreSQL 运行完整基础链。
6. migration catalog fingerprint 首次在 PostgreSQL 16/18 均真实失败；按两个真实服务版本
   重新取值后才通过，没有放宽 schema assertion。
7. 严格 Clippy 首次因参数过多失败；重构为有领域含义的 route tuple 后原命令通过，没有
   添加 lint allow。
8. 首次 Semgrep focused scan 检出 P06 API 测试用可预测 temp path；改为
   `tempfile::TempDir` 后 differential scan 为 0。
9. Semgrep 扩大到整个目录时报告 6 个既有基线 finding；精确变更文件非 differential
   扫描仍报告 P05 deletion test 中 2 个未改动 temp-dir finding。两组均未伪装为 P06
   新增 finding；HEAD differential 对 28 个 P06 文件最终为 0。
10. `cargo-audit` 返回 exit `1` 和 3 个 advisory；状态保持 FAIL/DISCLOSED，详见独立复核
    报告。

P06 migration SHA-384：
`47774b27008ca0ed39188582d97edc21beb3f31ad739da688e0702854e25504e26169ab864624e584c61777f6691f424`。

## 仓库与发布证据边界

当前工作树是用户要求的未提交 P06 patch。三份 generated manifest 已在隔离临时 index 中
绑定完整 patch snapshot，并以相同 SHA-256 通过 `manifest.py --check`（3932 行）；
这不等于 commit、Hosted CI 或 release attestation。外部 CodeRabbit
需要把未提交源码发送到第三方服务，当前没有该额外授权，所以保持 `NOT_RUN`，没有用本地
结果冒充 CodeRabbit。
