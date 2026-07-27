# P08 独立第三方复核

记录日期：2026-07-28（Australia/Brisbane）
审查基线：`18825746082886a63aee10891860aedb749349e1`

```text
SEMGREP_VERSION = 1.171.0
SEMGREP_EXECUTION = LOCAL_ISOLATED_VENV_SOURCE_ANALYSIS_METRICS_OFF_SINGLE_JOB
SEMGREP_RULE_ORIGIN = COMMUNITY_REGISTRY
SEMGREP_CONFIGS = p/rust,p/security-audit
SEMGREP_SCOPE = 34_P08_RUST_SQL_CI_TARGETS
SEMGREP_RULES_RUN = 13
SEMGREP_FINDINGS = 0
SEMGREP_ERRORS = 0
SEMGREP_SKIPPED = 0
SEMGREP_PARSED_LINES = APPROX_100_PERCENT
SEMGREP_EXIT = 0
SEMGREP_ROUND_22 = NOT_RUN_EXTERNAL_RULE_FETCH_REJECTED_NO_LOCAL_RULE_CACHE
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
GITHUB_FIFTH_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_SIXTH_REVIEW
GITHUB_SIXTH_AUTOMATED_REVIEW = 3_ACTIONABLE
GITHUB_SIXTH_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_SEVENTH_REVIEW
GITHUB_SEVENTH_AUTOMATED_REVIEW = 5_ACTIONABLE
GITHUB_SEVENTH_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_EIGHTH_REVIEW
GITHUB_EIGHTH_AUTOMATED_REVIEW = 3_ACTIONABLE
GITHUB_EIGHTH_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_NINTH_REVIEW
GITHUB_NINTH_AUTOMATED_REVIEW = 4_ACTIONABLE
GITHUB_NINTH_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_TENTH_REVIEW
GITHUB_TENTH_AUTOMATED_REVIEW = 2_ACTIONABLE
GITHUB_TENTH_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_ELEVENTH_REVIEW
GITHUB_ELEVENTH_AUTOMATED_REVIEW = 2_ACTIONABLE
GITHUB_ELEVENTH_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_TWELFTH_REVIEW
GITHUB_TWELFTH_AUTOMATED_REVIEW = 2_ACTIONABLE
GITHUB_TWELFTH_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_THIRTEENTH_REVIEW
GITHUB_THIRTEENTH_AUTOMATED_REVIEW = 2_ACTIONABLE
GITHUB_THIRTEENTH_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_FOURTEENTH_REVIEW
GITHUB_FOURTEENTH_AUTOMATED_REVIEW = 1_ACTIONABLE
GITHUB_FOURTEENTH_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_FIFTEENTH_REVIEW
GITHUB_FIFTEENTH_AUTOMATED_REVIEW = 3_ACTIONABLE
GITHUB_FIFTEENTH_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_SIXTEENTH_REVIEW
GITHUB_SIXTEENTH_AUTOMATED_REVIEW = 2_ACTIONABLE
GITHUB_SIXTEENTH_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_SEVENTEENTH_REVIEW
GITHUB_SEVENTEENTH_AUTOMATED_REVIEW = 2_ACTIONABLE
GITHUB_SEVENTEENTH_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_EIGHTEENTH_REVIEW
GITHUB_EIGHTEENTH_AUTOMATED_REVIEW = 1_ACTIONABLE
GITHUB_EIGHTEENTH_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_NINETEENTH_REVIEW
GITHUB_NINETEENTH_AUTOMATED_REVIEW = 2_ACTIONABLE
GITHUB_NINETEENTH_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_TWENTIETH_REVIEW
GITHUB_TWENTIETH_AUTOMATED_REVIEW = 1_BLOCKING_2_NONBLOCKING
GITHUB_TWENTIETH_BLOCKING_FIX_STATUS = FIXED_CONFIRMED_BY_TWENTY_FIRST_REVIEW
GITHUB_TWENTIETH_NONBLOCKING_STATUS = DEFERRED_BY_USER_THRESHOLD
GITHUB_TWENTY_FIRST_AUTOMATED_REVIEW = 3_ACTIONABLE
GITHUB_TWENTY_FIRST_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_TWENTY_SECOND_REVIEW
GITHUB_TWENTY_SECOND_AUTOMATED_REVIEW = 2_ACTIONABLE
GITHUB_TWENTY_SECOND_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_TWENTY_THIRD_REVIEW
GITHUB_TWENTY_THIRD_AUTOMATED_REVIEW = 2_ACTIONABLE
GITHUB_TWENTY_THIRD_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_TWENTY_FOURTH_REVIEW
GITHUB_TWENTY_FOURTH_AUTOMATED_REVIEW = 3_ACTIONABLE
GITHUB_TWENTY_FOURTH_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_TWENTY_FIFTH_REVIEW
GITHUB_TWENTY_FIFTH_AUTOMATED_REVIEW = 2_ACTIONABLE
GITHUB_TWENTY_FIFTH_REVIEW_FIX_STATUS = FIXED_CONFIRMED_BY_TWENTY_SIXTH_REVIEW
GITHUB_TWENTY_SIXTH_AUTOMATED_REVIEW = 1_ACTIONABLE
GITHUB_TWENTY_SIXTH_REVIEW_FIX_STATUS = IMPLEMENTED_LOCALLY_RERUN_PENDING
GITHUB_LATEST_REVIEWED_TARGET = 84e09023c65d144758be7573282392e9909d84d6
GITHUB_NEXT_REVIEW_TARGET = PENDING_COMMIT
GITHUB_THIRD_REPAIR_HOSTED_CI = 3_PASS_2_CANCELED_AFTER_REVIEW_BLOCKERS
GITHUB_FOURTH_REPAIR_HOSTED_CI = 2_PASS_3_CANCELED_AFTER_REVIEW_BLOCKERS
GITHUB_FIFTH_REPAIR_HOSTED_CI = 3_PASS_2_CANCELED_AFTER_REVIEW_BLOCKERS
GITHUB_SIXTH_REPAIR_HOSTED_CI = 3_PASS_2_CANCELED_AFTER_REVIEW_BLOCKERS
GITHUB_SEVENTH_REPAIR_HOSTED_CI = 3_PASS_2_CANCELED_AFTER_REVIEW_BLOCKERS
GITHUB_EIGHTH_REPAIR_HOSTED_CI = 3_PASS_2_CANCELED_AFTER_REVIEW_BLOCKERS
GITHUB_NINTH_REPAIR_HOSTED_CI = 3_PASS_2_CANCELED_AFTER_REVIEW_BLOCKERS
GITHUB_TENTH_REPAIR_HOSTED_CI = 3_PASS_2_CANCELED_AFTER_REVIEW_BLOCKERS
GITHUB_ELEVENTH_REPAIR_HOSTED_CI = 3_PASS_2_CANCELED_AFTER_REVIEW_BLOCKERS
GITHUB_TWELFTH_REPAIR_HOSTED_CI = 3_PASS_2_CANCELED_AFTER_REVIEW_BLOCKERS
GITHUB_THIRTEENTH_REPAIR_HOSTED_CI = 3_PASS_2_CANCELED_AFTER_REVIEW_BLOCKERS
GITHUB_FOURTEENTH_REPAIR_HOSTED_CI = 3_PASS_2_CANCELED_AFTER_REVIEW_BLOCKERS
GITHUB_TWENTY_FIRST_REPAIR_HOSTED_CI = 5_PASS
GITHUB_TWENTY_SECOND_REPAIR_HOSTED_CI = 3_PASS_2_RUNNING_AT_REVIEW_CUTOFF
GITHUB_TWENTY_THIRD_REPAIR_HOSTED_CI = 3_PASS_2_RUNNING_AT_REVIEW_CUTOFF
GITHUB_TWENTY_FOURTH_REPAIR_HOSTED_CI = 3_PASS_2_RUNNING_AT_REVIEW_CUTOFF
GITHUB_TWENTY_FIFTH_REPAIR_HOSTED_CI = 3_PASS_2_RUNNING_AT_REVIEW_CUTOFF
GITHUB_LATEST_REPAIR_HOSTED_CI = PENDING_LOCAL_COMMIT
CARGO_AUDIT_VERSION = 0.22.2
CARGO_AUDIT_EXIT = 1
CARGO_AUDIT_ADVISORIES = 3_BASELINE_DISCLOSED
```

