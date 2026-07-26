# P06 独立第三方复核

记录日期：2026-07-26（Australia/Brisbane）
补丁基线：`b2793988c5e2e021d556635d19e8d110a99ece8a`

## 结论

```text
SEMGREP_VERSION = 1.171.0
SEMGREP_RULE_ORIGIN = COMMUNITY_REGISTRY
SEMGREP_PATCH_SCOPE = 28_P06_CHANGED_FILES
SEMGREP_APPLICABLE_RULES = 90
SEMGREP_NEW_FINDINGS = 0
SEMGREP_EXIT = 0
CARGO_AUDIT_VERSION = 0.22.2
CARGO_AUDIT_EXIT = 1
CARGO_AUDIT_ADVISORIES = 3
CODERABBIT_EXTERNAL_REVIEW = NOT_RUN_NO_SOURCE_UPLOAD_AUTHORIZATION
```

Semgrep 是与实现/测试不同来源的第三方静态分析器。本轮以 `HEAD` 为 baseline、开启
`p/rust` 与 `p/security-audit` community rules、包含未跟踪的新 P06 文件，对 28 个精确
P06 变更文件执行 differential scan；90 条规则适用，结果为 0 个新增 finding，exit `0`。
最终扫描发生在 commit-scoped projection capability 与精确 target 攻击回归加入之后，
因此覆盖本轮最终安全加固状态。

首次 focused scan 曾检出 `campaign_character_api_integration.rs` 使用可预测临时审计路径；
该 finding 已用随机、自动清理的 `tempfile::TempDir` 修复。扩大范围的非 differential
扫描还报告未改动历史文件中的 6 个基线 finding；精确文件非 differential 扫描报告
P05 deletion test 中 2 个未改动 temp-dir finding。它们均被保留为基线风险，不被删除、
忽略或冒充为 P06 新增 finding。

## RustSec 依赖审计

`cargo-audit 0.22.2` 从本地 1169 条 RustSec advisory 数据库扫描当前 `Cargo.lock`
（381 dependencies），真实返回 exit `1`：
最终 `Cargo.lock` 上的 `--no-fetch` 复跑虽提示本地 crates.io package-cache lock
不可取得，仍完成 lockfile 扫描并稳定报告下列同三个 advisory；该 warning 与 exit `1`
都没有被隐藏。

| Advisory | 依赖 | 严重度 | 可达性/处置 |
| --- | --- | --- | --- |
| `RUSTSEC-2026-0194` | `quick-xml 0.38.4` | High 7.5 | 生产图可达：`trpg-security-governance -> rust-s3 0.37.2 -> aws-creds`; upstream 当前版本仍约束 0.38，P06 不越界替换对象存储客户端 |
| `RUSTSEC-2026-0195` | `quick-xml 0.38.4` | High 7.5 | 同上；发布前须升级/替换或完成正式风险接受 |
| `RUSTSEC-2023-0071` | `rsa 0.9.7` | Medium 5.9 | `cargo tree -i rsa@0.9.7 --workspace --all-features` 在当前 Linux 图无输出；lockfile/cross-target 残留，且 advisory 无已修复版本 |

所以本报告只给出“P06 patch 无新增 Semgrep finding”，不声称依赖安全扫描通过，也不声称
产品可发布。上述 dependency risk 属于既有 P05 对象存储/依赖治理面，不影响 P06 两个
主责 AUD 的代码验收，但必须在 release security gate 前处理。

## 外部 AI Review 边界

已尝试启用 CodeRabbit review skill，但把未提交源码发送给外部服务需要用户对该上传动作的
明确授权，当前执行环境没有授予，因此保持 `NOT_RUN`。本地 Semgrep/RustSec 结果没有被
标记为 CodeRabbit，也没有伪造外部 review receipt。
