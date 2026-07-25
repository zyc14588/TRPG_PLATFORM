# P05 测试与防伪结果

记录日期：2026-07-25（Australia/Brisbane）

```text
REPAIR_BASE_HEAD = dbbc91d58f29c38c9153567609e594fe77cfdee5
CURRENT_WORKTREE = STAGED_UNCOMMITTED
AVAILABLE_LOCAL_CODE_GATES_EXCLUDING_REPO_TRUTH = PASS
PRODUCTION_COMPOSE_RUNTIME = NOT_RUN
HOSTED_CI_CURRENT_PATCH = NOT_RUN
RELEASE_EVIDENCE = NOT_GENERATED
```

本文件是人工结果摘要，不是 raw machine evidence。当前会话没有生成可提交的 raw log/JUnit/SARIF
证据包，因此不提供虚构的日志路径、摘要或测试总数。

## 最终已执行命令

| 门禁 | 结果 |
| --- | --- |
| `cargo test --workspace --all-features --locked --no-fail-fast -- --test-threads=1` | PASS，exit 0；真实依赖已配置，doc tests 也完成 |
| `cargo build --workspace --all-targets --release --locked --offline` | PASS，exit 0；最新修复后的完整 release profile 用时 10m05s |
| `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` | PASS，exit 0 |
| `cargo fmt --all -- --check` | PASS |
| `psql ... -f scripts/ci/assert-schema.sql` | PASS；全新数据库返回 `P05_SCHEMA_ASSERTION_OK`，包含 migration checksum、恢复上限、销毁密钥 CHECK、cursor 和 worker/非 worker 权限断言 |
| NATS URL credential 精确单测 | PASS，1 passed / 0 failed / 0 ignored；percent-decoding、底层连接端点去 userinfo、明文远端拒绝、不完整凭据拒绝 |
| `python3 scripts/ci/test_repo_truth.py`（Node 24.17.0 / pnpm 11.9.0） | PASS，19/19；包含嵌套 JUnit error/skip、逐 test env false-green、sibling block 边界、裸 skip 状态和跨行不拼接负例 |
| `python3 scripts/ci/verify_test_inventory.py` | PASS；219 个 Rust test target、48 个 fixture、0 orphan fixture，新增脚本为 Git executable |
| workflow/discovery/dependency/product boundary gates | PASS；含依赖方向 3 个负例和产品边界 5 个负例 |
| `python3 scripts/ci/verify_compose_security.py --check` | PASS |
| Bash syntax + ShellCheck 0.10.0 | PASS |
| Actionlint 1.7.7 | PASS |
| production `compose.yml config --quiet` | PASS |
| production + CI override `config --quiet` | PASS |
| `opa test policy/opa` | PASS，16/16 |
| 根 `npm test` | PASS；S12 UI 与 Python 防伪回归均通过 |
| Web `pnpm test` | PASS；5 条 ready/degraded/unavailable 路径；首次 sandbox `listen EPERM` 不计通过，获准复跑后通过 |
| Web `pnpm build` | PASS |
| `git diff --check` | PASS |
| `release_readiness.py --require-blocked` | BLOCKED_CONFIRMED（exit 0；3 个 blocker；`P06_ENTRY=DENIED`） |
| `repo_truth.py --check` | FAIL，exit 1；分支非 canonical 且工作树未提交 |

全工作区测试使用 PostgreSQL 18.4 primary/witness、Redis、NATS JetStream、MinIO、OpenFGA 和 OPA。
编译只设置 `CARGO_INCREMENTAL=0`、debug info 关闭和 `CARGO_BUILD_JOBS=2` 以控制磁盘峰值；没有移除
package、feature、target 或 test。

## 关键真实集成结果

- PostgreSQL TLS：服务端拒绝明文 TCP，不受信 CA 被拒绝，提供当前 CA 后 verify-full 连接和
  readiness 成功；CI fixture 的 TCP host auth 使用 SCRAM，不再使用 `trust`。