## Semgrep

Semgrep 1.171.0 安装在 `/tmp` 隔离虚拟环境中。运行时关闭 metrics，只联网获取
社区规则；源码在本机分析，没有把仓库挂载给外部扫描容器。最终以 `--jobs 1`
规避扫描引擎并发初始化的环境资源错误，并明确传入 34 个
P08 Rust、SQL 与 CI 目标，实际运行 13 条适用规则：

- findings：0；
- engine errors：0；
- skipped targets：0；
- parsed lines：约 100%；
- exit：0。

早期复扫第一次在受限网络中拉取 registry 规则时停滞并被终止；联网重试取得规则后，
默认并行度因 `io_uring_queue_init` 资源分配失败返回 exit `2` 和 engine error。
第九轮复扫同样先在受限网络等待后主动终止，再以获准联网和固定 `--jobs 1` 完成；
第十轮修复后沿用相同 34 目标与规则配置，直接得到 0 finding、0 error、0 skipped。
第十一轮第一次启动因默认日志目录只读而在扫描前退出，第二次因受限网络无法解析规则
registry exit `2`；显式把设置/日志定向到 `/tmp` 并获准获取相同社区规则后，才得到
最终 0 finding、0 error、0 skipped。两个前置错误均未计为扫描通过。
第十二轮修复沿用相同隔离环境、规则与精确 34 目标范围，直接完成为
0 finding、0 error、0 skipped。
第十三轮修复再次沿用同一 Semgrep 1.171.0 环境、两组社区配置、关闭 metrics、
单 worker 和相同 34 目标，直接完成为 0 finding、0 error、0 skipped。
第十四轮修复使用完全相同的环境、配置和 34 目标再次直接完成为
0 finding、0 error、0 skipped。
第十五轮第一次因受限 DNS 无法取得相同 registry 配置而 exit `2`；授权联网后第二次
已取得 236 条规则，但默认并行引擎因 `io_uring_queue_init` 资源不足得到 1 error、
0 scanned，同样未计通过。不减规则、不减目标、不忽略错误，只固定 `--jobs 1` 后，
13 条适用规则对 34 目标完成为 0 finding、0 error、0 skipped。
第二十轮阻断修复继续使用 Semgrep 1.171.0、相同两组 registry 配置、关闭 metrics
并固定单 worker，对本轮实际变更的 2 个 Rust 与 1 个 SQL 目标运行 13 条适用规则，
得到 0 finding、0 error、0 skipped；没有用缩小规则集替代既有 34 目标基线扫描。
第二十一轮修复对实际变更的 3 个 Rust 与 2 个 SQL 目标沿用相同配置，13 条适用规则
同样得到 0 finding、0 error、0 skipped。
第二十二轮已在 `/tmp` 重新准备相同 Semgrep 1.171.0，但安全审查拒绝了向社区
registry 获取 `p/rust` 与 `p/security-audit`：该外联可能向未获本轮明确授权的服务
暴露仓库或扫描元数据。本机没有保留这两组规则的可复用缓存，因此没有绕过审查、
没有改用较弱规则，也没有产生或宣称本轮扫描结果；历史 34-target 基线与第二十一轮
changed-target PASS 仍保留为历史证据，本轮状态为
`NOT_RUN_EXTERNAL_RULE_FETCH_REJECTED_NO_LOCAL_RULE_CACHE`。
只有最终 0 error JSON 被计为通过，前述中间运行没有被覆盖或伪报。

