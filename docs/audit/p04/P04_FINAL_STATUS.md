# P04 最终修复状态

```text
AUDIT_LABEL = P04
NORMALIZED_PROMPT_OR_BATCH = NONE
SOURCE_BATCHES = B024,B025,B026,B027,B028
BASE_HEAD = fb6e146612e4df66a508292245da6b995bbe64fb
WORKTREE = UNSTAGED_AND_UNCOMMITTED
LOCAL_TECHNICAL_REPAIR = PASS
TRPG_DATA_EVENTING = 66_PASSED_0_FAILED_0_IGNORED
POSTGRES_BACKUP_RESTORE = 1_PASSED_0_FAILED_0_IGNORED_IN_TRPG_OPS
IDENTITY_GROUP_AUTHORIZATION = PASS
WORKER_LIVENESS = PASS
API_CANONICAL_REPLAY = PASS
DURABLE_WORKFLOW_POSTGRES = PASS
WORKSPACE_CHECK_AND_CLIPPY = PASS
REPOSITORY_STATIC_COMPONENTS = PASS_210_RUST_TARGETS_48_FIXTURES_0_ORPHANS
REPOSITORY_TRUTH_CLEAN_WORKTREE = BLOCKED_EXPECTED_UNCOMMITTED_WORKTREE
MACHINE_EVIDENCE = PASS_CURRENT_WORKTREE_P00_4
CODERABBIT_FINAL_REVIEW = PRIOR_FIX_CYCLE_PASS_NOT_RERUN_AFTER_ADDITIONAL_REPAIR
HOSTED_CI = NOT_RUN
REAL_OPENFGA_OPA = NOT_RUN
FULL_WORKSPACE_EXTERNAL_RUNTIME = NOT_CLAIMED
PRODUCT_RELEASE_READY = NO
```

## 已关闭的七项检查问题

1. 后台 worker health 现在拒绝 pending、stale、stopped、panic 和当前 cycle error，不再只检查
   启动配置或最后一条可覆盖错误。
2. Private-to-group 改为 Identity 持久化权威关系；授权精确绑定 campaign/group/subject，支持实时
   grant/revoke，Player 不能自授，Spectator/Human Keeper 不能伪装成 group target。
3. PostgreSQL canonical JSON 使用 `COLLATE "C"`，并以嵌套 Unicode fixture 与 Rust
   `serde_json` 做逐字节等价证明。
4. 机器证据升级为 p00-4：绑定环境摘要、服务版本命令、原始 stdout/stderr 字节、真实 Cargo
   case/JUnit 明细及完整 worktree；live-context 会重跑服务探针并检查环境漂移。
5. P04 追溯不再伪装成 batch/prompt；B024–B028 的 107 行与核心 primary owner 已独立列出。
6. S03 migration 口径修正为分类验证；正史/安全 migration forward-only，应用回滚保留 schema
   与 Event Store 历史。
7. 删除新增 RAG `too_many_arguments` lint allow，以 typed fixture 替换。

完整映射见 `P04_FINDINGS_TRACEABILITY.md`。

## 复验中追加关闭的问题

- 最初按报告命令重跑时，RAG PostgreSQL row decoder 和新增 contract test 直接构造/读取
  `RagSnapshotChunk` 私有字段，导致 crate 无法编译。decoder 现在只能通过
  `PersistedRagSnapshotChunk -> RagSnapshotChunk::from_persisted` 的校验入口物化，测试只使用公开
  accessor；随后真实服务 66 项矩阵、workspace check 与 Clippy 全部重跑通过。
- 66 项 Data/Eventing 矩阵中的 `postgres_event_store_integration` 内嵌真实
  backup—destroy—restore—rebuild drill；此外又独立执行 `trpg-ops` 的 custom-format archive、
  独立空库恢复、表计数核对和篡改 manifest 拒绝门禁，结果 1/1。两类证明现已分别表述，不再
  混淆“内嵌恢复 drill”和“独立 Cargo test target”。
- 更新 schema assertion 中过期的 migration SHA-384 与 PostgreSQL function signature；没有删除
  checksum/signature gate。
- canonical commit integration 现在要求显式 reset 授权和精确本机专用库名，连续两次运行通过，
  不再依赖“首次运行空库”。
- 备份恢复命令先验证实际 PostgreSQL 18.4 executable；不存在的临时路径不计为测试失败后的通过。
- CodeRabbit 既有修复轮次的稳定测试命名、主/见证 reset 身份比较和空数据库名 fail-closed 共
  3 项已修复；当时第二轮完整审查为 `findings: 0`。该结论早于上述追加修复，因此不冒充为
  当前工作树的重新审查结果。

## 当前数据库契约

| 属性 | 值 |
| --- | --- |
| P04 migration SHA-256 | `730da5a67fdb64e60898dee3ad911e0758261026d64afe5d414078e02cd69024` |
| P04 migration SQLx SHA-384 | `d67991333d4d9e06b5c1c51a9f3c17855bddfbb64558873f4821e7338700981a98fe78f1c05441244c95e5010e156adc` |
| Group migration SHA-256 | `bdf7831cbd46da473502f35b791f73a7e58ab834bc334b1d8eb37d83fdbd9f2f` |
| Group migration SQLx SHA-384 | `89f3a6b7554f9ab392ac0d5f9c06e3d886de9e607415e71fe34585439fee3c82c3580aa8494ffae153533683546163e9` |
| Constraint signature | `74abd245d298fcbc86ffaef6ab33a216` |
| Trigger signature | `3d0d5cdb9fbaa52551125b4a0c935bcb` |
| Trigger/function signature | `55817f97d554378f6cc4bd789115ebf5` |
| SQLx ledger | 9 installed; repeated apply no-op |

本地技术修复通过不等于产品发布：工作树未提交，因此 clean-worktree repository truth gate
按设计拒绝；Hosted CI、TLS、real OpenFGA/OPA 和完整 workspace 外部运行矩阵也未在本轮统一
执行，发布结论保持 `NO`。
