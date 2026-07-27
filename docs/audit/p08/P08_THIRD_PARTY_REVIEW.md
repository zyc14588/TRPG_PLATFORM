# P08 独立第三方复核

记录日期：2026-07-27（Australia/Brisbane）
审查基线：`18825746082886a63aee10891860aedb749349e1`

```text
SEMGREP_VERSION = 1.171.0
SEMGREP_EXECUTION = LOCAL_ISOLATED_VENV_SOURCE_ANALYSIS_METRICS_OFF_SINGLE_JOB
SEMGREP_RULE_ORIGIN = COMMUNITY_REGISTRY
SEMGREP_CONFIGS = p/rust,p/security-audit
SEMGREP_SCOPE = 32_P08_RUST_SQL_CI_TARGETS
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
GITHUB_INITIAL_REVIEW_FIX_STATUS = FIXED
GITHUB_SECOND_AUTOMATED_REVIEW = 5_ACTIONABLE
GITHUB_SECOND_REVIEW_FIX_STATUS = FIXED
GITHUB_THIRD_AUTOMATED_REVIEW = 5_ACTIONABLE
GITHUB_THIRD_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_FOURTH_REVIEW
GITHUB_FOURTH_AUTOMATED_REVIEW = 4_ACTIONABLE
GITHUB_FOURTH_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_FIFTH_REVIEW
GITHUB_FIFTH_AUTOMATED_REVIEW = 2_ACTIONABLE
GITHUB_FIFTH_REVIEW_FIX_STATUS = IMPLEMENTED_LOCALLY_RERUN_PENDING
GITHUB_THIRD_REPAIR_HOSTED_CI = 3_PASS_2_CANCELED_AFTER_REVIEW_BLOCKERS
GITHUB_FOURTH_REPAIR_HOSTED_CI = 2_PASS_3_CANCELED_AFTER_REVIEW_BLOCKERS
CARGO_AUDIT_VERSION = 0.22.2
CARGO_AUDIT_EXIT = 1
CARGO_AUDIT_ADVISORIES = 3_BASELINE_DISCLOSED
```

## Semgrep

Semgrep 1.171.0 安装在 `/tmp` 隔离虚拟环境中。运行时关闭 metrics，只联网获取
社区规则；源码在本机分析，没有把仓库挂载给外部扫描容器。最终以 `--jobs 1`
规避扫描引擎并发初始化的环境资源错误，并明确传入 32 个
P08 Rust、SQL 与 CI 目标，实际运行 13 条适用规则：

- findings：0；
- engine errors：0；
- skipped targets：0；
- parsed lines：约 100%；
- exit：0。

扩展范围首次复扫发现 `data_deletion_e2e.rs` 两处以可预测名称直接使用共享临时目录。
测试改用 `tempfile::Builder::tempdir()` 安全创建唯一目录后，以相同规则重跑得到
0 finding。本轮又把 Scenario participant 去重实现与负例加入范围，最终 32 个目标
仍为 0 finding；没有通过 ignore、规则删减或降低 severity 获得通过。

机器可读结果位于
`/tmp/p08-semgrep-output/p08-fifth-review-fix-final.json`，只作为本次本地复核记录，
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

四项均已在第一轮修复提交实现，并以真实 PostgreSQL/Witness E2E 及负向用例验证。
该提交的远端复审随后又发现 5 个有效问题：

- 同一 Session 使用不同 Ending ID 时，projection 唯一约束可能在正史 append 后失败；
- 同一 ending/character/skill 使用不同 Growth ID 时存在相同的正史孤儿风险；
- owner-bound 私密 fork 事件沿用了 command-wide `data_subject_id` 和通用密钥；
- Public events、Clues、NPC、Combat、Chase、Conclusion 只进入 manifest，未全部进入子投影；
- 完整来源 snapshot 作为一个事件字段没有大小上限，可能超过 1 MiB 事件限制。

第二轮修复已增加 append 前语义键检查、逐事件数据主体/加密绑定、全部声明 scope 的
正式事件/受保护投影/重放，以及内容寻址 snapshot 和受行数/字节双重限制的物化批次。
其远端复审又提出 5 个有效问题：

- Combat 正式状态仍接受调用方提供的原始伤害，缺少攻击与服务端骰绑定；
- Chase 正式状态仍接受调用方提供的 quarry/pursuer 成功布尔值；
- Growth 未限制为所选 Ending 在 Scenario 中声明的 `growth_awards`；
- 并发的不同 Ending 可能都通过 precheck 后各自追加正史；
- 同一来源角色卡的并发 Growth 可能都追加正史后才有一个 projection CAS 失败。