扩展范围首次复扫发现 `data_deletion_e2e.rs` 两处以可预测名称直接使用共享临时目录。
测试改用 `tempfile::Builder::tempdir()` 安全创建唯一目录后，以相同规则重跑得到
0 finding。本轮又把 Scenario participant 去重、miss 正史、Fork child lineage
唯一性实现/负例、全局 gameplay roll 消费及第五个 migration 加入范围，最终
34 个目标仍为 0 finding；
没有通过 ignore、规则删减或降低 severity 获得通过。

机器可读结果位于
`/tmp/p08-semgrep-output/p08-fifteenth-review-fix-final.json`、
`/tmp/p08-semgrep-round20.json` 与 `/tmp/p08-semgrep-round21.json`，只作为本次
本地复核记录，
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
第五轮修复提交 `fb3907e` 的精确 SHA 审查未重复上述两项，但继续发现 3 个有效问题：

- 攻击失败或 Dodge 成功被当作错误，未形成正式 mutation，攻击/防御骰证据会丢失；
- 两个不同 fork ID 可在同一空 child 上并发通过无锁 emptiness check，独立 stream
  均可能先写正史；
- Ending summary、Reconsideration review summary/resolution 在 canonical event
  中保留首尾空白，而 live projection trim，删除重建后结果会漂移。

第六轮修复加入 `ATTACK_MISSED` 无伤害转换，保存攻击/防御骰、拒绝伤害骰并由规则与
独立领域 replay 重算；Fork 在 emptiness check 前获取 child-scoped transaction
advisory lock，持有至 canonical commit/projection 完成，同时读取 verified Event
Store lineage，并由 `UNIQUE(child_campaign_id)` 兜底；三个文本字段在 event 构造前
统一规范化，idempotency/live projection/replay 共用同一值。真实双 PostgreSQL/Witness
测试以 `tokio::join!` 验证两个不同 fork ID 只有一个成功，并用带空白输入验证删除后
投影一致。

`fb3907e` 的 repository-truth、golden-scenarios、production-security 为 3/5 通过；
第六轮阻断出现后，workspace 与 release 两个长任务被主动取消，未写成成功。
第六轮修复提交 `2ed9df2` 的精确 SHA 审查未重复上述三项，但继续发现 5 个有效问题：

- Campaign member 可猜测不可见 source event sequence 并发起复议；
- 攻击成功或失败后没有消费当前回合动作，可在推进前再次攻击；
- `DYING/DEAD` 防守者仍能 Dodge/Fight Back；
- MajorWound 医疗 target 仍由调用方提交，可绕过治疗者真实技能；
- opaque 服务端 roll 对象可被 clone 并在后续 aggregate version 重复使用。

第七轮修复在发起复议前验证 verified source event，并把请求者访问权与
Visibility/subject/data subject 精确继承纳入同一 fail-closed 检查；review/resolve
也必须与上一链事件和新 envelope 三项一致。Combat 状态新增动作消费标记，
`TurnAdvanced` 是唯一重置路径；主动防御检查目标可行动。First Aid/Medicine target
由当前治疗者的持久化技能派生，失败也形成正式 mutation。Combat/Chase 状态同时保存
已消费 roll ID ledger，规则 replay 与独立领域 replay 均拒绝本次内部重复和后续版本
复用。

`2ed9df2` 的 repository-truth、golden-scenarios、production-security 为 3/5 通过；
第七轮阻断出现后，workspace 与 release 两个长任务被主动取消，未写成成功。
第七轮修复已通过规则/领域单元测试、工作区 check/Clippy、真实双
PostgreSQL/Witness 回归和 33 目标 Semgrep。对应提交 `56b648b` 的
repository-truth、golden-scenarios、production-security 为 3/5 通过；第八轮阻断
出现后，workspace 与 release 两个长任务被主动取消，未写成成功。第八轮精确 SHA
审查确认上述五项未重复，但继续发现 3 个有效问题：

- 仅按最大 sequence 取 source cutoff，会把同一 Campaign 中交错写入的其他 Session
  事件及其角色变化纳入选定 Session 的 fork；
- Combat/Chase rebuild 遇到相同 version 的损坏投影会跳过，且不会删除无 canonical
  history 的 ghost 行；
- aggregate-local roll ledger 不能阻止同一 opaque roll 在不同 aggregate 或
  Combat/Chase 之间重复消费。

