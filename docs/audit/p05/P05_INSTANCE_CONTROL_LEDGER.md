# P05 finding 实例与控制台账

记录日期：2026-07-25（Australia/Brisbane）

## 使用规则

本文件区分两件事：

- `CURRENT_CONTROL_*`：当前代码和测试是否证明某项控制有效；
- `RUNTIME_DOCKER_*`、`HOSTED_CI_*`、`SECURITY_RESCAN_*`、`EXTERNAL_REVIEW_*`：
  对应验证面是否真实执行；
- `P06_BLOCKING`：该行仍阻断进入 P06；
- `HISTORICAL_INSTANCE_*`：能否证明原始扫描中的某个 finding 实例已被逐项复核和关闭。

前者通过不自动推出后者通过。原始扫描工件在本轮不可用，因此任何缺失标题、严重度、文件位置
或 PoC 都不会根据编号猜测。

本台账只使用以下当前状态值：

- `CURRENT_CONTROL_PASS`
- `CURRENT_CONTROL_PASS_STATIC`
- `CURRENT_CONTROL_PASS_STATIC_AND_LOCAL`
- `CURRENT_CONTROL_PASS_STATIC_AND_UNIT`
- `CURRENT_CONTROL_IMPLEMENTED`
- `CURRENT_DOCUMENT_CONTROL_PASS`：审计文档与当前可观察状态一致，且没有把未运行项提升为通过
- `LOCAL_EXECUTED_NOT_RELEASE_ATTESTED`：命令只在未提交 patch 上本地执行，没有形成候选
  commit 绑定的持久机器证据
- `RELEASE_EVIDENCE_BLOCKED`：发布所需的 commit-bound raw output/JUnit/外部证明尚不存在
- `RUNTIME_DOCKER_NOT_RUN`
- `HOSTED_CI_NOT_RUN`
- `SECURITY_RESCAN_NOT_RUN_PREFLIGHT`
- `EXTERNAL_REVIEW_PASS`
- `EXTERNAL_REVIEW_NOT_ATTESTED`
- `P06_BLOCKING`

历史实例状态继续使用独立的 `HISTORICAL_INSTANCE_*` 命名空间。

## 可恢复的原始标识符

| 原始标识符 | 可恢复信息 | 当前控制证据 | 实例状态 |
| --- | --- | --- | --- |
| `P05-D015` | 仅能恢复编号；标题、位置和原始 PoC 不可用 | 无法在不猜测标题的情况下建立一对一映射 | `HISTORICAL_INSTANCE_PROVENANCE_UNAVAILABLE` |
| `P05-D025` | 仅能恢复编号；标题、位置和原始 PoC 不可用 | 无法在不猜测标题的情况下建立一对一映射 | `HISTORICAL_INSTANCE_PROVENANCE_UNAVAILABLE` |
| `P05-FAM-DELETION-TERMINAL-STATUS-FORGERY` | family 名称可恢复 | deletion 状态转换、缺失 surface、保留历史和真实多 surface E2E 已通过当前测试 | `CURRENT_CONTROL_PASS / HISTORICAL_INSTANCE_NOT_ATTESTED` |
| `P05-FAM-OUTBOX-TERMINAL-STATE-AUTHORIZATION` | family 名称可恢复 | leased claim token、ACK、DLQ、迁移权限和真实 PostgreSQL Outbox 测试已通过 | `CURRENT_CONTROL_PASS / HISTORICAL_INSTANCE_NOT_ATTESTED` |

以上是当前可证实的已知集合，不声明它是原始 finding 的完整集合。

## 本轮新增审查问题

