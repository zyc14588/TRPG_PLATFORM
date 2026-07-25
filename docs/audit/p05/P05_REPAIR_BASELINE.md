# P05 修复基线

记录日期：2026-07-25（Australia/Brisbane）

```text
REPAIR_BASE_HEAD = dbbc91d58f29c38c9153567609e594fe77cfdee5
BRANCH = agent/p03-p05-review
STARTING_WORKTREE = CLEAN
HISTORICAL_P05_ONLY_CHECKPOINT = NOT_AVAILABLE
ORIGINAL_SCAN_INSTANCE_SET = NOT_RECONSTRUCTABLE
```

## 修复契约

- 修复 P05 回顾暴露的实现、测试、CI、部署和证据控制缺口。
- 不删除测试、不新增 `#[ignore]`、不放宽 policy/visibility/authority gate。
- 缺少依赖、Docker、Hosted CI 或原始证据时，只能记为 `FAIL`、`BLOCKED` 或 `NOT_RUN`。
- family 级控制通过不能冒充原始 finding 实例逐项关闭。
- 只有绑定当前提交、真实命令退出码、raw output 和 JUnit 明细的机器证据才能用于发布准入。

## 可归因基线

本轮开始时 `HEAD` 为
`dbbc91d58f29c38c9153567609e594fe77cfdee5`，工作树没有未提交变更，因此本轮修复可以相对该
提交形成明确 diff。该提交本身同时包含 P03/P04/P05 历史修改，仓库历史中仍没有可验证的 P05-only
checkpoint；本轮不能事后重写该历史事实。

旧 P05 Markdown 引用的 `/tmp/codex-security-scans/...` 扫描目录在本轮回顾时已不存在，仓库内也
没有完整 finding 实例导出。因此无法证明原始实例总数、标题、严重度和逐实例关闭状态。已知
标识符及其诚实状态单列在 `P05_INSTANCE_CONTROL_LEDGER.md`，未恢复的信息保持
`PROVENANCE_UNAVAILABLE`，不猜测补齐。

## 回顾时确认的问题

1. 三个真实集成测试在缺少环境变量时直接 `return`，libtest 会显示 `ok`。
2. manifest 从 Git index 读取内容，可能忽略未暂存和未跟踪修改。
3. CI 只启动部分早期阶段依赖，未覆盖 P04/P05、TLS、MinIO 和真实备份恢复。
4. 发布准入只看泛化测试结果，没有要求关键 P04/P05 JUnit case、0 ignored 和生产安全证据。
5. `test-all.sh` 没有严格 `clippy -D warnings`，也未强制全部真实依赖环境。
6. Compose 验证停留在配置层，未证明 TLS/mTLS、external secret 和证书轮换。
7. 非规范 `trpg-privacy` crate 违反 normalized owner/output map。
8. 旧审计文档引用已经丢失的 raw log、扫描报告和固定计数，存在不可复验 PASS 风险。
9. 只有 family 汇总，没有实例级来源、状态和证据缺失台账。
10. 当前候选尚未形成干净提交，当前 patch 的 Hosted CI 和最终外部审查均未执行。

## 修复后的验证顺序

1. 先证明缺少环境时三个集成测试真实失败。
2. 运行 targeted P03/P04/P05 真实依赖测试。
3. 运行全工作区、all features、no-fail-fast 测试。
4. 运行格式、严格 clippy、Python 证据负向测试、测试库存、ShellCheck、Actionlint、OPA。
5. 解析 production Compose 与 CI override，并运行静态安全契约。
6. 在具备 Docker daemon 的环境运行生产 TLS/mTLS、external secret、轮换和完整服务图。
7. 生成绑定干净提交的机器证据，运行 Hosted CI 和最终审查。

截至本文件记录时，步骤 1–5 只在当前 staged、未提交 patch 上本地执行并观察到相应退出码；
这些 session-local 输出没有持久化为绑定当前候选 commit 的 raw output/JUnit 机器证据，因此
状态只能记为 `LOCAL_EXECUTED_NOT_RELEASE_ATTESTED`，不能记为发布验证完成。步骤 6 为
`NOT_RUN`；步骤 7 因尚无候选 commit、Hosted CI 和持久化外部审查结果而为
`BLOCKED/NOT_RUN`。故不得据此进入 P06。