第八轮修复从 verified `SessionStarted`、来源 Session ID、Scene/Action 归属构造 base
event set；rebuild 在 campaign-scoped 锁和单笔事务内清除 Combat/Chase/全局骰消费
读模型再重放；新 migration 以 `roll_id` 全局主键、排序 advisory lock 与 canonical
projection guard 建立跨 aggregate/类型单次消费。真实数据库回归把第二 Session
start/end 插在第一 Session Ending/Growth 之前，注入同版本污染及 ghost，并把已消费
Combat 医疗骰 clone 给 Chase；三类攻击均被正确隔离或拒绝。最终状态从空
primary/Witness 连续运行两次，工作区 check/Clippy 与 34 目标 Semgrep 也通过；仍须
等待新的精确 SHA 复审。

对应提交 `f1b0e70` 的 repository-truth、golden-scenarios、
production-security 为 3/5 通过；第九轮阻断出现后，workspace 与 release 两个长
任务被主动取消，未写成成功。第九轮精确 SHA 审查确认第八轮三项未重复，但继续发现
4 个有效问题：

- rebuild 只清除 Combat/Chase/roll，保留损坏或 ghost 的 Reconsideration、Fork、
  Ending、Growth 及 fork-created 基础实体；
- Growth 的 percentile/d10 未进入全局 gameplay roll ownership，可复用 Combat 或
  Chase roll；
- forked Combat/Chase participant、initiative、transition 和 roll 引用仍指向 parent
  character/NPC ID；
- fork 构造期间长期持有 projection pool connection，20 个并发请求可各持一条连接
  后等待第二条而耗尽 20-connection pool。

第九轮修复把所有 P08 writer 与 rebuild 绑定同一 campaign lock，清除并重放全部
P08/Fork materialization；Growth 角色回退仍需 secret capability、精确 canonical
target、仅 Growth 后缀和 canonical 派生版本。Growth 两类骰加入全局表；Fork 递归
重写 gameplay ID 并为非角色参与者产生 child NPC projection。Fork snapshot/build、
canonical commit 与 replay-page load 不再持有 projection connection，由 Event Store
partial unique index 和 projection unique constraint 保证 lineage，最终投影只使用短
事务；`max_connections=1` 的真实回归成功。全新 primary/Witness 完整套件连续通过
两次，all-features check/Clippy 与 34 目标 Semgrep 均通过。

对应提交 `3b90578` 的 repository-truth、golden-scenarios、
production-security 为 3/5 通过；第十轮精确 SHA review `4784615487` 确认第九轮
四项未重复，但继续提出 2 个有效问题：P08 rebuild 会删除 fork 后由正常工作流新增
的 Scenario/Character/Session/Scene，而 exact fork retry 又把整个 child Campaign
行数与 immutable manifest 比较。workspace/release 随即主动取消，未写成成功。
第十轮修复从 canonical `CampaignForkMaterialized` payload 提取 fork-owned 基础
ID，rebuild 只替换这些行；retry 则从 verified Event Store projection targets
计算该 fork 的实际行。真实双库随后创建后续 Scenario、Character/Sheet、
Session/Scene，再执行 exact retry 与 rebuild，后续投影逐字节不变且 Event Store
不增不改；完整套件从全新 primary/Witness 连续通过两次。

对应提交 `4250462` 的 repository-truth、golden-scenarios、
production-security 为 3/5 通过；第十一轮精确 SHA review `4784745614` 确认第十轮
两项未重复，但继续提出 2 个有效问题：

- source-session selector 只从 `payload.data` 读取归属字段，顶层 shape 的
  `PlayerActionSubmitted.action_id` 会被遗漏，并连带排除依赖的
  `SanityLossApplied`；
- child projection emptiness preflight 没有与普通 canonical write 共用线性化锁，
  普通写可在检查后、Fork commit 前进入另一条正史。

第十一轮修复让 replay 字段同时支持顶层与嵌套 shape，并在真实 Tutorial 中断言
snapshot 与 child sheet 的 SAN。Event Store 新增 campaign-scoped BEFORE INSERT
trigger：所有 canonical write 共用事务 advisory lock，`CampaignForkRecorded` 在
插入点重新拒绝非创建/邀请基线历史。确定性屏障测试让普通 Scenario write 先排队、
Fork 后排队，释放后只有普通正史/投影成功。迁移、双 primary/Witness、all-features
check/Clippy 和 34 目标 Semgrep 均已通过。对应提交 `453b063` 的
repository-truth、golden-scenarios、production-security 为 3/5 通过；第十二轮精确
SHA review `4784953431` 确认上述两项未重复，但继续提出 2 个有效问题：

- Combat damage evidence 把所有 Melee 固定为 `1d6`、Firearm 固定为 `1d6+5`，
  会拒绝验收 fixture 的 `1d6+1`，也无法表达正式武器差异；
- synthesized fork child scenario 只复制 ending ID/summary 而丢弃
  `growth_awards`，fork 前尚未结算的成长因此无法在 child 中通过授权检查。