- Redis 登录防护：两个 Identity 实例共享真实分布式 rate limit。
- 备份恢复：PostgreSQL 18 custom archive 恢复到独立目标并用同一 archive 连续恢复两次，表计数
  一致，篡改 manifest 被拒绝。
- P03 migration：空库、升级、重复执行、drift 和 schema assertion 通过。
- P03 migration 对“已标记销毁但仍保留 wrapped key material”的旧漂移显式拒绝；失败 migration
  不写成功 ledger，也不自动销毁材料。
- P04：Event Store、Outbox lease/ACK/DLQ、Projection resume、pgvector RAG 和恢复 drill 通过。
- P05 deletion：8 项真实多 surface 删除、缺失 surface、IO error、retained history、过期
  running/verifying lease 回收、恢复耗尽终态、销毁密钥 CHECK、原始错误保留测试通过；NATS 129
  条 subject 消息跨批推进，Redis 丢 subject index 后仍能独立发现 resident entry。
- P05 cloud/policy：cloud egress、OpenFGA/OPA permit/deny/unavailable、审计链通过。
- Platform privacy：14 项 authority、canonical evidence、跨 Campaign、幂等和不可见性测试通过。

## 防伪负向链

1. 移除环境变量后，TLS、Redis 和备份三个集成目标均以 panic/exit 101 失败，不再显示假 `ok`。
2. P03 首次使用正确库名后发现 CI fixture 缺少生产角色拓扑；增加 role bootstrap 后 migration
   和 schema assertion 才通过。
3. S09 旧断言拒绝 digest-pinned Dockerfile 和标准 Compose override；更新为验证精确摘要及双文件
   调用后 4/4 通过，没有去掉镜像固定。
4. Python 证据测试在错误 Node/pnpm 环境下真实失败 2 项；切换到仓库锁定版本后 14/14 通过，
   没有修改版本校验。
5. 测试库存曾因 index 仍包含已删除 crate 路径且新脚本无 executable mode 而失败；修正 Git
   index/mode 后通过。
6. 早期完整编译曾因临时 target 占满磁盘失败；仅清理可重建产物并降低 debug/incremental
   峰值后从头重跑。
7. 最终阶段第一次全工作区真实依赖复跑正确暴露 3 项失败：schema 断言未同步、TLS URL 走了
   loopback 本地分支、备份目标库非空。逐项修复 fixture/断言后分别通过，没有放宽产品检查。
8. 第二次全量复跑因运行中修改了尚未提交的 migration，旧临时库出现 checksum mismatch；
   没有改写 `_sqlx_migrations`，而是终止该次运行并使用全新数据库从零迁移。
9. 第三次全量复跑使用全新 primary/witness/P02–P05 数据库和独立空备份目标，最终 exit 0。
10. CodeRabbit 在台账状态词和非 worker 租约列权限断言后又发现 6 项有效缺口：S09 宽松断言、
    人工汇总可能掩盖 `repo_truth` 失败、migration 静默跳过 worker 授权、NATS 全 stream 扫描、
    Dockerfile local stage 误判和明文 HBA 漏检。6 项均按 issue 修复，没有放宽断言。
11. NATS data-subject 分区修复后的第一次全量运行正确暴露 2 个 fixture 问题：TLS 指向旧的未迁移
    临时库，备份工具路径是系统 symlink 而严格测试要求普通文件。P05 自身 5 项 deletion 测试在
    该轮已经通过；两个 fixture 分别改用全新迁移库和真实 PostgreSQL 18 binary 后 focused 通过。
12. 最新全量运行使用另一组全新 primary/witness/P02–P05 数据库、真实 PostgreSQL binary 和独立
    空备份目标，最终 exit 0；随后格式、严格 Clippy 与 release build 也再次 exit 0。
13. 下一轮 CodeRabbit 又发现 CI PostgreSQL host trust、P04 recovery 初始库缺失和嵌套 JUnit
    遍历 3 项问题。修复后 Bash syntax、ShellCheck 0.10.0、Python 15/15 和隔离 PostgreSQL TLS
    真实连接测试均退出 0；该 TLS 测试包含明文拒绝、无 CA 拒绝和正确 CA 成功三个连续断言。
