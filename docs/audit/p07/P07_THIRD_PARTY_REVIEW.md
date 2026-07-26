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
CODERABBIT_EXTERNAL_REVIEW = BLOCKED_BEFORE_SOURCE_UPLOAD
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

## CodeRabbit 边界

已按 CodeRabbit review skill 检查 CLI 版本与认证状态，并尝试
`--agent --uncommitted --include-untracked -c AGENTS.md`。环境安全审查在上传前拒绝该
动作，因为它会把完整未提交补丁发送到外部服务。没有绕过该限制，没有发生源码上传，也
没有把 Semgrep 或人工检查冒充为 CodeRabbit 结果。

用户要求的第三方检查由本机第三方 Semgrep 实际完成；CodeRabbit 状态明确保持
`BLOCKED_BEFORE_SOURCE_UPLOAD`。

## RustSec

`cargo audit --no-fetch` 使用本地 1169 条 RustSec advisory 数据扫描最终 lockfile，
稳定返回三个既有 advisory。两个 High 来自 P05 已引入的
`rust-s3 -> aws-creds -> quick-xml 0.38.4`，一个 Medium 为当前 Linux 图不可达的
`rsa 0.9.7` lockfile 残留。P07 新增的 NATS 测试依赖使用仓库已有
`async-nats 0.49.1`，没有引入第二版本或新增 RustSec finding。

因此，本报告证明 P07 实现差异及其 CI follow-up 精确差异均没有 Semgrep finding；不
声称依赖审计通过，也不以第三方扫描代替 Hosted CI 或产品发布签署。