workspace/release 随即主动取消，未写成 5/5。第十二轮修复把选定 melee/firearm
weapon ID 与受限公式持久化进每个 Combat participant；普通命中从攻击者、Fight Back
从实际反击者派生期望公式，规则 replay 与独立领域 replay 都拒绝错误公式。Fork
snapshot 则从 source scenario 的实现结局提取、验证并内容寻址完整 awards，再原样写入
child scenario。真实 Tutorial 使用 child-owned ending/character/current sheet 和
服务端骰成功结算尚未消费的 Psychology 奖励。完整双 primary/Witness 连续通过两次，
all-features check/Clippy 与 34 目标 Semgrep 均通过。对应提交 `c11f82c` 的
repository-truth、golden-scenarios、production-security 为 3/5 通过；第十三轮精确
SHA review `4785113440` 确认第十二轮两项未重复，但继续提出 2 个有效问题：

- 已存在 canonical lineage 的 exact fork retry 仍重新加载 parent 当前 snapshot；
  parent 后续相关 Growth/Reconsideration 改变 hash 后，重试会误报 mismatch，首次
  已写正史但投影失败的请求也无法借重试恢复；
- 合法的无 `growth_awards` Ending 只能进入 `AwaitingGrowth`，但空 settlement 被
  无条件拒绝，结论永久无法进入 `Completed`。

workspace/release 随即主动取消，未写成 5/5。第十三轮修复在检测到 canonical lineage
后，从 verified `CampaignForkRecorded`、materialization manifest 和全部 batch
事件恢复原 materialization、Visibility、data subject 与 projection targets，不再
读取可变 parent snapshot；同时允许空成长 settlement 完成结局，而完成后的重复调用
仍失败。真实 PostgreSQL/Witness 回归先证明 parent snapshot hash 因后续相关复议而
变化，再以原请求成功重试且 child Event Store 行数不增。runtime 状态机 `3/3`、
workspace all-target/all-feature check/Clippy 和相同 34 目标 Semgrep 均通过。
对应提交 `99e3374` 的 repository-truth、golden-scenarios、production-security
为 3/5 通过；第十四轮精确 SHA review `4785291315` 确认第十三轮两项未重复，但继续
提出 1 个 P1：`CombatantState` 唯一公开构造器只接收 max HP 并强制 current HP
为满值、condition 为 `ABLE`，因此上一遭遇留下的伤势、MajorWound、Dying/Dead 会在
新战斗被静默治愈。workspace/release 随即主动取消，未写成 5/5。

第十四轮修复新增 `CombatHealth` 值对象，显式绑定 current/max/condition 并校验
零 HP 与 Dying/Dead 的一致性；`CombatantState::health()` 可把上一遭遇的持久化
快照原样交给下一遭遇。规则与独立领域 initial-state validator 同时允许合法伤势、
拒绝矛盾组合。规则回归 `7/7`、独立领域 `6/6`、真实 PostgreSQL/Witness、workspace
all-target/all-feature check/Clippy 和相同 34 目标 Semgrep 均通过；第一次形成 8
参数构造器时严格 Clippy 拒绝，未计通过，改用值对象后才通过。仍须等待新的精确 SHA
5/5 Hosted CI 与远端复审，才允许合并。对应提交 `7466745` 的
repository-truth、golden-scenarios、production-security 为 3/5 通过；第十五轮精确
SHA review `4785498371` 确认第十四轮问题未重复，但继续提出 3 个 P1：

- Growth command 的 caller-supplied Visibility 可覆盖 owner-private Character/Sheet，
  把完整新角色卡扩大为 public/party；
- shared-kernel 公开原始 percentile/d10 组合构造器，调用方可从多次生成中拼装挑选
  后的成长证据；
- 唯一医疗转换拒绝 0 HP `Dying`，成功 First Aid 无法稳定濒死调查员。

workspace/release 随即主动取消，未写成 5/5。第十五轮修复在正式 append 前加载并比较
Character/当前 Sheet 的来源 label/subject，要求命令精确保持相同 envelope；真实
public widening 攻击得到 `PolicyEvidenceMismatch`，Event Store、私密来源及 current
Sheet 不变，随后合法私密 Growth 才成功。成长证据删除 `from_server_rolls` 和公开
d10 入口，唯一生成路径一次采样完整尝试；跨玩法复用负例继续用合法原子证据验证全局
消费。规则层与独立 replay 同时把成功 First Aid 派生为 1 HP `MajorWound`，拒绝
Medicine 跳过急救和伪造 `Able`。规则 `8/8`、独立领域 `6/6`、runtime conclusion
`3/3`、真实 PostgreSQL/Witness、workspace all-target/all-feature check/Clippy 与
相同 34 目标 Semgrep 均通过。仍须等待新的精确 SHA 5/5 Hosted CI 与远端复审，才允许
合并。

对应提交 `b39dc72` 的第十六轮精确 SHA review `4785705667` 确认第十五轮三项未重复，
并提出 2 个 P1：Growth 后发生 SAN 时 rebuild 会错误回退角色；fork 后普通角色完成
Growth 时又会因统一跳过 rewind 而在删除成长 sheet 后留下悬空 current version。
`f970d7f` 按 verified canonical tip 区分 P08-owned、later-canonical 与污染状态，
分别安全重建或保留后续状态；普通 Campaign 的 Growth→SAN 和 child Campaign 的
post-fork Character→Growth 真库路径均通过。

第十七轮精确 SHA review `4785952496` 确认上述两项未重复，并提出 2 个 P1：超出
Chrono 范围的 Ending 时间戳、以及全局冲突的 Growth 新 sheet ID，都会先写 Event
Store 再投影失败。`e118ad2` 把时间转换和 sheet identity 预检移到 canonical append
之前并纳入共享 writer lock；负例证明正史不增长且之后合法请求仍可完成。