14. CodeRabbit 对完整未提交补丁的一轮复审返回 0 issues，但更新审计元数据后的精确 diff 复审
    又发现 2 项防伪表述问题：readiness 裸 PASS 和 CodeRabbit-only 结果被提升为 external PASS。
    两项均已修复；session-local raw review 不记为 release external attestation，等价完整安全
    重扫仍为 `NOT_RUN_PREFLIGHT`。
15. 随后的精确复审又暴露 5 项删除执行缺口：测试库存整文件正则、NATS 无界遍历、Redis 盲信
    index、批处理首错提前退出、cleanup 覆盖原始错误。逐项补实现和负向测试后，P05 deletion
    当时 6/6 通过。
16. 一次最终全工作区运行虽然 P05 deletion 6/6 通过，但总命令 exit 101；唯一失败是备份目标
    残留上次恢复对象。没有清库后重报通过，而是为独立目标启用 `--clean --if-exists` 并把同一
    archive 连续恢复两次写入测试；聚焦和下一轮全工作区均通过。
17. CodeRabbit 随后发现 3 项有效问题：destroyed key INSERT 可保留 wrapped material、lease
    recovery 无上限、plaintext redirect 未验证 Location 且 curl failure 诊断会被 `set -e`
    截断。三项均修复；新 P05 负向用例把 deletion E2E 增至 8 项。
18. migration checksum 改变后没有编辑旧 `_sqlx_migrations`。创建全新 P02、备份、P05
    primary/witness 数据库，从零应用 23 个 migration；最新完整 workspace 命令最终 exit 0，
    随后 schema、格式、严格 Clippy、ShellCheck、actionlint、17 个 Python 防伪测试再次 exit 0。
19. 最终复审发现裸 skip 状态漏检；修复后 Python 在系统 Node 22/缺少 pnpm 的首次运行正确失败
    2 项，切换仓库锁定 Node 24.17.0 / pnpm 11.9.0 后 18/18 通过，没有放宽版本检查。
20. migration 自动清空旧 wrapped key 的建议因绕过 canonical deletion authority 被拒绝；新增
    锁表 preflight 和真实升级负例，证明 migration 明确失败、ledger 不记成功、材料不被静默
    销毁。相应 migration SHA-384 与 schema assertion 同步后，全新数据库返回
    `P05_SCHEMA_ASSERTION_OK`。
21. 全新 P05 数据库的第一次 deletion 复跑在沙箱内因本地网络隔离 7 项连接失败、1 项纯文件
    用例通过；同一测试二进制和同一数据库在获准访问本机服务后 8/8 通过。前一次失败保留在
    历史中，不把纯文件用例或沙箱失败冒充整套通过。
22. 最终完整 workspace 使用又一组全新 P02/P05 primary/witness、TLS 和备份数据库，P03/P04
    专用库由测试重置；总命令 exit 0。随后格式、严格 Clippy、18 个 Python 防伪测试、
    ShellCheck、actionlint、OPA 16/16 和 10m05s release/all-targets build 均 exit 0。
23. 最终尝试 `docker compose ... config` 返回 exit 125：主机没有 Compose v2 插件；脱离沙箱
    的 `docker ps` 仍因 Docker socket 权限失败，`sudo -n` 也被系统拒绝。因此 production
    runtime smoke 保持 `NOT_RUN`，没有用静态 Compose 检查替代运行态 TLS/mTLS 证据。
24. 最终 HBA 建议核查时发现 realtime、agent-worker、migration-runner 的 verify-full URL 引用
    `/run/secrets/postgres_ca_certificate`，但服务未挂载该文件。补齐 3 个 mount 后，Compose
    security contract 与 S09 4/4 通过；Docker runtime 仍未运行，所以没有把静态结果写成容器
    启动 PASS。HBA `samenet` 保留，但新增门禁禁止数据库连接额外网络、要求 backend 为
    `internal` 并禁止生产端口。
