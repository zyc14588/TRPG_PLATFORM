# P05 测试与防伪证据

记录日期：2026-07-24（Australia/Brisbane）

```text
BASE_HEAD = fb6e146612e4df66a508292245da6b995bbe64fb
AVAILABLE_LOCAL_TECHNICAL_GATES = PASS
STRICT_BATCH_ACCEPTANCE = BLOCKED | PARTIAL_NOT_ACCEPTED
HOSTED_CI = NOT_RUN
PRODUCTION_COMPOSE_TLS_MTLS = NOT_RUN
CLEAN_P05_ONLY_SCOPE = NOT_PROVABLE
```

所有 raw log 保存在：

```text
/tmp/codex-security-scans/TRPG_PLATFORM/fb6e1466_20260723T132131Z/artifacts/
```

## 最终结果

| 门禁 | 最终结果 | 可核验事实 |
| --- | --- | --- |
| Workspace tests | PASS, exit 0 | 688 `ok`、0 failed、0 ignored、275 个 result blocks |
| Strict clippy | PASS, exit 0 | workspace、all targets、all features、`-D warnings` |
| Format / whitespace | PASS | `cargo fmt --all -- --check`、`git diff --check` |
| Release build | PASS | workspace 全部 bins，release profile |
| Service process smoke | PASS | 5 个 release 服务和 web；readiness、liveness、EOF、404 均验证 |
| Schema assertion | PASS | 输出 `P05_SCHEMA_ASSERTION_OK` |
| PostgreSQL login-role probes | PASS | 4 个允许操作成功、6 个越权操作均得到预期 permission error |
| Repository static gates | PASS | dependency/product boundaries 及负向测试、workflow、manifest、evidence schema、test inventory |
| OPA | PASS | `PASS: 16/16` |
| Release readiness | BLOCKED（预期） | `MISSING_CURRENT_EVIDENCE`、`DIRTY_WORKTREE` |
| Repository truth | FAIL（诚实阻塞） | `worktree is not clean` |
| Production TLS/mTLS | NOT RUN | Docker/证书不可用；TLS 测试环境变量未设置 |

## 最终 workspace 命令

真实服务环境使用相互独立的 PostgreSQL 18.4 primary/witness 测试库，以及：

```text
Redis          redis://127.0.0.1:56379/
NATS           nats://127.0.0.1:54222
MinIO          http://127.0.0.1:59000
OpenFGA        127.0.0.1:58080
OPA            127.0.0.1:58082
OPA revision   opa-security-governance-v3
```

执行 argv：

```bash
cargo test --workspace --all-features --locked --no-fail-fast -- --test-threads=1
```

为降低磁盘峰值，仅设置 `CARGO_PROFILE_TEST_DEBUG=0`、`CARGO_INCREMENTAL=0` 和
`CARGO_BUILD_JOBS=2`；没有移除 feature、package、test target 或负向用例。最终日志：

```text
p05_repair_workspace_test_final.log
sha256 b7d90a57b08739ace954e126bd01baa20de2a1747053ac19e0dfd90f28d8576b
```

该日志中的 `postgres_tls_integration` 因未设置 `P02_TLS_DATABASE_URL`/CA 路径而提前返回。它由
libtest 显示为 `ok`，但本报告将 production TLS 明确记为 `NOT RUN`，不把这个返回外推为通过。

## 真实依赖与关键负向路径

| 范围 | 最终验证 |
| --- | --- |
| Canonical primary/witness | 原子 commit、独立 witness、恢复、错误密钥、篡改、幂等和全元数据绑定 |
| OpenFGA + OPA | 合法成员 permit、非成员 deny、服务不可达 fail closed、动作词汇一致 |
| Deletion | 真实 PostgreSQL/RAG/Redis/NATS/MinIO/filesystem/export/backup-key 删除与逐面 absence proof |
| Privacy authorization | 跨 Campaign 删除拒绝、job existence oracle 不透明、失败 commit 不留 pending job |
| Cloud egress | consent/notice/revocation、精确 bytes、endpoint/model/credential/route/revision 绑定 |
| RAG/Replay | live membership、unscoped multi-campaign 拒绝、内容/embedding/derivation receipt 绑定 |
| Plugin/tool | manifest 分类、host 派生 visibility、ToolResult 只能在成功执行后铸造、完整 grant tuple |
| Provider/secret | 远端伪装 local 拒绝、Level 4 签名认证、持久撤销、无静默 cloud fallback |
| Backup/restore | PostgreSQL 18 常规二进制、独立空目标、表计数一致、篡改 manifest 拒绝 |

## 服务进程 smoke

执行 `scripts/ci/service-process-smoke.sh`，传入上述真实服务地址并使用 release binaries。脚本创建
0700 临时 secret mount/catalog/export 目录、0600 secret 文件，只向进程传 secret ID/version，
不传 `*_HEX` material。启动顺序为 migration runner 先完成迁移，再启动：

