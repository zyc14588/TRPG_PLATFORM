# P05 CodeRabbit finding 处置记录

记录日期：2026-07-25（Australia/Brisbane）

本文件只记录 CodeRabbit CLI 对当前未提交补丁的审查结果，不替代完整安全扫描，也不证明
历史 P05 finding 实例全部关闭。

## 首轮审查

首轮返回 16 项；15 项经当前代码确认有效并完成修复，1 项为误报。

| ID | finding 摘要 | 处置 |
| --- | --- | --- |
| `CR-01` | primary PostgreSQL owner HBA 范围过宽 | 已限制到指定数据库和 `samenet`，并加入静态验证 |
| `CR-02` | witness PostgreSQL owner HBA 范围过宽 | 已限制到指定 witness 数据库和 `samenet` |
| `CR-03` | Compose 缺少稳定 project name | 误报；审查时 `compose.yml` 首行已为 `name: coc-ai-trpg` |
| `CR-04` | production smoke 缺少命令前置验证 | 已补 fail-closed preflight |
| `CR-05` | Swarm task 等待循环重复且易漂移 | 已收敛到共享 helper |
| `CR-06` | deletion evidence mismatch 被泛化为数据库错误 | 已增加专用 `DeletionEvidenceMismatch` |
| `CR-07` | cloud egress 篡改测试接受任意 SQL 错误 | 已绑定精确 SQLSTATE/错误语义 |
| `CR-08` | stale running/verifying deletion 无 lease 回收 | 已增加租约、回收、计数、迁移和真实 PostgreSQL 测试 |
| `CR-09` | image digest 接受大写十六进制 | 已限定小写 SHA-256 |
| `CR-10` | completed deletion 可被后续验证改写终态 | 已禁止 terminal 状态回退或改写 |
| `CR-11` | `redis://` 配置可忽略 TLS material | 已对不一致配置 fail closed |
| `CR-12` | completed deletion claim 错误返回可执行 | 已返回不可 claim |
| `CR-13` | IPv6 endpoint 组装缺少方括号 | 已使用正确 bracketed authority |
| `CR-14` | cloud consent/notice 读取缺少并发锁 | 已增加 `FOR SHARE` |
| `CR-15` | S3 每次删除验证都会写 probe object | 已改为启动时一次性 versioning 验证 |
| `CR-16` | security-governance NATS URL credential 未安全解析 | 已 percent-decode，并在交给底层前移除 userinfo |

修复后第二轮审查覆盖当时 65 个变更文件，返回 `findings: 0`。

## 迁移追加修改后的复审

随后租约 migration 和 schema assertion 继续调整，因此重新审查最新补丁。该轮返回 2 项，
均确认有效并修复：

| ID | finding 摘要 | 处置 |
| --- | --- | --- |
| `CR-17` | 实例台账使用未定义、不可机器区分的状态词 | 已定义并统一 current-control、Docker runtime、未运行和 P06 blocking 状态命名空间 |
| `CR-18` | schema drift 只检查一个非 worker 租约列 | 已覆盖 API/canonical/realtime 对 job 三列及 fence lease 列的全部禁止 UPDATE 断言；真实 PostgreSQL 返回 `P05_SCHEMA_ASSERTION_OK` |

## 可扩展性与防伪收口复审

`CR-17` 与 `CR-18` 修复后再次审查当前补丁，返回 6 项，均经代码和运行路径确认有效并修复：

| ID | finding 摘要 | 处置 |
| --- | --- | --- |
| `CR-19` | S09 对 active Compose/smoke 参数只做宽松字符串包含检查 | 改为解析 YAML 并精确比较 active command、production/CI secret 定义和文件来源 |
| `CR-20` | 人工测试摘要可能把预期失败的 `repo_truth` 汇总进总体 PASS | 总体状态显式改为 `AVAILABLE_LOCAL_CODE_GATES_EXCLUDING_REPO_TRUTH`，并保留 `repo_truth` exit 1 |
| `CR-21` | migration 在 worker role 缺失时静默跳过关键授权 | migration 改为缺少 `trpg_worker_service` 即抛错，并无条件执行精确列授权 |
| `CR-22` | canonical deletion 按 stream sequence 全量扫描 NATS，随历史长度线性退化 | canonical transport 改为 data-subject 哈希分区，并使用 JetStream 服务端 subject filter 逐条定位；旧未分区 subject 存在时 fail closed |
| `CR-23` | Dockerfile digest 检查把本地 build stage 名误当外部镜像 | 解析 `FROM ... AS ...` stage，摘要要求只应用于真正的外部 base image |
| `CR-24` | HBA unrestricted 规则只检查 `hostssl`，可能漏过明文 `host` | 正则同时检查 `host` 与 `hostssl`，保留 CIDR/认证方法限制 |

