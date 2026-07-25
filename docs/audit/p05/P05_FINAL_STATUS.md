# P05 最终修复状态

记录日期：2026-07-25（Australia/Brisbane）

```text
REPAIR_BASE_HEAD = dbbc91d58f29c38c9153567609e594fe77cfdee5
CURRENT_WORKTREE = STAGED_UNCOMMITTED
LOCAL_WORKSPACE_REAL_DEPENDENCY_TEST = PASS
STRICT_CLIPPY = PASS
FALSE_GREEN_REGRESSIONS = PASS
PRODUCTION_COMPOSE_CONFIG = PASS
PRODUCTION_COMPOSE_RUNTIME_TLS_MTLS = NOT_RUN
HOSTED_CI_CURRENT_PATCH = NOT_RUN
CODE_RABBIT_REVIEW = SESSION_REVIEW_NOT_RELEASE_ATTESTED
EQUIVALENT_COMPLETE_SECURITY_RESCAN = NOT_RUN_PREFLIGHT
FINAL_EXTERNAL_REVIEW = NOT_ATTESTED
ORIGINAL_FINDING_INSTANCE_COMPLETENESS = UNPROVEN
P04_STRICT_PREREQUISITE = NOT_PROVEN_COMPLETE
P06_ENTRY = DENIED
```

## 结论

P05 回顾中能够由当前仓库修复的实现、测试、CI、normalized ownership、Compose 安全配置和证据
控制缺口已经完成修复，并通过当前可用的本地真实依赖门禁。尤其是三个原先可静默返回的集成
测试现在缺少环境即失败；全工作区已在真实 PostgreSQL/Redis/NATS/MinIO/OpenFGA/OPA 上退出 0。
全工作区 release/all-targets 构建、严格 Clippy、格式、数据库 schema 权限断言和前端行为测试也
分别退出 0。

这仍不等于具备 P06 准入资格。当前 patch 尚未提交和推送，Hosted CI 未运行；本机 Docker daemon
不可访问，因此生产容器 TLS/mTLS、external secret 和证书轮换脚本未执行；原始扫描实例集合也
无法从已消失的临时工件中恢复。上述任一项都不能由 Markdown 声明替代。

## 已完成修复

- 移除 TLS、Redis、备份集成测试的 early-return 假通过路径。
- manifest、evidence、inventory 和 readiness 全部改为 fail closed。
- CI 统一覆盖 P02–P05、TLS PostgreSQL、备份、MinIO、OpenFGA、OPA 和角色拓扑。
- release gate 要求精确关键 JUnit case、0 ignored 和独立 production-security evidence。
- `test-all.sh` 强制全 workspace/all features 测试和严格 clippy。
- production Compose 全镜像摘要固定，PostgreSQL TLS-only，Redis/NATS mTLS 与认证，MinIO HTTPS，
  external secret 和轮换路径均有可执行 smoke；全部数据库客户端挂载 verify-full URL 所引用的
  PostgreSQL CA，且静态门禁禁止数据库额外网络和生产端口。
- NATS/Redis 客户端支持生产 credential/TLS 配置并拒绝不完整凭据。
- 删除非规范 `trpg-privacy`；privacy 归 security-governance，cipher 归 data-eventing。
- 删除旧审计中的不可复验日志/哈希/计数，新增实例级 provenance 台账。
- CodeRabbit 首轮 16 项中 15 项有效并已修复，1 项经当前文件内容确认是误报；后续复审曾为
  0 issues。之后两轮分别发现 2 项和 6 项有效问题，均已修复；最后 6 项包括精确 S09 解析、
  测试状态防伪、worker role fail-closed migration、可扩展的 NATS subject 定位、Dockerfile
  stage 解析和明文 HBA 检查。下一轮又发现 3 项 CI TLS/recovery database/JUnit 递归问题，
  也已修复并完成 targeted 验证；随后一次完整补丁复审返回 0 issues，但精确元数据 diff 复审
  又发现 2 项状态表述问题并已修复。CodeRabbit raw 结果仅保存在 session-local `/tmp`，没有绑定
  不可变候选提交，因此不记为 final external review PASS，也不替代完整安全重扫。
