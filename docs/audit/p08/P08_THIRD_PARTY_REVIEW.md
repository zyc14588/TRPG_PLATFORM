# P08 独立第三方复核

记录日期：2026-07-27（Australia/Brisbane）
审查基线：`18825746082886a63aee10891860aedb749349e1`

```text
SEMGREP_VERSION = 1.171.0
SEMGREP_EXECUTION = LOCAL_ISOLATED_VENV_SOURCE_ANALYSIS_METRICS_OFF_SINGLE_JOB
SEMGREP_RULE_ORIGIN = COMMUNITY_REGISTRY
SEMGREP_CONFIGS = p/rust,p/security-audit
SEMGREP_SCOPE = 29_P08_RUST_SQL_CI_TARGETS
SEMGREP_RULES_RUN = 13
SEMGREP_FINDINGS = 0
SEMGREP_ERRORS = 0
SEMGREP_SKIPPED = 0
SEMGREP_PARSED_LINES = APPROX_100_PERCENT
SEMGREP_EXIT = 0
CODERABBIT_VERSION = 0.7.0
CODERABBIT_AUTH = NOT_AUTHENTICATED
CODERABBIT_EXTERNAL_REVIEW = NOT_RUN
GITHUB_PR = 9
GITHUB_INITIAL_AUTOMATED_REVIEW = 4_ACTIONABLE
GITHUB_REVIEW_FIX_STATUS = IMPLEMENTED_RERUN_PENDING
CARGO_AUDIT_VERSION = 0.22.2
CARGO_AUDIT_EXIT = 1
CARGO_AUDIT_ADVISORIES = 3_BASELINE_DISCLOSED
```

## Semgrep

Semgrep 1.171.0 安装在 `/tmp` 隔离虚拟环境中。运行时关闭 metrics，只联网获取
社区规则；源码在本机分析，没有把仓库挂载给外部扫描容器。最终以 `--jobs 1`
规避扫描引擎并发初始化的环境资源错误，并明确传入 29 个
P08 Rust、SQL 与 CI 目标，实际运行 13 条适用规则：

- findings：0；
- engine errors：0；
- skipped targets：0；
- parsed lines：约 100%；
- exit：0。

扩展范围首次复扫发现 `data_deletion_e2e.rs` 两处以可预测名称直接使用共享临时目录。
测试改用 `tempfile::Builder::tempdir()` 安全创建唯一目录后，以相同规则和最终 29 个
目标重跑得到 0 finding；没有通过 ignore、规则删减或降低 severity 获得通过。

机器可读结果位于 `/tmp/p08-semgrep-output/p08-final.json`，只作为本次本地复核记录，
不进入发布包，也不含密码或 token。Semgrep 0 finding 只代表所运行规则未发现问题，
不替代功能、数据库、权限、重放或依赖审计。

## CodeRabbit 边界

`coderabbit auth status --agent` 返回未认证；随后启动登录流程，但浏览器回调未完成，
因此仍为：

```json
{"status":"not_authenticated","authenticated":false}
```

因此没有运行或冒充 CodeRabbit review，也没有上传源码或索取认证 token。
强制第三方代码检查由实际完成的 Semgrep 社区规则扫描满足；本地 Clippy、人工复核和
数据库测试没有被冒充为 CodeRabbit 结果。

## GitHub PR 自动审查

PR #9 的首轮远端自动审查在所有 5 个 Hosted CI workflow 通过后仍阻止合并，并提出
4 个 actionable issue：

- Fork 使用当前 projection 过滤，cutoff 后更新的角色会被遗漏；
- keeper-only fork command envelope 错误覆盖 party/private 子实体可见性；
- Combat、Chase、Ending、Growth exact retry 在 projection 已落库时返回冲突；
- Ending 未校验 ID 是否由 Session 绑定 Scenario 声明。

四项均已在本修复提交实现，并以真实 PostgreSQL/Witness E2E 及负向用例验证。修复
提交推送后必须等待远端自动复审与 Hosted CI 再次通过，才允许合并。

## RustSec

`cargo audit 0.22.2 --no-fetch --json` 使用本地 1169 条 advisory 数据检查
381 个 lockfile dependencies，exit `1`：

- `RUSTSEC-2026-0194`、`RUSTSEC-2026-0195`：quick-xml `0.38.4`；
- `RUSTSEC-2023-0071`：rsa `0.9.7`。

基线 `Cargo.lock` 已包含上述版本。P08 增加 `rand_core`、`serde_json` 及测试用
`sqlx`/`trpg-data-eventing` 的已有版本依赖边，没有升级或引入上述受影响版本。
本报告保留 audit 失败，不用 Semgrep 结果覆盖依赖风险，也不宣称 Hosted CI 或发布签署。

## 独立复核结论

在 Semgrep 最终覆盖范围内未发现阻断项；CodeRabbit 因未认证未执行；GitHub 首轮
自动审查的 4 项阻断均已修复但等待远端复审；RustSec 的三个基线 advisory 仍需在
独立依赖治理批次处理。P08 的功能验收结论依赖真实测试和数据库证据，不依赖预写
状态或单一第三方工具。