第三轮修复将 Combat 的攻击、闪避、伤害和 Chase 每名参与者的 percentile roll
替换为共享内核字段私有、不可反序列化的 OS CSPRNG evidence；规则 replay、独立领域
replay 与持久层绑定分别重算。Growth 精确读取所选 Ending 的 `growth_awards`。
Ending 按 Campaign/Session、Growth 按 Campaign/Character 使用事务级 advisory
lock 覆盖 precheck、canonical append 和 projection。真实 PostgreSQL/Witness
错配证据与 `tokio::join!` 并发负例、单元/结构门禁和最终 Semgrep 复扫均通过。
下一轮远端审查未重复上述五项，但继续发现 4 个有效问题：

- Combat 仍以 DEX 而不是 Melee/Firearm 技能验证攻击；
- Combat 缺少 Fight Back、防守方反击和与 Dodge 不同的平手规则；
- Combat/Chase 只要求 Session 存在，未要求状态为 `ACTIVE`；
- Scenario encounter 接受重复 participant ID，导致文档通过后无法构造正式聚合。

第四轮修复把 Melee、Firearm、Dodge 目标随参与者写入正式状态，在规则 replay 与独立
领域 replay 中按动作重新计算；Fight Back 的 tie/counterattack 派生 outcome 也进入
mutation 并防篡改。Combat/Chase 的写入事务通过 Session 行锁与状态精确匹配关闭
TOCTOU，Scenario parser 在入口去重。真实 PostgreSQL/Witness、规则/领域/工作区回归
和扩展到 32 个目标的 Semgrep 均通过。

第三轮修复提交的 Hosted CI 在第四轮阻断出现前为 3/5 通过；两个尚在运行的长任务被
主动取消，未将其写成成功。第四轮修复提交 `ea760c1` 的精确 SHA 审查未重复上述
四项，但继续发现 2 个有效问题：

- 相关复议的 resolution sequence 通过 `GREATEST` 扩大全局 fork cutoff，可能把其间
  较新 Session 的事件与角色成长带入旧 Session 分支；
- Fight Back 使当前攻击者进入 `DYING/DEAD` 后，在 `advance_turn` 前仍可再次攻击，
  且规则 replay 与独立领域 replay 同样遗漏 `can_act` 检查。

第五轮修复把来源 Session 的 gameplay base cutoff 与补充复议链分离；从 verified
Event Store 的 request payload 识别原事件在 base cutoff 内且公开可复制的
reconsideration ID，仅其公开 request/review/resolution 事件被额外纳入。Combat
聚合、规则前驱 replay 与独立领域 serialized replay
同时检查当前攻击者可行动。真实 E2E 把较新 Session/Ending/Growth 放在 cutoff 与
晚期复议之间，证明复议链被保留而较新状态不泄漏；失能重复攻击的聚合与手工 serialized
负例也通过。

`ea760c1` 的 repository-truth 与 golden-scenarios 为 2/5 通过；第五轮阻断出现后，
production-security、workspace 与 release 三个长任务被主动取消，未写成成功。
第五轮修复提交仍须等待全新 5/5 Hosted CI 和精确 SHA 远端复审，才允许合并。

## RustSec

`cargo audit 0.22.2 --no-fetch --json` 使用本地 1169 条 advisory 数据检查
381 个 lockfile dependencies，exit `1`：

- `RUSTSEC-2026-0194`、`RUSTSEC-2026-0195`：quick-xml `0.38.4`；
- `RUSTSEC-2023-0071`：rsa `0.9.7`。

基线 `Cargo.lock` 已包含上述版本。P08 增加 `rand_core`、`serde_json` 及测试用
`sqlx`/`trpg-data-eventing` 的已有版本依赖边，没有升级或引入上述受影响版本。
本报告保留 audit 失败，不用 Semgrep 结果覆盖依赖风险，也不宣称 Hosted CI 或发布签署。

## 独立复核结论

在 Semgrep 最终覆盖范围内未发现阻断项；CodeRabbit 因未认证未执行；GitHub 五轮
自动审查先后提出的 4、5、5、4、2 项阻断均已修复或完成本地验证，最新提交的远端
复审尚待运行；
RustSec 的三个基线 advisory 仍需在独立依赖治理批次处理。P08 的功能验收结论依赖
真实测试和数据库证据，不依赖预写状态或单一第三方工具。
