# P05 修复控制台账

记录日期：2026-07-26（Australia/Brisbane）

## 状态语义

- `CLOSED_PASS`：实现与相应真实测试均已通过。
- `CLOSED_PASS_LOCAL`：控制已经通过本地真实验证，但没有 commit-bound/Hosted CI 证明。
- `RELEASE_ONLY_NOT_RUN`：只影响发布/合并证明，不是 P05 完成或 P06 进入条件。
- `HISTORICAL_PROVENANCE_UNAVAILABLE`：旧临时扫描实例不可恢复，不据此猜测。

## 已知历史实例

| 标识符 | 可恢复信息 | 当前处理 |
| --- | --- | --- |
| `P05-D015` | 仅编号；标题、位置、PoC 不可用 | `HISTORICAL_PROVENANCE_UNAVAILABLE`；不冒充已关闭 |
| `P05-D025` | 仅编号；标题、位置、PoC 不可用 | `HISTORICAL_PROVENANCE_UNAVAILABLE`；不冒充已关闭 |
| `P05-FAM-DELETION-TERMINAL-STATUS-FORGERY` | family 名称可恢复 | 当前 deletion 状态、证据和多 surface E2E `CLOSED_PASS`；历史实例不做伪一对一认领 |
| `P05-FAM-OUTBOX-TERMINAL-STATE-AUTHORIZATION` | family 名称可恢复 | 当前 claim token、ACK/DLQ、列权限与 PostgreSQL integration `CLOSED_PASS` |

P05 的权威外部提示词另有明确的九个 `AUD-*`，其逐项关闭见
`P05_FINDINGS_TRACEABILITY.md`。旧实例工件缺失不阻止对这些已知 AUD 的代码与测试验收。

## 回顾发现与终态

| 控制 ID | 问题与修复摘要 | 当前状态 |
| --- | --- | --- |
| `P05-R01` | TLS、Redis、备份测试缺环境可静默返回；改为强制环境并由 inventory 负向扫描 | `CLOSED_PASS` |
| `P05-R02` | manifest 可能忽略 index 外修改；加入独立 worktree/结构不变量 | `CLOSED_PASS_LOCAL` |
| `P05-R03` | raw output 中伪 skip/not-run 可进入 PASS；evidence generator fail closed | `CLOSED_PASS` |
| `P05-R04` | CI fixture 缺 P04/P05/TLS/MinIO/backup/policy 依赖；统一真实服务 bootstrap | `CLOSED_PASS_LOCAL` |
| `P05-R05` | PostgreSQL fixture 角色拓扑漂移；双向撤销 managed role 旧 membership 后只恢复白名单 | `CLOSED_PASS` |
| `P05-R06` | readiness 可接受泛化/ignored case；要求精确 case、0 ignored 和独立 runtime evidence | `CLOSED_PASS` |
| `P05-R07` | 缺严格 Clippy；workspace/all targets/all features `-D warnings` | `CLOSED_PASS` |
| `P05-R08` | production TLS/mTLS、external secret 与轮换只停留在脚本/静态层 | 完整 runtime smoke 已通过，`CLOSED_PASS` |
| `P05-R09` | NATS/Redis 客户端认证/TLS 与生产配置不一致 | 真实 mTLS、credential、轮换与重连通过，`CLOSED_PASS` |
| `P05-R10` | 非规范 `trpg-privacy` owner/output | 归一到 security-governance/data-eventing，`CLOSED_PASS` |
| `P05-R11` | S09 对 digest/Compose override 的旧断言 | 精确解析 active 配置，`CLOSED_PASS` |
| `P05-R12` | 旧文档引用丢失 raw log/固定计数 | 撤回不可复验声明并分离 session-local/release evidence，`CLOSED_PASS` |
| `P05-R13` | Docker runtime 与 Hosted CI 均缺失 | Docker runtime 已 `CLOSED_PASS`；Hosted CI 为 `RELEASE_ONLY_NOT_RUN` |
| `P05-R14` | 原始临时扫描实例集合不可恢复 | `HISTORICAL_PROVENANCE_UNAVAILABLE`；不影响明确九个 AUD 的 P05 验收 |
| `P05-R15` | 等价完整安全重扫未执行 | `RELEASE_ONLY_NOT_RUN`；未伪造结果，不是 P05/P06 批次条件 |
| `P05-R16` | 外部审查未绑定不可变候选 | `RELEASE_ONLY_NOT_RUN`；历史 CodeRabbit 仅作修复 provenance |
| `P05-R17` | 三个服务缺 PostgreSQL CA mount | CA/hostname/verify-full 与生产启动通过，`CLOSED_PASS` |
| `P05-R18` | false-green scanner 可越过 sibling block | delimiter 边界与负向回归通过，`CLOSED_PASS` |
| `P05-R19` | baseline 将本地执行混同发布证明 | 当前文档明确分层，`CLOSED_PASS` |
| `P05-R20` | manifest renderer/checker 共享错误可能一起变绿 | 三份当前输出从 staged 内容重生；独立 header/行数/sentinel/duplicate 校验为 3916 lines，`CLOSED_PASS` |
| `P05-R21` | Rust scanner raw/character regex 可吞 lifetime 代码 | positional scanner 与回归通过，`CLOSED_PASS` |
| `P05-R22` | role bootstrap 可保留涉及 managed role 的越权 membership | 真实 PostgreSQL 注入/清理/schema assertion 通过，`CLOSED_PASS` |
| `P05-R23` | 审计 digest 排除范围过大、终态文档陈旧 | 当前 P04/P05 证据重写并将旧记录标为历史，`CLOSED_PASS_LOCAL` |

## 发布证明与 P06 准入

```text
P05_BATCH_ACCEPTANCE = COMPLETE
P06_ENTRY = ALLOWED
HOSTED_CI_CURRENT_PATCH = RELEASE_ONLY_NOT_RUN
COMMIT_BOUND_MACHINE_EVIDENCE = RELEASE_ONLY_NOT_GENERATED
FINAL_EXTERNAL_REVIEW = RELEASE_ONLY_NOT_ATTESTED
PRODUCT_RELEASE = NOT_ATTESTED
```

`repo_truth.py --check` 仍应因当前非 canonical 分支和未提交工作树失败。保留该失败可防止把本地
批次完成误报成发布完成；它不属于外部 P05 或 P06 提示词列出的批次前置。