第十八轮精确 SHA review `4786025401` 确认上述两项未重复，并指出生产 API role
没有 P08 rebuild 直接 DELETE 权限，owner-pool 集成测试掩盖了线上必失败路径。
`e7210f8` 增加固定 `search_path`、PUBLIC 无执行权、仅 API role 可调用且必须持有
最新 verified/formal P08 commit 秘密 capability 的 target-scoped
`SECURITY DEFINER` 清理函数；真实 API role 测试同时证明直接 DELETE 和缺 capability
调用失败。

第十九轮精确 SHA review `4786159483` 确认权限问题未重复，并提出 2 项：source
Session 启动后创建、cutoff 前完成但未参与 action 的 idle character 会被 fork
遗漏；带首尾空白的 Ending ID 可通过场景验证却无法选择。`b6bd4a0` 纳入 cutoff 前
Character create/submit/approve 生命周期，并让真实 fork 物化 late joiner；场景
验证同时拒绝 padded Ending ID 与 growth skill。两项均由第二十轮未重复确认。

第二十轮精确 SHA review `4786284972` 对 `b6bd4a0` 提出 1 个 P1 与 2 个 P2。
P1 是 fork 复制角色后发生 SAN 等非 P08 更新时，cleanup 仍删除角色并撞上非延迟
sheet 外键。提交 `8087404e852afa2642e887fea9662443dc02213c` 只清理 canonical tip
仍由 P08 拥有的共享投影；任何保留角色都必须由最新 verified/formal 角色与当前
sheet 事件、版本计数、Visibility 与 Fact Provenance 联合证明。真实 PostgreSQL
回归执行 fork→copied character SAN→P08 rebuild，并逐字节核对角色、全部 sheet、
player action 与 SAN，完整数据库套件通过。本轮 changed-target Semgrep 为 3 targets、
13 rules、0 finding/0 error/0 skipped。

两个 P2 分别是“显式授权的非默认私密 fork scope 尚未贯穿 persistence”和“非默认
canon status 尚未持久化”。当前 P08/P09 入口只承诺默认公开 fork，两项不影响现有
游玩闭环、P09 进入条件或重大安全边界；按用户明确门槛记录延期，未标记为已修。
`8087404` 的第二十一轮精确 SHA review `4786393257` 确认第二十轮 P1 未重复，并提出
3 个 P2：v2 fork materialization nullable snapshot 字段可借 CHECK=`UNKNOWN` 绕过；
REVIEWED/RESOLVED reconsideration nullable evidence 同样可绕过；攻击 `Dead` 目标时
miss 可被记录、hit 才失败，使命令有效性依赖随机结果。前两项直接影响 P08/P09
正史形状和反伪造边界，第三项直接影响战斗体验，因此均不符合延期条件。

修复没有改写已发布顺序的 `00300`/`00400`，而是新增 forward-only
`20260727000900_enforce_p08_projection_shapes.sql`，显式要求 v2 fork snapshot 与
Reconsideration 每个状态的证据非空。Schema assertion 除 catalog 指纹外，使用 4 个
临时表行为探针证明 Fork、REVIEWED、UPHELD、CORRECTED 的 NULL 均触发
`check_violation`。Combat 在解析/消费骰前统一拒绝 `Dead` target，独立领域 replay
同步拒绝；新回归证明 hit 与 miss 都返回相同错误且 aggregate 不变。空库、B24 upgrade
与 repeat、真实 PostgreSQL/Witness、完整 Tutorial、规则 `9/9`、领域 `6/6`、
workspace strict Clippy 及 changed-target Semgrep 5 targets/13 rules/0 finding/
0 error/0 skipped 全部通过。新提交的 Hosted CI 与第二十二轮精确 SHA 复审仍待运行，
本报告保持 pending，不把本地结果写成远端通过。

提交 `b611eabea05d22a88fb0bb9e5ec40bf285a48714` 随后由五个
pull-request-triggered Hosted workflow 全部验证通过：
`repository-truth`、`golden-scenarios`、`workspace-ci`、
`production-security-runtime` 与 `release-readiness-evidence` 均为 completed/success。
第二十二轮精确 SHA review `4786508911` 没有重复第二十一轮三项，并提出 1 个 P1、
1 个 P2：

- Combat/Chase/Growth 的正式事件先在 canonical 事务提交，骰消费再由独立 projection
  事务写入；取消、连接丢失或状态投影失败会留下正史却释放骰 ownership，使 clone
  证据可能在另一个 aggregate 再次进入正史；
- Scenario Ending 可重复同一 `growth_awards.skill_name`，入口接受后却会被 fork
  conclusion snapshot 的唯一性校验拒绝。

本地修复新增 forward-only
`20260728000100_reserve_gameplay_rolls_with_canonical_commit.sql`。Canonical service
在 event、audit、formal commit 尚未提交的同一事务内，经 HMAC-bound projection
target 调用最小权限 `SECURITY DEFINER` 函数预留每条 opaque roll；状态投影失败不再
释放 ownership。API role 只能计算内容寻址 reservation ID，不能执行预留函数；
PUBLIC 与 worker 均无执行权。真库 trigger 故障注入证明 canonical event/roll
reservation 在 Combat 状态投影失败后仍持久，clone 到 Chase 会被拒绝，随后 exact
retry 只恢复投影、不追加第二条正史；旧无 marker 的已提交请求仍保持原 request hash
重试兼容。Scenario 验证同时按每个 Ending 拒绝重复技能。

