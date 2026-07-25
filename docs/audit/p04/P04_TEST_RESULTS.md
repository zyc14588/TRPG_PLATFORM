# P04 当前测试与防伪结果

复验日期：2026-07-26（Australia/Brisbane）
基线 HEAD：`63e708afe3560a419fe66afbb6f55dc99e79e175`

本文件是人工可读摘要。测试输出保存在当前会话的临时日志中，不把这些临时文件声明为
commit-bound 发布工件。

## P04 必需命令

| 命令 | 结果 |
| --- | --- |
| `cargo test --locked -p trpg-data-eventing --test postgres_event_store_integration --all-features -- --test-threads=1` | PASS，`1/1`，exit `0` |
| `cargo test --locked -p trpg-data-eventing --test postgres_outbox_integration --all-features -- --test-threads=1` | PASS，`1/1`，exit `0` |
| `cargo test --locked -p trpg-data-eventing --test projection_resume --all-features -- --test-threads=1` | PASS，`1/1`，exit `0` |

这些目标连接由当前 migration 初始化的真实 PostgreSQL；Event Store、Outbox claim/ACK/DLQ 和
Projection checkpoint/resume 均走持久化路径。

## 关联回归

| 门禁 | 结果 |
| --- | --- |
| `cargo test --workspace --all-features --locked --no-fail-fast -- --test-threads=1` | PASS，exit `0` |
| `cargo check --workspace --all-targets --all-features --locked` | PASS |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS |
| `cargo build --workspace --all-targets --release --locked` | PASS |
| PostgreSQL 16 schema assertion | `P05_SCHEMA_ASSERTION_OK` |
| PostgreSQL 18 migration upgrade + schema assertion | `1/1`，随后 `P05_SCHEMA_ASSERTION_OK` |
| `scripts/ci/production-security-smoke.sh` | PASS；完整生产服务图、TLS/mTLS、证书与 external-secret 轮换 |

完整 workspace 使用 PostgreSQL primary/witness/TLS 实例、Redis、NATS JetStream、MinIO、
OpenFGA 和 OPA。该结果也重新覆盖 P04 的 replay、RAG、backup/restore 和授权回归，未用 mock
服务替代网络/数据库边界。

## 首次失败不计入通过

1. 完整 workspace 在文件系统沙箱内运行时，本地 bind/connect 被操作系统以 `EPERM` 拒绝。该
   命令被终止并保留为失败；在获准访问本地测试服务后，原范围命令从头重跑并 exit `0`。
2. PostgreSQL 16 schema 检查的一次 wrapper 调用没有正确传入相对 SQL 文件；另一次因缺少
   Docker stdin 选项出现 exit `0` 但无 marker。两次均被拒绝，最终仅接受直接挂载 SQL 且输出
   精确 `P05_SCHEMA_ASSERTION_OK` 的运行。
3. PostgreSQL 18 首个官方镜像缺少 `vector` extension；后续直接执行 SQL 又缺少 SQLx ledger。
   两次都未计通过。最终使用 PostgreSQL 18.4 pgvector 镜像，先执行真实 migration upgrade
   测试，再验证精确 schema marker。

## 当前非批次门禁状态

- `repo_truth.py --check`：按设计 FAIL；当前分支不是 canonical `master` 且工作树未提交。
- 当前 patch 的 Hosted CI：`NOT_RUN`。
- 绑定不可变提交的发布证据：`NOT_GENERATED`。

这些状态阻止“产品发布完成”声明，但不否定已实际通过的 P04 批次验收。
