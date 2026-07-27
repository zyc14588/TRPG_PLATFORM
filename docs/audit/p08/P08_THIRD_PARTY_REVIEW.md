# P08 独立第三方复核

记录日期：2026-07-27（Australia/Brisbane）
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
GITHUB_NINTH_REVIEW_FIX_STATUS = IMPLEMENTED_LOCALLY_RERUN_PENDING
GITHUB_THIRD_REPAIR_HOSTED_CI = 3_PASS_2_CANCELED_AFTER_REVIEW_BLOCKERS
GITHUB_FOURTH_REPAIR_HOSTED_CI = 2_PASS_3_CANCELED_AFTER_REVIEW_BLOCKERS
GITHUB_FIFTH_REPAIR_HOSTED_CI = 3_PASS_2_CANCELED_AFTER_REVIEW_BLOCKERS
GITHUB_SIXTH_REPAIR_HOSTED_CI = 3_PASS_2_CANCELED_AFTER_REVIEW_BLOCKERS
GITHUB_SEVENTH_REPAIR_HOSTED_CI = 3_PASS_2_CANCELED_AFTER_REVIEW_BLOCKERS
GITHUB_EIGHTH_REPAIR_HOSTED_CI = 3_PASS_2_CANCELED_AFTER_REVIEW_BLOCKERS
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
第九轮复扫同样先在受限网络等待后主动终止，再以获准联网和固定 `--jobs 1` 完成。
只有最终 0 error JSON 被计为通过，前述中间运行没有被覆盖或伪报。

扩展范围首次复扫发现 `data_deletion_e2e.rs` 两处以可预测名称直接使用共享临时目录。
测试改用 `tempfile::Builder::tempdir()` 安全创建唯一目录后，以相同规则重跑得到
0 finding。本轮又把 Scenario participant 去重、miss 正史、Fork child lineage
唯一性实现/负例、全局 gameplay roll 消费及第五个 migration 加入范围，最终
34 个目标仍为 0 finding；
没有通过 ignore、规则删减或降低 severity 获得通过。

机器可读结果位于
`/tmp/p08-semgrep-output/p08-ninth-review-fix-final.json`，只作为本次本地复核记录，
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
两次，all-features check/Clippy 与 34 目标 Semgrep 均通过；仍须等待新的精确 SHA
5/5 Hosted CI 与远端复审，才允许合并。

## RustSec

`cargo audit 0.22.2 --no-fetch` 使用本地 1169 条 advisory 数据检查
381 个 lockfile dependencies，exit `1`：

- `RUSTSEC-2026-0194`、`RUSTSEC-2026-0195`：quick-xml `0.38.4`；
- `RUSTSEC-2023-0071`：rsa `0.9.7`。

基线 `Cargo.lock` 已包含上述版本。P08 增加 `rand_core`、`serde_json` 及测试用
`sqlx`/`trpg-data-eventing` 的已有版本依赖边，没有升级或引入上述受影响版本。
本报告保留 audit 失败，不用 Semgrep 结果覆盖依赖风险，也不宣称 Hosted CI 或发布签署。

## 独立复核结论

在 Semgrep 最终覆盖范围内未发现阻断项；CodeRabbit 因未认证未执行；GitHub 九轮
自动审查先后提出的 4、5、5、4、2、3、5、3、4 项阻断均已修复或完成本地验证，最新
提交的远端复审尚待运行；
RustSec 的三个基线 advisory 仍需在独立依赖治理批次处理。P08 的功能验收结论依赖
真实测试和数据库证据，不依赖预写状态或单一第三方工具。
