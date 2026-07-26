# P07 独立第三方复核

记录日期：2026-07-27（Australia/Brisbane）
补丁基线：`b2793988c5e2e021d556635d19e8d110a99ece8a`

## 结论

```text
SEMGREP_VERSION = 1.171.0
SEMGREP_EXECUTION = LOCAL_ONLY_METRICS_OFF
SEMGREP_RULE_ORIGIN = COMMUNITY_REGISTRY
SEMGREP_BASELINE = HEAD
SEMGREP_PATCH_SCOPE = 37_P07_FILES
SEMGREP_RULES_RUN = 13
SEMGREP_FINDINGS = 0
SEMGREP_ERRORS = 0
SEMGREP_EXIT = 0
CI_REPAIR_SEMGREP_BASELINE = 6657d90a47110e3df4ce4f0e53f1e78e2b661a4c
CI_REPAIR_SEMGREP_SCOPE = 4_SHELL_FILES
CI_REPAIR_SEMGREP_RULES_RUN = 3
CI_REPAIR_SEMGREP_FINDINGS = 0
CI_REPAIR_SEMGREP_ERRORS = 0
CI_REPAIR_SEMGREP_EXIT = 0
GITHUB_CODEX_REVIEW_COMMIT = 6657d90a47110e3df4ce4f0e53f1e78e2b661a4c
GITHUB_CODEX_REVIEW_P1 = 5_FIXED_REPLIED_RESOLVED
REVIEW_REMEDIATION_COMMIT = eb02b24d8d2a422dc70d0d2e5052b3f0267431c1
FINAL_SEMGREP_SCOPE = 21_CHANGED_RUST_SQL_TARGETS
FINAL_SEMGREP_RULES_RUN = 13
FINAL_SEMGREP_FINDINGS = 4_INFO_PREEXISTING
FINAL_SEMGREP_NEW_FINDINGS = 0
FINAL_SEMGREP_ERRORS = 0
FINAL_SEMGREP_EXIT = 0
CODERABBIT_VERSION = 0.7.0
CODERABBIT_EXTERNAL_REVIEW = NOT_RUN_SIGNED_OUT_NO_SOURCE_UPLOAD
CARGO_AUDIT_VERSION = 0.22.2
CARGO_AUDIT_EXIT = 1
CARGO_AUDIT_ADVISORIES = 3_PREEXISTING_DISCLOSED
```

## Semgrep 差异扫描

Semgrep 1.171.0 安装在 `/tmp` 隔离虚拟环境中。扫描关闭 metrics，只从社区 Registry
下载规则，源码分析在本机执行；使用 `HEAD` baseline、`p/rust` 与
`p/security-audit`，精确传入 37 个 P07 Rust/SQL/Shell 文件并包含新增未跟踪文件。

首次默认并发扫描在当前主机创建 `io_uring` 队列时失败，exit `2`、结果中的
`paths.scanned` 为空，因此没有被计为检查通过。以 `--jobs 1` 对同一 baseline、规则和
文件集合重跑后：

- 扫描完成，约 100% parsed lines；
- 13 条规则实际适用于 37 个目标；
- 0 finding、0 engine error、exit `0`。

原始机器可读结果保存在本次会话临时文件 `/tmp/p07-semgrep.json`；它不是仓库发布
artifact，也不包含秘密。

P07 实现发布后，托管 CI 暴露 PostgreSQL 客户端 16/服务端 18 的环境不匹配。对该
follow-up 以实现 commit 为 baseline，精确扫描
`postgres-container-client.sh`、`integration-services.sh`、
`generate-integration-evidence.sh` 与 `test-all.sh`。单并发、metrics off 的本机扫描
实际运行 3 条适用规则，4/4 文件约 100% parsed，0 finding、0 engine error、exit `0`；
机器可读结果位于临时文件 `/tmp/p07-ci-fix-semgrep.json`。

## GitHub Codex 审查与最终复检

GitHub 上配置的 Codex 第三方审查针对 implementation commit
`6657d90a47110e3df4ce4f0e53f1e78e2b661a4c` 提出五个 P1：Agent formal auth
排序、Runtime/Agent 非幂等 retry 排序、邀请服务端时间、旧 request hash 兼容，以及
邀请 Event/membership 原子性。修复提交
`eb02b24d8d2a422dc70d0d2e5052b3f0267431c1` 推送后，每个线程均收到对应代码与测试
说明，随后通过 GitHub GraphQL thread state 确认为 `isResolved=true`。

对最终修复差异再次运行 Semgrep 1.171.0，关闭 metrics，以单并发从社区 Registry 使用
`p/rust` 与 `p/security-audit`。21 个明确传入的 Rust/SQL 目标全部被扫描，约 100%
parsed lines，实际运行 13 条适用规则、无 engine error、exit `0`。结果只有四个
`INFO` 级 `rust.lang.security.temp-dir.temp-dir`：

- `crates/trpg-agent-runtime/tests/common/mod.rs` 两处；
- `crates/trpg-platform/tests/security_privacy_copyright_contract_tests.rs` 一处；
- `crates/trpg-runtime/tests/batch_012_runtime_contract_tests.rs` 一处。

四处均为测试 fixture 的既存代码；`git blame` 分别指向早于本批次的 `dbbc91d5` 或
`0f52f274`，且 review baseline `3fa341f988171b15cbfb13dc345e95622ce07882` 中存在同一
调用。因此最终结论是 `0` 个新增 finding，而不是把四个 INFO 隐去。机器可读结果位于
`/tmp/p07-review-fix-semgrep-final-security.json`，不作为仓库发布 artifact。

## CodeRabbit 边界

已按 CodeRabbit review skill 检查本机 CLI：版本 `0.7.0`，认证状态为 signed out。
因此没有启动外部 review，没有上传源码，也没有把 Semgrep、GitHub Codex 或人工检查
冒充为 CodeRabbit 结果。

用户要求的第三方检查由 GitHub Codex 审查与本机第三方 Semgrep 实际完成；CodeRabbit
状态明确保持 `NOT_RUN_SIGNED_OUT_NO_SOURCE_UPLOAD`。

## RustSec

`cargo audit --no-fetch` 使用本地 1169 条 RustSec advisory 数据扫描最终 lockfile，
稳定返回三个既有 advisory。两个 High 来自 P05 已引入的
`rust-s3 -> aws-creds -> quick-xml 0.38.4`，一个 Medium 为当前 Linux 图不可达的
`rsa 0.9.7` lockfile 残留。P07 新增的 NATS 测试依赖使用仓库已有
`async-nats 0.49.1`，没有引入第二版本或新增 RustSec finding。

因此，本报告证明 P07 实现与 CI follow-up 没有 Semgrep finding，最终 P1 修复没有新增
finding，且四个 INFO 已按基线来源完整披露；不声称依赖审计通过，也不以第三方扫描代替
Hosted CI 或产品发布签署。
