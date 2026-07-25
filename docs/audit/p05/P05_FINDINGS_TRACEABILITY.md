# P05 问题修复追溯

记录日期：2026-07-25（Australia/Brisbane）

```text
REPAIR_BASE_HEAD = dbbc91d58f29c38c9153567609e594fe77cfdee5
CURRENT_CONTROL_REPAIR = LOCAL_EXECUTED_NOT_RELEASE_ATTESTED
ORIGINAL_FINDING_INSTANCE_COMPLETENESS = UNPROVEN
P06_ENTRY = DENIED
```

## 当前控制覆盖

| 控制面 | 主要实现位置 | 当前验证 |
| --- | --- | --- |
| Canonical Event Store、Outbox、payload cipher | `trpg-data-eventing` normalized owner | P03 migration、P04 Event Store/Outbox/Projection/RAG、全工作区测试通过 |
| Privacy、deletion、cloud consent/egress | `trpg-security-governance` normalized owner | deletion 8 项、cloud egress、policy fail-closed、platform privacy 14 项通过 |
| PostgreSQL/Redis/NATS/MinIO 多 surface 删除 | security-governance adapters + 真实临时服务 | PostgreSQL primary/witness、Redis、NATS JetStream、MinIO 实测通过 |
| OpenFGA/OPA 一致授权 | security-governance policy adapter | 真实 OpenFGA/OPA 12 项及 OPA 16/16 通过 |
| TLS、mTLS 和凭据边界 | identity/data-eventing client + Compose secrets | PostgreSQL verify-full TLS 实测；全部数据库客户端的 CA mount 有静态/S09 约束；Redis/NATS unit/static 通过；生产容器 mTLS 未运行 |
| 备份恢复 | `trpg-ops` | PostgreSQL 18 custom archive、独立目标连续两次恢复、计数核对、篡改拒绝通过 |
| 证据与 false-green 防护 | `scripts/ci/repo_truth.py`、manifest、inventory、readiness | Python 19/19，覆盖裸 skip 状态、跨行不拼接、逐测试环境控制边界和静态工作流检查 |
| 发布环境安全 | production Compose + runtime smoke | 两种 Compose config 解析和静态安全契约通过；Docker runtime 未运行 |

## 修复对应关系

- `P05-R01`–`P05-R03`：关闭静默跳过、index 外 manifest 漂移和欺骗性 skip marker。
- `P05-R04`–`P05-R07`：补齐完整依赖 CI、角色 fixture、关键 JUnit 准入和严格 lint。
- `P05-R08`–`P05-R09`：补齐生产 TLS/mTLS、external secret、证书轮换脚本和客户端兼容。
- `P05-R10`–`P05-R11`：恢复 normalized owner/output，并更新失效的验收断言。
- `P05-R12`–`P05-R14`：撤回不可复验证据，建立实例台账，并将未运行/不可恢复项保持阻断。
- `P05-R15`–`P05-R16`：完整安全重扫前置失败保持 `NOT_RUN`；CodeRabbit 后续多轮发现的
  actionable issues 全部逐条验证和修复，最近几轮为删除执行完整性 5 项、终态/redirect 3 项
  及裸 skip 状态漏检 1 项。
  CodeRabbit 结果仍是 session-local、非不可变提交绑定的 review，不提升为 external
  attestation。NATS canonical deletion 采用 data-subject 分区、服务端 subject filter、每批
  128 条和持久 cursor；Redis absence 不再盲信 index；lease recovery 三次耗尽后保持 terminal；
  CI TLS fixture 由 host trust 改为 SCRAM，并以真实连接证明明文拒绝。异常旧 key material 的
  migration 不会绕过 canonical deletion workflow 自动清空，而是锁表并显式拒绝升级。
- `P05-R17`：补齐 realtime、agent-worker、migration-runner 的 PostgreSQL trust anchor mount；
  同时把 HBA `samenet` 的安全前提固化为数据库容器仅连接 internal backend 且生产无端口。
  PostgreSQL 客户端证书建议因不属于当前 `verify-full TLS + SCRAM` 契约而未伪装成已实现的 mTLS。
- `P05-R18`：修复 false-green inventory 越过 preceding sibling block 的反向扫描边界，并保留
  对真实 missing-env early return 的拒绝能力。
- `P05-R19`：消除 baseline 中“工作树为空”的歧义，并把未持久化、未绑定候选 commit 的本地
  步骤 1–5 结果明确降格为 `LOCAL_EXECUTED_NOT_RELEASE_ATTESTED`；Docker runtime、Hosted CI
  和 commit-bound evidence 继续保持 `NOT_RUN/BLOCKED`。
- `P05-R20`：manifest 当前 path/hash/sentinel 数量经独立计数一致；验证器额外独立解析 header
  与表格行，拒绝畸形、重复、缺行或错误 sentinel，避免与 renderer 共享同一错误而一起变绿。
- `P05-R21`：Rust 测试库存 scanner 的 raw/character pattern 改为 compiled positional match，
  且字符字面量不再跨多个 lifetime 吞代码；保留逐测试 missing-env early return 检测。
- `P05-R22`：integration bootstrap 不再只撤销枚举的 service→login 组合；任何以 managed role
  为 granted role 或 member 的旧关系都会清除，然后只恢复 4 条批准映射，schema 断言拒绝额外
  `pg_auth_members` 行。
- `P05-R23`：CodeRabbit 摘要纳入除自引用 disposition 外的全部 P05 审计记录；最新轮次和
  post-fix 未运行状态同步到台账，避免旧审查结果冒充当前结果。

详细状态和原始标识符见 `P05_INSTANCE_CONTROL_LEDGER.md`。

## 不得标记为已关闭的范围

1. 原始扫描实例集合、标题、严重度和逐实例 PoC 不可恢复。
2. 当前 patch 尚未提交，因此没有绑定干净 commit 的证据包。
3. 当前 patch 的 Hosted CI 未运行。
4. 本机缺少 Compose v2 插件且当前用户无 Docker daemon/sudo 权限，production Compose runtime
   TLS/mTLS、external secret 和轮换未运行。
5. P04 历史严格准入材料仍没有当前 Hosted CI/干净候选证明。
6. 等价完整安全重扫在 preflight 阶段即因 native multi-agent V2 前置不满足而停止，没有扫描结果。

因此本文只描述“当前控制覆盖”，不使用 `ALL_FINDINGS_FIXED` 或实例关闭计数。