上述 6 项的 focused 测试、真实 JetStream/Redis/P05 多 surface deletion、schema assertion、
全工作区真实依赖测试、严格 Clippy 和 release build 均已通过。

## CI TLS 与 release evidence 收口复审

对上述修复重新审查后又返回 3 项，均确认有效并修复：

| ID | finding 摘要 | 处置 |
| --- | --- | --- |
| `CR-25` | CI PostgreSQL fixtures 使用 permissive host `trust`，TLS fixture 未证明服务端拒绝明文 | 三个 fixture 全部改用 password + SCRAM host auth；TLS fixture 增加 `hostnossl ... reject`，Rust 真实连接测试同时验证明文拒绝、无 CA 拒绝和正确 CA 成功 |
| `CR-26` | 导出的 `P04_RECOVERY_DATABASE_URL` 没有对应初始数据库 | 建库循环显式创建 `p04_eventing_recovery`；恢复 drill 仍会在每轮 drop/create 独立空目标 |
| `CR-27` | release JUnit 只遍历根级 testcase，嵌套 error/skip 可逃逸 | 改为递归遍历全部 testcase；failure/error/skipped 均不能计入 passing required case，并增加嵌套负向测试 |

`CR-25`–`CR-27` 修复后的 targeted 验证已经通过。随后 CodeRabbit 对完整未提交补丁重新审查，
覆盖 70 个变更文件并返回 `0 issues`。更新该结果的审计元数据后，对精确新 diff 再审又发现
2 项防伪文档问题，均确认有效并修复：

| ID | finding 摘要 | 处置 |
| --- | --- | --- |
| `CR-28` | `release_readiness --require-blocked` 的裸 PASS 可能被误读为 release ready | 改为 `BLOCKED_CONFIRMED`，显式记录 exit 0、3 blockers 和 `P06_ENTRY=DENIED` |
| `CR-29` | CodeRabbit-only 结果被错误提升为 external review PASS，且无不可变目标/raw artifact 绑定 | 撤回 external PASS；记录非审计摘要/manifest 的实质补丁 SHA-256，并为最终 CLI 输出保留 session-local raw NDJSON 路径 |

```text
CODE_RABBIT_DIGEST_SCOPE = git diff --cached --binary HEAD excluding only docs/audit/p05/P05_CODERABBIT_DISPOSITION.md, MANIFEST.md, manifests/**
CODE_RABBIT_REVIEW_INPUT_STATE = FULLY_STAGED_NO_UNSTAGED_OR_UNTRACKED
CODE_RABBIT_INPUT_STATE_CHECKS = git diff --name-only; git status --short
CODE_RABBIT_LAST_COMPLETE_REVIEW_ARTIFACT = /tmp/p05-coderabbit-final8.ndjson
CODE_RABBIT_LAST_COMPLETE_REVIEW_FINDINGS = 3
CODE_RABBIT_LAST_COMPLETE_REVIEW_INPUT_LEGACY_SCOPE_SHA256 = fcd7b786d7f4b5413020d1d8ef713851577c3075bf9163c7620dd007e62fd222
CURRENT_POST_FIX_SUBSTANTIVE_PATCH_SHA256 = 47daf34a611078a6f7b9c2325e7b7127669f7421bd262c44492ace8e242b3d7e
CURRENT_POST_FIX_REVIEW = NOT_RUN_AFTER_CR53_CR55
CODE_RABBIT_PRIOR_RATE_LIMITED_ATTEMPT = /tmp/p05-coderabbit-final5.ndjson
CODE_RABBIT_RAW_ARTIFACT_DURABILITY = SESSION_LOCAL_ONLY_NOT_COMMIT_BOUND
FINAL_EXTERNAL_REVIEW = NOT_ATTESTED
```

排除项是自引用审计摘要和由其生成的 manifest；所有实现、配置、workflow、migration 和测试均在
上述 SHA-256 范围内。session-local raw artifact 不属于提交绑定的发布证据，因此即使最终 CLI
返回 0 issues，也不能把 `FINAL_EXTERNAL_REVIEW` 提升为 PASS，更不能替代完整安全重扫、Docker
production runtime 或 Hosted CI。

## 删除执行完整性复审

随后对精确未提交差异的复审又返回 5 项，均确认有效并修复：