workspace all-target/all-feature check、严格 Clippy、完整 ruleset/domain、data-eventing
lib、锁定工具链 repo-truth、migration upgrade、decision atomicity、core-domain、
Tutorial 与 P06/P07/P08 schema/权限断言均已通过；本轮 Semgrep 因上述外联拒绝
明确记为未运行。

提交 `bfdc6f4222e9d944be2d8fdde2c7642b51a0dddc` 后，
`repository-truth`、`golden-scenarios` 与 `production-security-runtime` 三个 Hosted
workflow 已完成通过；`workspace-ci`、`release-readiness-evidence` 在第二十三轮
审查意见到达时仍运行，因此只记录 3/5，不冒充完整通过。精确 SHA review
`4789295455` 没有重复第二十二轮两项，并提出两个 P2：

- `record_ending` 以 trimmed ID 通过场景匹配，却把原始 padded ID 写入 canonical
  event 与 projection；fork snapshot 随后用精确相等匹配场景 Ending，可能取不到
  `growth_awards` 并使 snapshot 反序列化失败；
- Scenario encounter participant 只校验非空/不重复，含空格、标点或超过运行时上限
  的 ID 可通过导入，却无法构造 Combat/Chase 状态机。

修复提交 `b165094194cb8fc39a7bfa37bbc89074eed4686f` 在命令入口只生成一次
`normalized_ending_id`，场景匹配、正式事件与投影共同使用该值；真实数据库以 padded
ID 调用，核对 event/projection 后继续完成 Growth 与 fork。Scenario validator 增加
与 Combat/Chase 状态机完全相同的 ID 谓词：非空、最多 128 字节、仅 ASCII
字母数字、`_`、`-`；combat 空格、chase 标点和重复 participant 均由入口负例拒绝。
其 `repository-truth`、`golden-scenarios`、`production-security-runtime` 已完成
通过；`workspace-ci`、`release-readiness-evidence` 在第二十四轮意见到达时仍运行，
因此只记录 3/5 通过、2 项运行中。精确 SHA review `4789452227` 确认第二十三轮两项
未重复，并提出两个 P1、一个 P2：

- fork snapshot 没有保存来源角色已经消费的成长奖励；child 拥有同一 sheet/ending
  但没有 `growth_events` 行，可再次成长同一技能；
- `record_campaign_fork` 只验证 child 空且唯一，没有证明 child Authority Contract
  是父级通过不可变 `fork_for_child` 派生，任意独立 Campaign 可冒充 lineage；
- 仍为 `ONGOING` 的 Combat/Chase 可被复制到状态固定为 `ENDED` 的 child Session，
  该聚合随后无法继续推进。

当前最小修复把来源正式 `growth_events` 与已有 marker 合并成按角色/技能的内容寻址
消费集合，只对被复制角色映射 child ID；`record_growth` 在 append 前拒绝已消费 pair，
但不阻止同一结局的其他技能。Fork 写入 SQL 同时验证 child contract 的确定性 ID、
version 1、locked/FORK_ONLY、父级全部规则/安全/模型/角色卡 snapshot 以及精确
`+1ms` 创建时间，和共享内核 `fork_for_child` 保持一致。Preview 与最终
materialization 都拒绝非终态 Combat/Chase。

真实 core-domain 回归证明非派生 child 返回 `fork_authority_contract` 且 Event Store
不增加；Tutorial 证明 ongoing gameplay 返回 `fork_source_gameplay_not_terminal`，
child 重领 Library Use 返回 `growth_skill_already_recorded` 且不追加正史，而
Psychology 仍成功。新增 Authority 负例最初令超长 async 集成测试越过默认栈；
业务断言在诊断栈下通过后，仅把该负例 future 堆分配隔离，随后默认栈 `1/1` 通过。
完整 ruleset、data-eventing lib `26/26`、Tutorial `2/2`、workspace check 与严格
Clippy 均通过。修复提交 `7acc802df979c8eb282ce2cc577e10d0a3b9f0b8`
的 repository-truth、golden-scenarios、production-security-runtime 已完成通过；
workspace 与 release-readiness 在下一轮意见到达时仍运行，因此只记录 3/5 通过、
2 项运行中。第二十五轮精确 SHA review `4789695257` 确认第二十四轮三项未重复，
并提出一个 P1、一个 P2：

- 旧 fork 把 `CampaignForkRecorded.campaign_id` 记为 parent，同一 parent 的多个
  合法 child 会在 migration `20260727000700` 按 campaign_id 创建唯一索引时冲突，
  阻断数据库升级；
- Scenario 可接受超过 128 字节的 `growth_awards.skill_name`，但持久层请求检查和
  `growth_events` schema 都会拒绝，该奖励永远无法正式结算。

当前最小修复让新 child-owned v2 lineage 的首事件同时声明其合法 materialization
projection row；该 target 被 request hash、event HMAC 与 formal commit 保护。Partial
unique index 与 fork-empty trigger 只选择带此判别的 v2 事件，legacy parent-owned
事件保持不可变且不参与 child 唯一约束。真实 migration gate 在仅到
`20260727000600` 的 schema 中种入同 parent 两条旧 fork，再完整升级至 HEAD；两条
都保留且新索引存在。函数定义变化第一次被 catalog fingerprint 正确拒绝，更新实际
完整指纹后从空库重跑，legacy/B24/empty/repeat/drift/constraints `1/1` 通过。
Scenario Ending 校验同步 `record_growth` 的 128 字节上限，并以 129 字节负例证明
入口拒绝。场景 `5/5`、真实 core-domain `1/1`、Tutorial `2/2` 已通过。修复提交
`84e09023c65d144758be7573282392e9909d84d6` 的 repository-truth、
golden-scenarios、production-security-runtime 已完成通过；workspace 与
release-readiness 在下一轮意见到达时仍运行，因此只记录 3/5 通过、2 项运行中。
第二十六轮精确 SHA review `4789850275` 确认第二十五轮两项未重复，并提出一个 P1：