```text
api-server
realtime-server
agent-worker
admin-server
migration-runner
web
```

最终输出：

```text
service process smoke: 5 services and web passed
```

日志：

```text
p05_repair_service_smoke_final.log
sha256 b506dcf504046148db91e0cdc143e231adfa18343ec959366e5d0cafe3fbefb4
```

## 数据库验证

- 在由当前 migration 创建且尚未被故障注入测试篡改的 canonical 数据库上运行
  `psql -X -v ON_ERROR_STOP=1 -f scripts/ci/assert-schema.sql`，得到
  `P05_SCHEMA_ASSERTION_OK`。
- 登录角色探针验证：
  - API 可读 users；
  - canonical role 可读 Event Store；
  - worker 可读 deletion jobs；
  - realtime 可读 projection；
  - API 不能 append canonical、销毁 subject key 或 assume canonical role；
  - canonical role 不能读 users；
  - worker/realtime 不能 append canonical。
- schema assertion raw log：
  `p05_repair_schema_assertion_final.log`
  (`sha256 5d42d23d5fe56f26e5fcca6346ae0e35b8d505a5096e164f296cb465443ace04`)。
- 角色探针 raw log：`p05_repair_login_role_probes.log`。

## 静态门禁

以下均以正确的 Node 24.17.0 / pnpm 11.9.0 工具链运行并通过：

```bash
python3 scripts/ci/check_dependency_directions.py
python3 scripts/ci/test_dependency_directions.py
python3 scripts/ci/check_product_boundaries.py
python3 scripts/ci/test_product_boundaries.py
python3 scripts/ci/test_repo_truth.py
python3 scripts/ci/validate_workflows.py
python3 scripts/ci/discover_tests.py --check
python3 scripts/ci/verify_test_inventory.py
python3 scripts/ci/manifest.py --check
python3 scripts/ci/verify_evidence_schema.py
opa test policy/opa
```

结果包括 219 个 Rust test targets、48 个 fixture、0 个 orphan fixture、manifest 3856 行、
evidence schema 拒绝历史假 PASS、OPA `PASS: 16/16`。

严格 lint：

```bash
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

最终日志 `p05_repair_clippy_final.log`，SHA-256：
`66edcbce6e22f47a3505a87ce172fb70d47afd845084d9863d716cdb7833b66e`。

## 保留的真实失败—修复—复测链

以下失败均保留 raw log，没有删除测试或弱化 gate：

1. deletion E2E 未配置真实依赖时失败；启动隔离依赖后才通过。
2. 初次 workspace 运行发现 canonical wrong-key recovery 可污染 witness；修复为读取/恢复/提交/
   witness append 均在锁内重新校验。
3. policy 测试因错误 revision 返回 `PolicyEvidenceUntrusted`；使用已加载的真实
   `opa-security-governance-v3` 后通过。
4. 故障注入后的数据库使 schema assertion 正确失败；在干净迁移数据库重跑才得到成功 marker。
5. service smoke 首次因缺少 mounted secret boundary 失败；脚本改为真实 mount/catalog 后继续。
6. 第二次 service smoke 发现 Redis namespace 使用 `:` 却被通用 ID validator 拒绝；新增专用
   namespace validator，并让真实 deletion E2E 使用生产 namespace 后通过。
7. 手工传入带 `http://` 的 OpenFGA 地址被 API 正确拒绝；改用实现约定的 `SocketAddr` 后通过，
   未放宽解析器。
8. 最终 workspace 重编译第一次因磁盘写满失败；只清理 24.7 GiB 可重建 `target/debug` 产物，
   限制编译峰值后重跑。
9. backup gate 拒绝 `/usr/bin/pg_dump` symlink；改用
   `/usr/lib/postgresql/18/bin/pg_dump` 常规二进制。非空目标恢复仍正确失败，清空专用测试目标后
   完成真实恢复和篡改拒绝。

对应日志包括：

```text
p05_repair_workspace_test_disk_full.log
p05_repair_workspace_test_backup_path_failure.log
p05_repair_service_smoke_initial.log
p05_repair_service_smoke_second.log
p05_repair_schema_assertion.log
p05_repair_schema_assertion_clean.log
p05_repair_backup_restore_final.log
```

## 明确未执行

- 生产 Docker Compose TLS/mTLS、external secret 和证书轮换；
- Hosted CI；
- 本次最终工作树的 CodeRabbit review；
- 干净 P05-only checkpoint；
- 与干净 commit 绑定的完整 `scripts/ci/test-all.sh` 发布证据。

这些项目使严格状态保持 `BLOCKED | PARTIAL_NOT_ACCEPTED`。