| ID | finding 摘要 | 处置 |
| --- | --- | --- |
| `CR-30` | 测试库存按整文件正则搜索，注释/字符串或其他函数可制造 false positive/negative | 改为先屏蔽 Rust 注释、字符串、raw string 和 char，再提取每个真实 `#[test]` / `#[tokio::test]` 函数体并关联环境分支；增加负向 Python 用例 |
| `CR-31` | canonical NATS 删除仍可能一次遍历全部 subject 历史，无法持久恢复 | 每批最多 128 条，以 `progress_cursor` 持久推进；每页续租并在 crash 后从数据库 cursor 恢复；129 条真实 JetStream 消息证明跨页 |
| `CR-32` | Redis absence 只信 subject index，丢索引后的 resident entry 可被误报为不存在 | 增加 bounded `SCAN` 和 resident payload 独立分类；损坏条目 fail closed；真实测试先删 index 再证明不能误报 absence |
| `CR-33` | `execute_next` 遇第一个失败即返回，后续已选作业被饿死 | 遍历全部已选作业、保留首个错误后返回；后续 legal-hold 作业仍会被处理 |
| `CR-34` | failure cleanup 可覆盖原始 surface error，且 lease 丢失后不做独立 best-effort | refresh、target failure、finish 独立尝试并保留首个 cleanup error；所有调用方返回原始 surface error；模拟 lease loss 负向测试通过 |

## 终态约束与 production smoke 复审

上述 5 项修复后的完整差异复审返回 3 项，均确认有效并修复：

| ID | finding 摘要 | 处置 |
| --- | --- | --- |
| `CR-35` | destroyed subject key 缺少表级 `destroyed_at => wrapped_key IS NULL` 不变量，INSERT 可绕过 UPDATE trigger | migration 增加命名 CHECK；全新数据库负向 INSERT 被拒绝，schema assertion 同时验证约束定义和存量行 |
| `CR-36` | `DELETION_LEASE_EXPIRED` 可无限恢复，耗尽作业不能保持终态证据 | 恢复上限固定为 3；migration CHECK、trigger 单调性/终态不变量、Rust selection/claim 全部执行同一上限；三次过期后不可重入且不再入队 |
| `CR-37` | plaintext proxy smoke 在 curl 失败时被 `set -e` 提前终止，且只验 308 不验 HTTPS Location | 捕获 curl failure 供诊断，要求结果同时匹配 HTTP 308 与 `https://` redirect target；静态安全门禁锁定该行为 |

`CR-35`–`CR-37` 修复后在全新 primary/witness 数据库上执行 8 项 P05 deletion E2E、
`P05_SCHEMA_ASSERTION_OK`、全工作区真实依赖测试、严格 Clippy、ShellCheck 和 actionlint，均
exit 0。

## 最终防伪与迁移升级复审

随后复审返回 3 项。1 项有效并修复，1 项与当前权威 normalized map 冲突而不采纳，1 项提出的
自动修复方式会造成无授权数据销毁，改为显式失败关闭：

| ID | finding 摘要 | 处置 |
| --- | --- | --- |
| `CR-38` | skip marker 正则漏掉裸 `SKIPPED` / `NOT RUN` / `NOT_EXECUTED`，且 `\s` 可跨行拼接 | 使用仅横向空白的逐行表达式，接受带或不带详情的状态行；新增裸状态、下划线和跨行不拼接负向测试 |
| `CR-39` | 建议把 `source_processing_record_docs_adr_adr_0005_postgres_pgvector.md` 改为领域名 | 不采纳。该精确 output/module 同时由 `CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md` 的 `CODEX-0641-06-DATA-EVENTING-95d90eabef` 明确规定；P05 无权绕过更高顺位 overlay 单独改名 |
| `CR-40` | 建议 migration 在增加 destroyed-key CHECK 前自动把旧 `wrapped_key` 清空 | 不采纳自动清空。销毁密钥是正式状态变更，现有触发器要求 canonical deletion job 和存活租约；DDL 静默清空会绕过该权威路径并掩盖安全事件。migration 改为锁表、检测异常行、抛出明确错误；真实升级回归证明 migration 不记成功且原材料保持不变，等待授权处置 |

`CR-38`–`CR-40` 处置后，锁定 Node/pnpm 工具链的 Python 防伪测试为 18/18；migration upgrade
真实 PostgreSQL 测试 1/1；全新数据库上的 P05 deletion 8/8、schema assertion、完整 workspace、
严格 Clippy、ShellCheck、actionlint 和 release/all-targets build 均 exit 0。最终 CodeRabbit
post-fix 复审返回 1 项防伪元数据问题：