- marker 增加到首个 `CampaignForkRecorded.projection_targets` 后参与 canonical
  request hash；已存在的 pre-marker child-owned fork exact retry 若无条件构造新
  target，会发生 idempotency conflict，canonical 已成功但投影缺失的历史无法补建。

当前最小修复在加载唯一 canonical lineage 时保留 sequence。该 replay 先经过 payload、
HMAC 与 Witness 验证，Event Store 又是 append-only，因此可从同一行可信读取 marker
relation 与 fork row ID。首次写入固定使用双-target v2；检测到旧正史则重建原
单-target draft，新正史仍重建双-target draft。单元
`fork_lineage_target_shape_preserves_pre_marker_retries` 精确比较两种 hash-relevant
target vectors；核心真库另断言新首事件持久化 marker，并让 existing exact retry、
projection recovery 与并发回归在默认栈 `1/1` 通过。data-eventing lib `27/27`、
Tutorial `2/2` 通过。修复提交
`6b8e39b976b907281999b1815859007fc0a14eea` 的 repository-truth、
golden-scenarios、production-security-runtime 已完成通过；workspace 与
release-readiness 在下一轮意见到达时仍运行，因此只记录 3/5 通过、2 项运行中。
第二十七轮精确 SHA review `4789952622` 确认第二十六轮问题未重复，并提出两个 P1、
一个 P2：

- Combat v1 只验证调用方提供的 state JSON 自洽，没有把 participant、DEX、技能、
  武器、护甲、max/current HP 与 Scenario、角色卡、NPC 或前序 canonical gameplay
  绑定，可注入虚构/增强初态或跨遭遇治疗；
- `EndingRecorded` 先进入 Event Store，Session 的唯一结局所有权再由独立 projection
  事务建立；投影失败或取消会留下正史却允许另一 stream 写入第二个结局；
- P08 rebuild 只在 replay 非空时执行清理，因此没有 canonical P08 event 的 Campaign
  无法清除被注入或遗留的 P08 ghost。

当前最小修复让 Combat v1 在排序 participant advisory lock 下读取经 HMAC/Witness
验证的 campaign replay，要求 participant 集合匹配会话 Scenario 的 Combat encounter，
角色卡为 approved/locked，角色/NPC `combat_profile` 精确匹配 DEX、技能、武器、
护甲与 max HP，并从最新 canonical Combat snapshot 原样承接 current HP/condition；
同一 participant 也不能同时处于另一 active Combat。

新增 forward-only `20260728000200_bind_session_endings_and_empty_rebuild.sql`。
HMAC-bound Session ending reservation 由 canonical-only `SECURITY DEFINER` 函数与
Event Store、audit、formal commit 同事务提交；旧无 marker 的 exact retry 保留原
request-hash shape。另一 API-role-only、秘密 capability 约束的清理函数只在再次确认
Campaign 不含任意 canonical P08 event 后删除 Campaign-local P08 投影。真实数据库
故障注入证明 Ending projection 失败后不同 ending ID 仍在 append 前被拒绝，exact
retry 恢复原投影；空历史 ghost 由真实 API role 清除；伪造 Combat 初态与跨遭遇治疗
均不写 Event Store。migration upgrade、decision atomicity、默认栈 core-domain
`1/1`、Tutorial `2/2`、schema assertion、workspace check 与严格 Clippy 已通过；
新提交、Hosted CI 和第二十八轮精确 SHA review 仍为 pending。

## RustSec

`cargo audit 0.22.2 --no-fetch` 使用本地 1169 条 advisory 数据检查
381 个 lockfile dependencies，exit `1`：

- `RUSTSEC-2026-0194`、`RUSTSEC-2026-0195`：quick-xml `0.38.4`；
- `RUSTSEC-2023-0071`：rsa `0.9.7`。

基线 `Cargo.lock` 已包含上述版本。P08 增加 `rand_core`、`serde_json` 及测试用
`sqlx`/`trpg-data-eventing` 的已有版本依赖边，没有升级或引入上述受影响版本。
本报告保留 audit 失败，不用 Semgrep 结果覆盖依赖风险，也不宣称 Hosted CI 或发布签署。

## 独立复核结论

在 Semgrep 最终覆盖范围内未发现阻断项；CodeRabbit 因未认证未执行；GitHub
二十七轮自动审查的真实意见均已逐项记录。所有影响游玩、P09 入口或重大安全/正史
完整性的项目均已修复或完成本地验证；第二十轮两个默认公开 fork 之外的扩展 P2 按
用户门槛明确延期，没有冒充修复。当前最小修复的远端 CI/精确 SHA 复审尚待运行；
RustSec 的三个基线 advisory 仍需在独立依赖治理批次处理。P08 的功能验收结论依赖
真实测试和数据库证据，不依赖预写状态或单一第三方工具。