- 后续精确复审又发现 5 项删除执行完整性问题和 3 项终态/production smoke 问题：逐 test
  false-green 检测、NATS bounded cursor、Redis 独立 resident scan、批处理排空、原始错误保留、
  destroyed-key CHECK、三次租约恢复上限和 HTTPS Location 验证均已补齐；全新数据库上的 P05
  deletion 8/8、schema assertion 和完整 workspace 均 exit 0。
- 最后一轮又发现裸 `SKIPPED` / `NOT RUN` 漏检并已补齐。领域文件改名意见因与两个当前权威
  normalized map 的精确 output 冲突而不采纳；migration 自动清空旧 wrapped key 的意见因会绕过
  canonical deletion authority 而不采纳，改为锁表、显式拒绝且不销毁材料。对应 Python 18/18、
  migration upgrade、全新数据库 P05 8/8、完整 workspace、严格 Clippy 和 10m05s release build
  均 exit 0。
- 最后一次 HBA 建议复核没有把 PostgreSQL 客户端证书 hardening 冒充当前 mTLS 要求；当前契约
  仍是 PostgreSQL verify-full TLS + SCRAM。核查中实际发现并修复了 3 个数据库客户端漏挂 CA 的
  启动故障，S09 4/4 和静态 Compose security contract 已通过；运行态仍保持 `NOT_RUN`。
- 测试库存的环境控制反向扫描不再越过 preceding sibling block；旧 false positive 已有回归，
  原有 missing-env early return 负例继续被拒绝，最终 Python 防伪为 19/19。
- 最新完整 CodeRabbit 复审又发现 2 项基线证据措辞问题：含糊的“工作树为空”和没有区分
  session-local 执行与 commit-bound 机器证据。两项均已修正；本地步骤 1–5 明确降格为
  `LOCAL_EXECUTED_NOT_RELEASE_ATTESTED`，步骤 6–7 保持 `NOT_RUN/BLOCKED`。该审查仍只是
  `/tmp` 中的 session-local artifact，不写成最终外部审查通过。
- 随后的完整复审返回 4 项：补齐台账状态词；manifest 当前 3906/3903 计数经独立复现本来就
  一致，但验证器新增独立 header/行数/sentinel/重复行校验；Rust scanner 改用原 source 的
  compiled positional regex，并阻止 character literal 吞掉 lifetimes。相关负向回归已加入；
  这轮结果同样不写成最终外部审查通过。
- 新摘要复审又返回 3 项并确认有效：integration role bootstrap 现在双向撤销所有涉及 managed
  role 的旧 membership 后只恢复 4 条白名单；摘要范围不再整体排除 P05 审计目录，只排除
  自引用 disposition 和三份 generated manifest；台账同步为当前轮次。真实 PostgreSQL 注入
  三类越权关系后已证明清理结果只剩 4 条白名单，schema assertion 通过。

## P06 准入阻断项

1. 创建并审核当前 P05 repair commit，确保工作树干净。
2. 推送候选并取得当前 patch 的 Hosted CI 全绿证据。
3. 在同时具备 Compose v2、隔离 Docker daemon 权限的环境运行
   `production-security-smoke.sh`，生成 PASS evidence。
4. 对不可变候选提交运行并持久化最终外部审查 raw artifact；session-local CodeRabbit 结果不足以
   形成 release attestation。
5. 提供原始 finding 实例导出，或在满足 native multi-agent V2 前置后执行新的、可保存实例级结果的
   等价完整扫描；本轮 preflight 为 `incomplete`，没有启动扫描，也没有产生扫描结果。
6. 补齐 P04 严格前置的当前候选/Hosted CI 证明，不能沿用旧 Markdown 自述。
7. 生成绑定候选 commit、raw output、JUnit/SARIF、工具和服务版本的发布证据。

当前 `release_readiness.py --require-blocked` 明确列出
`MISSING_CURRENT_EVIDENCE`、`MISSING_PRODUCTION_SECURITY_EVIDENCE`、`DIRTY_WORKTREE`；
`repo_truth.py --check` 以 exit 1 拒绝非 canonical 分支和未提交工作树。

在这些条件全部满足前，唯一诚实状态是 `P06_ENTRY = DENIED`。