| ID | finding 摘要 | 处置 |
| --- | --- | --- |
| `CR-41` | cached-diff 摘要未显式证明 review 输入覆盖 staged 与 tracked edit 的完整集合 | 有效并修复。审查开始前 `git diff --name-only` 为空，`git status --short` 只有 index 状态且无 `??`；现显式记录 `FULLY_STAGED_NO_UNSTAGED_OR_UNTRACKED` 及核对命令。审计/manifest 排除仍只用于避免自引用，实质补丁摘要不变 |

`CR-41` 修复后的最终复审仍须再次运行；在该复审完成前不得写成零 finding。

## PostgreSQL 网络与认证建议复审

`CR-41` 修复后的复审返回 3 项。逐项对照实际 Compose 拓扑、HBA 语义和当前 P05 安全契约后，
3 项均不是当前代码中的可成立缺陷；核查过程中另发现 1 项真实的 PostgreSQL CA 挂载遗漏并已
修复：

| ID | finding 摘要 | 处置 |
| --- | --- | --- |
| `CR-42` | witness HBA 的 `samenet` 应替换为“专用 witness 网络”的固定 CIDR | 不采纳固定 CIDR。当前不存在 finding 所假设的第二个 witness 网络；witness 容器只连接 Compose `internal` backend，且生产不发布端口。`samenet` 在该容器内只匹配 Docker IPAM 分配的 backend 子网。新增静态门禁和 S09 回归，任何额外数据库网络、非 internal backend 或生产端口都会失败 |
| `CR-43` | 所有 PostgreSQL role 应增加 `clientcert=verify-full` 并配置 `ssl_ca_file` | 不采纳。当前明确契约是 PostgreSQL server-auth `verify-full TLS + SCRAM`，Redis/NATS 才要求 mTLS；强制此项会新增每个数据库 role 的客户端证书身份、secret、URL、轮换和撤销架构，不是对现有缺陷的最小修复，也不能在 Docker runtime 未执行时伪称完成。它只能作为未来经顶层设计批准的 hardening 变更 |
| `CR-44` | primary HBA 的 `samenet` 应替换为固定 backend CIDR | 不采纳固定 CIDR，理由同 `CR-42`。primary 容器只连接同一个 internal backend，生产无端口；固定私网 CIDR不会缩小当前可达集合，反而会引入宿主路由冲突和一键部署可移植性问题 |
| `P05-R17` | 使用 `sslrootcert=/run/secrets/postgres_ca_certificate` 的 realtime、agent-worker、migration-runner 没有挂载该 CA | 有效并修复。3 个服务均挂载 PostgreSQL trust anchor；静态 Compose 门禁和 S09 测试要求全部 4 个数据库客户端服务包含该 secret。聚焦验证为 S09 4/4 与 Compose security contract PASS |

以上“不采纳”均基于当前文件内容和权威边界，不等于把 finding 隐藏为通过。若未来为 PostgreSQL
正式引入 mTLS 或给数据库容器增加网络，必须同步修改顶层安全契约、角色证书生命周期、secret
轮换、HBA 和运行态 smoke；当前 P06 仍因 Docker runtime 未执行而阻断。

## Manifest 命名与测试库存边界复审

上述处置后的复审返回 2 项：