25. 下一轮复审发现环境控制语句的反向扫描会越过前一个 sibling block，把不相关 `return`
    误归到后续 `env::var`。最小反例在修复前返回该测试名；改为类型匹配 delimiter stack 后，
    sibling block 不再进入语句，同时原有两个真实 missing-env early return 负例仍被识别。
    聚焦 4/4、锁定 Node/pnpm 的完整 Python 19/19 和库存检查均通过。
26. 最新全工作区复跑第一次复用了旧 P02 临时数据库，migration checksum mismatch 被正确拒绝；
    没有编辑 `_sqlx_migrations` 或把中断的 exit 130 计为通过，而是为 P02/P05 primary/witness、
    TLS 和备份源/目标创建全新数据库。
27. 在上述全新数据库及真实 PostgreSQL/Redis/NATS/MinIO/OpenFGA/OPA 上，从零应用当前 23 个
    migration 后，`cargo test --workspace --all-features --locked --offline --no-fail-fast --
    --test-threads=1` 完整退出 0；随后 schema assertion、格式、严格 Clippy 和锁定工具链的
    Python 19/19 均退出 0。输出仍是 session-local，未提升为 commit-bound release evidence。
28. `CR-46` 后第一次 CodeRabbit post-fix 尝试只返回 rate-limit 错误，不计审查结果；限流结束后
    的完整复审返回 2 项基线证据措辞问题。两项均已修正为更严格的未提交/未证明状态，没有用
    Markdown 把本地退出码冒充发布证据。
29. 最终静态复验第一次虽锁定 Node 24.17.0/pnpm 11.9.0，却显式调用了系统
    `/usr/bin/python3` 3.14.4，与 `.python-version` 3.14.6 不符，Python 防伪测试真实失败
    3 项。改用 `/usr/local/bin/python3` 3.14.6 后原样复跑 19/19 通过；前一次失败没有删除或
    冒充成功。
30. 下一次完整 CodeRabbit 复审返回 4 项。台账状态词缺失和 Rust raw/character scanner 两项
    有效并修复；manifest “当前缺行/计数不符”以 Git index 3906、表格 3906、哈希 3903、
    sentinel 3 及三份 byte-identical 独立反证，但其要求独立计数验证的部分已实现。新增负例
    会拒绝伪造 header 或删行，并证明 Rust lifetime 不被字符正则吞掉。
31. 新摘要的完整复审返回 3 项：角色 membership reset 不完整、review digest 排除整个 P05
    审计目录、R16 状态陈旧，均确认有效。真实 PostgreSQL 18.4 中先注入
    managed→external、external→managed、managed→managed 三类越权关系，再执行新 bootstrap；
    查询只剩 4 条批准映射，删除临时 probe role 后 schema assertion 返回
    `P05_SCHEMA_ASSERTION_OK`。摘要范围和台账也已收紧，但这些新修复不能沿用上一轮 review。

## 明确未执行

- `scripts/ci/production-security-smoke.sh`：本机没有 Compose v2 插件，当前用户无法访问
  Docker daemon，且无非交互 sudo 权限；
- production PostgreSQL/Redis/NATS/MinIO 容器网络的 TLS/mTLS 握手和证书轮换；
- Docker Swarm external secret v1/v2 轮换；
- 当前 patch 的 Hosted CI；
- 绑定当前干净 commit 的 machine evidence；
- 等价完整安全重扫：deep-scan preflight 返回 `incomplete`，native multi-agent V2 owner/version
  无法证明且 V2 配置未启用，因此状态为 `NOT_RUN_PREFLIGHT`，没有创建扫描 goal；
- PowerShell wrapper：当前环境没有 PowerShell，状态为 `NOT_RUN`，只能由 Hosted CI 覆盖。

这些未执行项不计为 PASS，并继续阻断 P06。

当前 readiness 的精确 blocker 为
`MISSING_CURRENT_EVIDENCE`、`MISSING_PRODUCTION_SECURITY_EVIDENCE` 和 `DIRTY_WORKTREE`。
`repo_truth.py --check` 另外正确报告 `canonical branch does not match current branch` 与
`worktree is not clean`。这些失败是当前候选状态的真实结果，不会在提交/合并前改写为 PASS。