| 控制 ID | 问题 | 修复/证据 | 当前状态 |
| --- | --- | --- | --- |
| `P05-R01` | TLS、Redis、备份测试可因缺少环境而静默成功 | 三个测试改为强制环境；无环境复跑均退出 101；库存扫描阻止 `env::var + return` | `CURRENT_CONTROL_PASS` |
| `P05-R02` | manifest 可忽略 unstaged/untracked 修改 | manifest write/check 拒绝 index 外变化；新增负向单测 | `CURRENT_CONTROL_PASS` |
| `P05-R03` | raw 输出中的伪“skipped/not run”可进入 PASS 证据 | evidence generator 检测欺骗性 skip marker 并以 integrity exit 86 失败 | `CURRENT_CONTROL_PASS` |
| `P05-R04` | CI 缺少 P04/P05/TLS/MinIO/备份真实依赖 | 统一 integration service 脚本，创建专用 DB、TLS PostgreSQL、MinIO 和 policy sidecars | `CURRENT_CONTROL_PASS_STATIC_AND_LOCAL`; Hosted CI 未运行 |
| `P05-R05` | 临时 PostgreSQL fixture 缺少生产角色拓扑 | 新增 integration role bootstrap；P03 migration/schema assertion 和 P04 targets 实测通过 | `CURRENT_CONTROL_PASS` |
| `P05-R06` | release readiness 可接受泛化或 ignored 测试 | 要求精确 JUnit case、PASS、0 ignored，并单独要求生产安全证据 | `CURRENT_CONTROL_PASS_STATIC`; release evidence 未生成 |
| `P05-R07` | CI 不执行严格 clippy | `test-all.sh` 加入 workspace/all-targets/all-features/`-D warnings` | `CURRENT_CONTROL_PASS` |
| `P05-R08` | production Compose 缺少可执行 TLS/mTLS/secret 轮换验证 | 增加静态契约和完整 runtime smoke 脚本 | `CURRENT_CONTROL_IMPLEMENTED / RUNTIME_DOCKER_NOT_RUN / P06_BLOCKING` |
| `P05-R09` | NATS/Redis 客户端与生产 mTLS/认证配置不一致，NATS 原始 credential URL 可能进入底层诊断 | 客户端解析 URL credential、配置 TLS，并在交给底层前剥离 userinfo；Compose secret 路径对齐 | `CURRENT_CONTROL_PASS_STATIC_AND_UNIT / RUNTIME_DOCKER_NOT_RUN / P06_BLOCKING` |
| `P05-R10` | 非规范 `trpg-privacy` owner/output | privacy 归入 security-governance；payload cipher 归入 data-eventing | `CURRENT_CONTROL_PASS` |
| `P05-R11` | 旧 S09 断言拒绝 digest-pinned Dockerfile/新 override 模式 | 断言改为精确摘要与双 Compose 调用；4/4 通过 | `CURRENT_CONTROL_PASS` |
| `P05-R12` | 旧审计材料引用丢失 raw log 并宣称固定计数 | 删除不可复验日志/哈希/计数；本台账显式记录证据缺失 | `CURRENT_CONTROL_PASS` |
| `P05-R13` | 当前 patch 无 Hosted CI 与 Docker runtime 结果 | 未伪造；保持 P06 阻断 | `HOSTED_CI_NOT_RUN / RUNTIME_DOCKER_NOT_RUN / P06_BLOCKING` |
| `P05-R14` | 原始实例集合无法恢复 | 保持实例 provenance 阻断；需要原始导出或重新执行完整扫描 | `HISTORICAL_INSTANCE_PROVENANCE_UNAVAILABLE / P06_BLOCKING` |
| `P05-R15` | 等价完整安全重扫的执行前置不满足 | deep-scan preflight 为 `incomplete`；未降级扫描、未创建 goal、未生成结果 | `SECURITY_RESCAN_NOT_RUN_PREFLIGHT / P06_BLOCKING` |
| `P05-R16` | 最终外部审查 | CodeRabbit 最近一次完整复审返回 `CR-53`–`CR-55` 三项并已修复；这些修复的新摘要尚未完成 post-fix review。session-local review 未绑定不可变候选提交或持久 raw artifact，不能充当 release attestation | `EXTERNAL_REVIEW_NOT_ATTESTED / P06_BLOCKING` |
| `P05-R17` | realtime、agent-worker、migration-runner 的 verify-full PostgreSQL URL 引用未挂载 CA | 3 个服务补挂 trust anchor；静态门禁要求全部数据库客户端挂载 CA，并锁定数据库仅连接 internal backend、生产无端口；S09 4/4 通过 | `CURRENT_CONTROL_PASS_STATIC`；Docker runtime 未运行 |
| `P05-R18` | 环境 false-green inventory 的反向扫描可越过 preceding sibling block | 改为类型匹配 delimiter stack，顶层 `}` 终止当前语句；新增旧 false positive 回归，Python 19/19 与完整库存检查通过 | `CURRENT_CONTROL_PASS` |
| `P05-R19` | baseline 把本地执行写成可能被误读的验证完成，且“工作树为空”含糊 | 明确初始状态为无未提交变更；步骤 1–5 降格为 `LOCAL_EXECUTED_NOT_RELEASE_ATTESTED`，步骤 6–7 保持未运行/阻断 | `CURRENT_DOCUMENT_CONTROL_PASS / RELEASE_EVIDENCE_BLOCKED` |
| `P05-R20` | manifest 验证器与生成器共享 renderer，缺少独立 header/表格计数不变量 | 当前三份输出的 3906 path/3903 hash/3 sentinel 本来一致；新增独立解析、计数、畸形/重复/sentinel 校验及篡改负例，仍要求 byte-identical | `CURRENT_CONTROL_PASS` |
| `P05-R21` | Rust non-code scanner 重复切片 raw regex，且 character literal 可吞掉 lifetimes 之间的代码 | compiled positional regex；字符只接受一个字符或一个 escape；lifetime/raw/character offset 回归通过 | `CURRENT_CONTROL_PASS` |
| `P05-R22` | integration role bootstrap 只撤销部分 service→login 关系，可保留 involving managed role 的旧 membership | 查询 `pg_auth_members` 并双向撤销所有涉及 9 个 managed role 的关系，再仅恢复 4 条白名单；真实 PostgreSQL 注入三类越权关系后只剩白名单，schema assertion 通过 | `CURRENT_CONTROL_PASS` |
| `P05-R23` | review digest 整体排除 `docs/audit/p05/**`，且 R16 仍显示旧轮次 | 摘要只排除自引用 disposition 与三份 generated manifest，其他 P05 审计记录全部纳入；R16 同步最新完整审查与 post-fix 未运行状态 | `CURRENT_DOCUMENT_CONTROL_PASS / RELEASE_EVIDENCE_BLOCKED` |

## 准入含义

当前控制通过可以支持继续生成候选提交和 Hosted CI 证据，但不能证明历史 finding 实例全部关闭。
在补齐原始实例导出或完成新的、可保存实例级结果的等价完整扫描前，
`ORIGINAL_FINDING_INSTANCE_COMPLETENESS` 必须保持 `UNPROVEN`。
