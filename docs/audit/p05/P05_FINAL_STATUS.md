# P05 最终修复状态

记录日期：2026-07-24（Australia/Brisbane）

```text
BATCH_ID = P05
BASE_HEAD = fb6e146612e4df66a508292245da6b995bbe64fb
LOCAL_IMPLEMENTATION_AND_CONTROL_REPAIR = VERIFIED_ON_AVAILABLE_LOCAL_GATES
STRICT_BATCH_ACCEPTANCE = BLOCKED | PARTIAL_NOT_ACCEPTED
FULL_WORKSPACE_TEST = PASS
STRICT_CLIPPY = PASS
REAL_SERVICE_PROCESS_SMOKE = PASS
REAL_POSTGRES_PRIMARY_AND_WITNESS = PASS
REAL_REDIS = PASS
REAL_NATS_JETSTREAM = PASS
REAL_MINIO = PASS
REAL_OPENFGA_AND_OPA = PASS
P05_SCHEMA_ASSERTION = PASS
DATABASE_LOGIN_ROLE_PROBES = PASS
PRODUCTION_COMPOSE_TLS_MTLS = NOT_RUN
HOSTED_CI = NOT_RUN
FINAL_CODERABBIT_REVIEW = NOT_RUN
P04_STRICT_PREREQUISITE = NOT_PROVEN_COMPLETE
CLEAN_P05_ONLY_SCOPE = NOT_PROVABLE
PRODUCT_RELEASE_READY = NO
```

## 结论

2026-07-23 深度检查列出的、能够在当前仓库工作树内修复的 P05 实现与控制缺口已经修复，并在
2026-07-24 使用真实 PostgreSQL primary/witness、Redis、NATS JetStream、MinIO、OpenFGA 和 OPA
完成复验。最终单命令 workspace 回归记录了 688 个 `ok` 测试用例、0 个失败、0 个 ignored，
并以 exit 0 结束。release 二进制和五个服务进程加 web 的真实 readiness smoke 也已通过。

这不等于 P05 严格批次已经通过。当前工作树从
`fb6e146612e4df66a508292245da6b995bbe64fb` 起混合了未提交的 P03/P04/P05 修改，无法重建
P05 开始前的独立 checkpoint，也无法证明 P05 修改范围只包含预期文件。既有 P04 报告仍未证明
Hosted CI、干净工作树和最终 CodeRabbit 门禁完成。本机 Docker daemon 不可访问且没有 Compose
plugin，因此生产 Compose TLS/mTLS、external secrets 和证书握手没有运行。

`release_readiness.py --require-blocked` 的最终结果是 `BLOCKED`，阻塞项为：

- `MISSING_CURRENT_EVIDENCE`：没有由完整 `scripts/ci/test-all.sh` 生成并绑定当前干净提交的发布证据；
- `DIRTY_WORKTREE`：发布候选要求干净工作树。

因此本文件只确认“当前工作树内可执行的技术修复已通过本地真实依赖验证”，不把缺失的历史、
部署或托管门禁伪造成通过，也不允许据此开始 P06。

## 已修复的控制面

- canonical Event Store 使用版本化完整性格式并绑定全部安全元数据；读取、恢复、提交和 witness
  追加均执行 keyed cryptographic verification，错误密钥不能污染 witness。
- 删除路由统一使用 `delete_personal_data`，真实 OpenFGA/OPA 同时裁决；请求在 canonical
  证据验证前不分配 job，跨 Campaign 与存在性探测返回不透明结果。
- 删除 job 绑定 Campaign、data subject、正式事件和 witness，幂等重试不产生孤儿 job；PostgreSQL、
  RAG、Redis、JetStream、对象、导出与 backup-key 均执行真实删除及逐面 absence proof。
- application roles 不能伪造 canonical tuple、删除终态、legal hold、subject key、erasure tombstone、
  consent、membership 或 Outbox terminal transition。
- cloud authorization 绑定 consent、notice、可信时钟、精确 payload bytes、endpoint、provider、
  model、credential、route 和 policy revision，并在实际发送边界重新检查撤销。
- Fact Provenance 使用仓库信任根，绑定目标 visibility、canonical event 和来源语义；普通 workflow
  payload 不能自证 server dice 或人工确认。
- RAG、Memory RAG、Replay、插件输入/输出和 tool grant 均绑定 live authorization、来源 manifest、
  embedding/内容摘要及完整执行 tuple，处理者身份不能放宽最终受众。
- secret 撤销和 Level 4 认证持久化并签名；“local” provider 不能指向远端 endpoint，也不能静默
  fallback 到云端。
- Compose 不再用 nginx health placeholder 代替应用；release smoke 通过挂载式 secret boundary
  启动真实服务。Redis 生产 namespace 的合法冒号经过专用校验器和真实 E2E 覆盖。
- schema assertion 改为语义验证，新增 forward-only migration 覆盖 authority、RAG/embedding 和
  canonical integrity；PostgreSQL password 不再通过 argv 传递。

逐项追溯见 `P05_FINDINGS_TRACEABILITY.md`；完整命令、失败记录与限制见
`P05_TEST_RESULTS.md`。扫描工件中的修复报告位于
`/tmp/codex-security-scans/TRPG_PLATFORM/fb6e1466_20260723T132131Z/artifacts/fix_report.md`。

## 未通过项

1. P04 严格前置验收没有可验证的完成证据。
2. 没有 P05-only clean checkpoint，`repo_truth.py --check` 正确返回 `worktree is not clean`。
3. `P02_TLS_DATABASE_URL` 与 CA 证书未配置；`postgres_tls_integration` 在当前测试实现中会提前返回，
   虽由 libtest 显示 `ok`，本轮明确不把它计作生产 TLS 通过。
4. 生产 Docker Compose TLS/mTLS、external secrets、证书轮换和真实容器网络未运行。
5. Hosted CI 和本次最终变更后的 CodeRabbit 未运行。
6. 缺少绑定当前干净提交、完整 `test-all.sh`、服务版本和 raw output 的发布证据包。

## 前向部署与回滚边界

- P05 安全 migration 为 forward-only；不得通过 down migration 删除 Event Store、witness、审计或
  privacy evidence。
- 历史明文、无签名或旧完整性版本的数据只能隔离/只读迁移，不能重新声明为可信。
- 应用 binary 可回滚，但必须保留新 schema、quarantine、密钥版本、subject fence 和删除证据。
- 部署前必须在独立环境运行 production Compose TLS/mTLS、完整 `scripts/ci/test-all.sh`、schema
  assertion、角色探针、Hosted CI 和最终审查，并生成与干净 commit 绑定的外部证据。