| ID | finding 摘要 | 处置 |
| --- | --- | --- |
| `CR-45` | 根据 manifest 行批量改名全部 `adr_000x`、`source_processing_record_docs`、`security_privacy_copyrightmpl` 和 `batch_0xx` 文件/模块/测试 | 不采纳。`MANIFEST.md` 是 Git index 的生成清单，不是命名权威；这些精确 current-safe output 大量由更高顺位的 `CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 和 `CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md` 规定。根 AGENTS.md 同时禁止 P05 绕过 normalized overlay 从 manifest/历史 token 派生命名。批量改名会违反当前执行边界 |
| `CR-46` | 测试库存的环境控制语句反向扫描会越过前一个 sibling block | 有效并修复。扫描器现在使用类型匹配的 opening-delimiter stack；顶层 `}` 立即终止，只有已处于未闭合圆/方括号表达式内才跨越嵌套 block。新增 sibling block 回归复现旧 false positive；相关聚焦测试 4/4、完整 Python 防伪 19/19、库存检查均通过 |

`CR-46` 修改了实质补丁，因此还需要对最新摘要执行一次 post-fix 复审；在得到明确 complete
结果前不声明零 finding。`CR-45` 的不采纳是服从权威 overlay，不是隐藏或伪造关闭。

## 基线证据措辞复审

`CR-46` 修复后的第一次 post-fix 尝试只返回 `rate_limit`，没有 `complete` 事件，不能计作审查
结果。限流窗口结束后重新审查，明确完成并返回 2 项，均确认有效并修复：

| ID | finding 摘要 | 处置 |
| --- | --- | --- |
| `CR-47` | baseline 的“工作树为空”可能被误读为目录为空，而不是无未提交变更 | 改为“工作树没有未提交变更”，保留原始 HEAD 与 P05-only checkpoint 不可恢复的事实 |
| `CR-48` | “步骤 1–5 已执行”没有区分 session-local 退出码与当前 commit 绑定的机器证据 | 明确降格为 `LOCAL_EXECUTED_NOT_RELEASE_ATTESTED`；步骤 6 保持 `NOT_RUN`，步骤 7 保持 `BLOCKED/NOT_RUN`，不得作为 P06 发布证明 |

`CR-47`–`CR-48` 只修正文档证据语义，不改变实质补丁摘要。修复后的审查若未获得明确
`complete` 事件，仍不得声明零 finding；session-local 完整审查也不等于 release attestation。

## 状态词、manifest 与 Rust scanner 复审

`CR-47`–`CR-48` 修复后的完整复审返回 4 项。3 项或其一部分确认有效并修复；manifest 已缺行的
断言经独立计数不可复现，但其中要求验证器不要只信同一 renderer 的防伪建议有效并已实现：

| ID | finding 摘要 | 处置 |
| --- | --- | --- |
| `CR-49` | `P05-R19` 使用了台账未定义的 3 个状态词 | 有效并修复。台账在首次使用前定义 `CURRENT_DOCUMENT_CONTROL_PASS`、`LOCAL_EXECUTED_NOT_RELEASE_ATTESTED`、`RELEASE_EVIDENCE_BLOCKED` 及其证据边界 |
| `CR-50` | 声称 manifest 缺少 tracked path/声明计数不符，并要求验证器独立核对 header 与行数 | “当前输出不符”不可复现：Git index 与表格均为 3906 行，3903 个哈希行加 3 个精确 sentinel，三份输出 byte-identical。防伪加固部分有效：`verify_manifest.py` 现独立解析 header、普通/畸形/重复/sentinel 行并核对计数，保留 byte-level 比较；现有负向测试增加 header 和删行篡改 |
| `CR-51` | Rust non-code scanner 在每个位置切片并重新编译 raw-string 正则，造成不必要的重复扫描 | 有效并修复。raw-string start pattern 只编译一次，并用 `pattern.match(source, index)`；终止符、绝对 end 和 blanking 语义保持不变 |
| `CR-52` | character-literal 正则的 `+` 可把多个 Rust lifetime 之间的代码吞成字符字面量 | 有效并修复。现在只匹配一个未转义字符或一个合法 simple/hex/unicode escape；使用原 source/position 的 compiled match。回归证明三个 `'a` lifetime 保持、字符和 raw string 被 blank，换行/offset 不变 |

以上修改使实质补丁摘要变为
`fcd7b786d7f4b5413020d1d8ef713851577c3075bf9163c7620dd007e62fd222`。最近完整审查的输入仍是
前一摘要且返回 4 项，因此在新摘要的 post-fix 审查取得明确 `complete` 前不声明零 finding。

## Role membership 与审查证据范围复审

上述新摘要的完整复审返回 3 项，均确认有效并修复：

| ID | finding 摘要 | 处置 |
| --- | --- | --- |
| `CR-53` | integration bootstrap 只撤销固定 service→login 组合，可遗留任何一端为 managed role 的旧 membership | 改为遍历 `pg_auth_members`，双向撤销涉及 9 个 managed role 的全部关系，再只恢复 4 条白名单；schema assertion 拒绝任意额外关系。真实 PostgreSQL 18.4 注入 managed→external、external→managed、managed→managed 后重跑，只剩 4 条白名单且返回 `P05_SCHEMA_ASSERTION_OK` |
| `CR-54` | digest scope 排除整个 `docs/audit/p05/**`，可漏掉实质台账变更 | 有效并修复。范围现在只排除自引用的本 disposition 与三份 generated manifest；`P05_FINAL_STATUS`、traceability、ledger、baseline、test results 全部纳入摘要 |
| `CR-55` | ledger 的 R16 仍把较早的 2 finding 审查写成当前状态 | 有效并修复。R16 明确记录本轮 3 项及 post-fix 尚未运行，不再把历史结果冒充当前结果 |

按收紧后的范围，当前 post-fix 摘要为
`47daf34a611078a6f7b9c2325e7b7127669f7421bd262c44492ace8e242b3d7e`。`final8` 的文件列表实际
包含全部 P05 审计文件，但其记录摘要使用的是旧的过宽排除范围，所以不追溯重绑为新摘要的通过
证据。当前新摘要仍需明确完成 post-fix review，且无论结果如何都不是 release external
attestation。
