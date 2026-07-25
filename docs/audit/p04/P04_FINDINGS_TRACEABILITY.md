# P04 检查发现与权威追溯

日期：`2026-07-21`
基线 HEAD：`fb6e146612e4df66a508292245da6b995bbe64fb`

## 身份边界

`P04` 是本轮检查标签，不是 `batches/B###.md` 中的 batch，也不是
`codex-prompts/**/P####.md` 中的 prompt。当前 normalized overlay 中不存在名为 P04 的
primary implementation prompt。因此本轮修复只映射到已有 owner；不存在精确 owner 的文件会
明确标注，而不会补造 Prompt ID。

S03 的真实输入是 B024–B028，共 107 行：38 个 primary implementation、46 个
supplemental requirement、23 个 documentation/traceability。Supplemental 行只约束其
primary owner，没有独立创建 Rust 输出。

## 七项检查发现

| ID | 检查发现 | 权威 owner / 边界 | 修复面 | 可执行证明 |
| --- | --- | --- | --- | --- |
| P04-F01 | 后台线程尚未完成首轮、超过时限、退出或 panic 时，health 仍可能为绿 | `CODEX-0062-06-DATA-EVENTING-09d943908d` / `codex-prompts/06-data-eventing/P0006.md` 拥有 `outbox_projection_workers`；`apps/agent-worker` 没有精确 normalized output owner，它是既有 P02 repair #7 的生产组合根 | `apps/agent-worker/src/main.rs` | `background_health_rejects_pending_stale_and_stopped_workers`、cycle error 与 delivery/projection 独立失败测试 |
| P04-F02 | `private_to_group` 只比较调用方 principal，缺少权威、持久、可撤销的群组成员关系 | `CODEX-0022-01-FOUNDATION-78879e4006` / Foundation P0008 拥有共享 Visibility 语义；Identity/API 文件无精确 normalized owner，属于既有 P02 replay repair #3 的闭环补强 | 新 group/membership migration，`trpg-identity` live grant/revoke，API replay | Identity 15 unit；真实 PostgreSQL restart/revoke；API 7-event replay；player self-grant 与跨组访问拒绝 |
| P04-F03 | PostgreSQL canonical JSON 的对象键排序未固定为 bytewise C collation，Unicode 跨实现一致性没有证明 | `CODEX-0595-06-DATA-EVENTING-f8fc21553c` / P0020；`CODEX-0606-06-DATA-EVENTING-96df5cfdb1` / P0031；`CODEX-0625-06-DATA-EVENTING-181b11b4cd` / P0050 | migration 中 `COLLATE "C"`；数据库函数签名与 checksum 更新 | `projection_checkpoint_resumes_monotonically_with_stable_protected_hashes` 比较 serde_json 与 PostgreSQL 的中日韩、重音组合、emoji 嵌套对象输出；schema assertion 通过 |
| P04-F04 | PASS manifest 未绑定实际环境、服务版本命令、stdout/stderr 字节与真实 Cargo/JUnit 明细，存在摘要伪造空间 | CI 证据脚本不在 normalized map 的精确 product output 中；本项是用户要求的验收基础设施修复，不冒充 `trpg-testing` 输出 | `generate_evidence.py`、`repo_truth.py`、schema 与 11 个单测 | 环境值只存 SHA-256；服务版本 argv/exit/output 可重放；raw log 长度/摘要 framing；JUnit 与 Cargo 输出逐例比对；`--live-context` 重验 |
| P04-F05 | P04 报告没有真实 prompt/batch inventory，且可能把审计标签写成 batch | documentation/traceability 边界；只可维护 Markdown | 本文件与 S03 traceability | B024–B028 数量核对；列出六个核心 primary；明确 app/identity/scripts 无精确 owner |
| P04-F06 | 阶段文档把所有 migration 泛称为“正反向”，与正史/安全迁移 forward-only 边界冲突 | Data Eventing P0007、P0031、P0050 | S03 README、TEST_PLAN、TEST_RESULTS、acceptance evidence | forward apply、重复 no-op、旧库 upgrade、应用回滚保留 schema、备份恢复与 replay；不对当前正史 migration 执行 destructive down |
| P04-F07 | 新 RAG 测试用 `too_many_arguments` lint allow 掩盖测试 helper 设计问题 | `CODEX-0605-06-DATA-EVENTING-7aa50c4023` / Data Eventing P0030 | `RawSnapshotChunkInsert` typed fixture 替代九参数函数；删除新增 allow | workspace Clippy `-D warnings` 与 RAG 真实 PostgreSQL 测试通过 |

## 核心 Data/Eventing primary 映射

| Prompt | Canonical ID | 当前安全模块 |
| --- | --- | --- |
| `06-data-eventing/P0006` | `CODEX-0062-06-DATA-EVENTING-09d943908d` | `data_eventing::outbox_projection_workers` |
| `06-data-eventing/P0007` | `CODEX-0063-06-DATA-EVENTING-f6f824261f` | `data_eventing::persistence_migrations` |
| `06-data-eventing/P0020` | `CODEX-0595-06-DATA-EVENTING-f8fc21553c` | `data_eventing::event_store_sqlx_outbox_projection` |
| `06-data-eventing/P0030` | `CODEX-0605-06-DATA-EVENTING-7aa50c4023` | `data_eventing::postgre_sql_sq_lx_pgvector` |
| `06-data-eventing/P0031` | `CODEX-0606-06-DATA-EVENTING-96df5cfdb1` | `data_eventing::sqlx_migrations` |
| `06-data-eventing/P0050` | `CODEX-0625-06-DATA-EVENTING-181b11b4cd` | `data_eventing::sqlx_migrations_contract` |

## 独立复验追加缺陷

| ID | 复验发现 | 权威 owner / 边界 | 修复与证明 |
| --- | --- | --- | --- |
| P04-R01 | 报告声称可运行，但 RAG PostgreSQL decoder 与测试直接访问 `RagSnapshotChunk` 私有字段，crate 实际无法编译 | `CODEX-0605-06-DATA-EVENTING-7aa50c4023` / Data Eventing P0030 | decoder 只经 `PersistedRagSnapshotChunk -> RagSnapshotChunk::from_persisted` 校验物化；测试只用公开 accessor；真实服务 66/0、workspace check、Clippy `-D warnings` 通过 |
| P04-R02 | 报告没有区分 Data/Eventing test 内嵌的 backup—destroy—restore drill 与 `trpg-ops` 的独立 runbook target，容易误判实际覆盖边界 | Data/Eventing P0020 拥有 Event Store integration；`CODEX-0097-11-OPS-MIGRATION-e7c0cc1d29` / Ops Migration P0001 拥有 `ops_migration::backup_restore_runbook` | 66 项矩阵继续验证内嵌恢复与 projection rebuild；另独立运行 `-p trpg-ops --test postgres_backup_restore_integration`，验证 custom archive、独立空库、四表计数和篡改 manifest 拒绝，1/1 |
| P04-R03 | clean-worktree repository truth 与未提交修复状态被同时写成 PASS | documentation/traceability 边界 | 静态子门禁独立列为 PASS；`repo_truth.py --check` 对当前 dirty worktree 的拒绝保留为 EXPECTED BLOCK，不伪造 clean 状态 |

全量复验还捕获 schema assertion 冻结旧 migration checksum/function signature，以及 canonical
commit integration 依赖空库、第二次运行会 HMAC mismatch。前者更新为 PostgreSQL 18 实际
catalog 值，后者增加显式本机专用库名与 reset 授权并连续运行两次通过。所有首次失败均保留，
不回写成“首次即通过”。
