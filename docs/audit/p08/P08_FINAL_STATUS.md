BATCH_STATUS: COMPLETE
BATCH_ID: P08

# P08 最终验收状态

记录日期：2026-07-27（Australia/Brisbane）

```text
BASE_HEAD = 18825746082886a63aee10891860aedb749349e1
WORK_BRANCH = agent/p08-combat-chase-conclusion
AUD_031 = CLOSED_PASS
AUD_032 = CLOSED_PASS
AUD_036 = CLOSED_PASS
AUD_043 = CLOSED_PASS
P08_REQUIRED_COMMANDS = PASS
REAL_POSTGRESQL_WITNESS = PASS
P08_PROJECTION_REBUILD = PASS
P08_SCHEMA_ASSERTION = PASS
FORWARD_MIGRATION_UPGRADE = PASS
P07_INVESTIGATION_SAN_CHARACTER_VISIBILITY_REGRESSION = PASS
DEPENDENCY_DIRECTION_POLICY = PASS_NO_EXCEPTION
THIRD_PARTY_SEMGREP = PASS_0_FINDINGS
CODERABBIT_EXTERNAL_REVIEW = NOT_RUN_NOT_AUTHENTICATED
DEPENDENCY_ADVISORY_SCAN = FAIL_3_DISCLOSED_BASELINE_ADVISORIES
GITHUB_PR = 9
INITIAL_HOSTED_CI = PASS_5_OF_5
GITHUB_INITIAL_AUTOMATED_REVIEW = 4_ACTIONABLE_FIXED
GITHUB_SECOND_AUTOMATED_REVIEW = 5_ACTIONABLE_FIXED
GITHUB_THIRD_AUTOMATED_REVIEW = 5_ACTIONABLE_FIXED
GITHUB_FOURTH_AUTOMATED_REVIEW = 4_ACTIONABLE_FIXED
GITHUB_FIFTH_AUTOMATED_REVIEW = 2_ACTIONABLE_FIXED
GITHUB_SIXTH_AUTOMATED_REVIEW = 3_ACTIONABLE_FIXED
GITHUB_SEVENTH_AUTOMATED_REVIEW = 5_ACTIONABLE_FIXED_CONFIRMED_BY_EIGHTH_REVIEW
GITHUB_EIGHTH_AUTOMATED_REVIEW = 3_ACTIONABLE_FIXED_CONFIRMED_BY_NINTH_REVIEW
GITHUB_NINTH_AUTOMATED_REVIEW = 4_ACTIONABLE_FIXED_CONFIRMED_BY_TENTH_REVIEW
GITHUB_TENTH_AUTOMATED_REVIEW = 2_ACTIONABLE_FIXED_CONFIRMED_BY_ELEVENTH_REVIEW
GITHUB_ELEVENTH_AUTOMATED_REVIEW = 2_ACTIONABLE_FIXED_LOCALLY
GITHUB_LATEST_AUTOMATED_REVIEW = RERUN_PENDING
THIRD_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
FOURTH_REPAIR_HOSTED_CI = PASS_2_OF_5_3_CANCELED_AFTER_REVIEW_BLOCKERS
FIFTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
SIXTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
SEVENTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
EIGHTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
NINTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
TENTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
ELEVENTH_REPAIR_HOSTED_CI = PENDING
P09_IMPLEMENTATION = NOT_STARTED
```

P08 已完成 Combat、Chase、Reconsideration、Fork、Ending/Growth 和 Tutorial
完整流程。正式状态继续经过
`Command -> Workflow -> Decision -> Event Store -> Projection`，投影可从经过
HMAC 与 Witness 校验的正史事件重建。

## 验收矩阵

| 验收项 | 代码与真实证据 | 状态 |
| --- | --- | --- |
| MajorWound 持续 | 聚合保存 prior condition；后续小伤或护甲全吸收不会清除；医疗必须由当前可行动治疗者以持久化 First Aid/Medicine 目标和服务端骰尝试，失败也留痕并消费回合/骰，只有成功事件可清除 | PASS |
| 多角色战斗 | DEX 仅用于先攻；正式近战/射击/闪避分别绑定持久化的 Melee/Firearm/Dodge 技能；Fight Back 实现与 Dodge 不同的平手规则和防守方反击伤害目标；miss/成功 Dodge 以 `ATTACK_MISSED` 无伤害转换保存攻击/防御骰且拒绝伤害骰；一次攻击即消费当前回合动作，`advance_turn` 前不能再次攻击；`DYING/DEAD` 目标不能 Dodge/Fight Back；跨轮次推进、伤害、护甲和终态均有测试；骰证据、outcome、serialized replay 与持久层逐项独立重算 | PASS |
| Chase 终态 | `Escaped`/`Caught` 后普通推进失败；新追逐必须使用新 ID；每名参与者结果由 opaque 服务端 percentile evidence 和 MOV 派生，调用方不能提交成功布尔值；全局消费投影拒绝跨 segment、跨 aggregate 以及 Combat/Chase/Growth 间复用骰 ID | PASS |
| 复议追加链 | Request → Review → Upheld/Corrected 均为正式事件；请求者必须能查看源事件，源事件与整条复议链的 Visibility/subject/data subject 完全一致；review/resolution 在事件创建前统一 trim，live projection 与删除后 replay 一致；精确重试幂等，原事件不删除 | PASS |
| Fork 范围与 Hash | 来源快照 hash 被重新计算并精确匹配请求；角色状态由截止序列前的 verified canonical events 重建；单事件只保存有界的内容寻址引用，实际数据按大小受限的正式事件批次物化；私密 scope 以及 `keeper_only` 角色/角色卡均被排除 | PASS |
| Fork 实体化与重放 | 子 Campaign 实际创建 scenario、character/sheet、ended session、scenes、public events、clues、NPC、combat、chase、conclusion 和 manifest；参与者、先攻、转换及 roll 引用全部改写为确定性 child-owned ID；同版本污染与 ghost 行会先被删除，再从子 Campaign 正史逐字节重建；P08 rebuild 只替换 immutable fork target ID，不删除 fork 后正常创建的实体 | PASS |
| Fork child lineage 唯一性与连接池 | canonical Event Store 对每个 child Campaign 的 `CampaignForkRecorded` 建立 partial unique index，projection 另有 `UNIQUE(child_campaign_id)`；同一 child 的所有 canonical INSERT 还经过共享事务 advisory lock，Fork 插入时在锁内重新验证只存在创建/邀请基线，封闭 emptiness preflight TOCTOU；snapshot/build/canonical commit/replay-page load 均不持有投影池连接；真实竞争与 `max_connections=1` 均通过 | PASS |
| Fork cutoff 隔离 | `source_cutoff_event_sequence` 只用于确定上界；实际 base event set 由来源 Session ID、其 Scene/Action 归属和 Session 启动前 campaign baseline 组成；顶层字段与 `data` 包装两种 canonical payload 都能解析；即使第二 Session 的事件先写入、第一 Session 的 Ending/Growth 后写入，也不会把第二 Session 纳入旧快照；cutoff 后相关公开复议链仍单独加入 | PASS |
| 可见性保持 | Fork materialization 按 keeper、party 和 owner-bound private 行分批；每个事件自己的 Visibility、`data_subject_id` 与主体密钥进入 request hash、HMAC、Event Store 和 Outbox，投影触发器继续要求事件/行完全一致 | PASS |
| 幂等与语义唯一性 | Combat、Chase、Ending、Growth 的 exact retry 返回原 persisted commit；Ending 的 Session 键与 Growth 的 Character 键在事务 advisory lock 下串行检查、append 和 projection；真实并发竞争各只产生一条正史 | PASS |
| 活跃会话边界 | Combat/Chase 在同一事务内对 Session 行持有 `FOR SHARE` 锁并要求状态精确为 `ACTIVE`；Session 终止路径的 `FOR UPDATE` 锁封闭状态检查与正式 append 间的 TOCTOU；结束态负例不增加 Event Store | PASS |
| 场景参与者唯一性 | Scenario 验证在接受 Combat/Chase encounter 前拒绝重复 participant ID，保证通过验证的 encounter 可构造正式聚合 | PASS |
| 结局与成长 | 活跃会话不能结局；`ending_id` 必须存在于会话绑定场景的 `endings`；Ending summary 在事件创建前规范化并与 replay 投影一致；成长技能还必须存在于该 Ending 的 `growth_awards`；结果从共享内核不可构造的 OS CSPRNG 证据计算，并生成新锁定角色卡版本 | PASS |
| Tutorial 完整闭环 | 真实 PostgreSQL 上完成角色、场景、调查、服务端骰、线索、SAN、战斗、追逐、结局、成长、复议和 Fork | PASS |
| Schema/最小权限 | 五个 forward migration、projection guards、受秘密 capability 与 canonical target 约束的 Growth rewind、可延迟外键、成长算术/证据约束、完整 Fork scope 表、canonical/projection child lineage 唯一约束、fork-empty trigger/function 完整 catalog 指纹、Combat/Chase/Growth 全局 gameplay roll 主键及角色权限断言 | PASS |
| 第三方检查 | Semgrep 1.171.0 本机复扫 34 个 P08 Rust/SQL/CI 目标，13 条适用规则，0 finding、0 error、0 skipped；PR #9 十一轮远端自动审查先后提出 4、5、5、4、2、3、5、3、4、2、2 项真实问题，前十轮修复已由下一轮确认，第十一轮已完成本地根因修复并等待精确 SHA 复审 | PASS_WITH_REMOTE_RERUN_PENDING |

## 反伪造修复

- 原先只验证一行 snapshot 的 Fork 已替换为子 Campaign 所有的正式事件批次、实际实体和可重放 manifest。
- 原先塞入单个事件的完整 snapshot 已替换为内容寻址引用；实际物化事件同时限制每批
  行数与序列化字节数，防止超过 Event Store 的 1 MiB 事件上限。
- Fork 角色快照不再按当前 projection 的 `last_event_sequence` 过滤；它从经过完整
  HMAC/Witness 校验的 Event Store 回放到 source cutoff，因此角色在后续 Session
  发生 SAN/Growth 后不会从旧快照消失，也不会把新状态倒灌进旧分支。
- 相关复议的 resolution sequence 不再用 `GREATEST` 抬高全局 source cutoff；base
  snapshot 从 verified Event Store 构造来源 Session 专属事件集合：来源 Session
  启动前的 campaign baseline 加上该 Session ID、Scene 与 Action 绑定事件。真实负例
  把第二 Session 的 start/end 插在第一 Session 的 Ending/Growth 之前，仍证明其
  Session/Ending/Growth 不会泄漏进旧分支；cutoff 后相关公开复议链再按 ID 单独加入。
- 默认声明的 Public events、Clues、NPC、Combat、Chase 和 Conclusion 不再只出现在
  hash/manifest 中；它们都有子 Campaign 正式事件、受保护投影和删除后重放证据。
- Fork 子实体不再全部继承 keeper-only command envelope；scenario、session/scene、
  character/sheet 使用来源可见性或更严格的安全派生标签，owner-bound 私有行仍只对
  原 owner 可见。
- owner-bound 私有 materialization 不再使用 command-wide 的
  `data_subject_id=not_applicable` 或通用密钥；逐事件主体绑定贯穿请求 hash、加密、
  Event Store、Outbox 和 replay。
- Combat、Chase、Ending、Growth 的 exact retry 不再因 projection 已存在而误报
  conflict；场景文档外的任意 `ending_id` 在 Event Store append 前被拒绝。
- 同一 Session 的第二个 ending ID，以及同一 ending/character/skill 的第二个
  growth ID，都会在正式事件 append 前按语义键拒绝；负例同时断言 Event Store 行数
  不增加。
- 默认 Fork 的角色查询同时约束角色行和当前角色卡的 Visibility；真实数据库负例证明
  `keeper_only` 角色名称与角色卡 sentinel 均不会进入快照。
- 原先可由调用方提交的 Combat/Chase JSON 已替换为严格 shape 与前驱转换校验；同 ID 的异源聚合会在 Event Store append 前失败。
- Combat 不再接受原始伤害值，Chase 不再接受成功布尔值；Combat 的攻击/闪避/伤害和
  Chase 的每名参与者骰均由字段私有、不可反序列化的共享内核 OS CSPRNG 对象生成。
  状态 JSON 保存完整证据，规则 replay、领域 replay 和持久层绑定三次独立验证。
- Combat 的命中目标不再错误复用 DEX；Melee、Firearm 与 Dodge 技能随参与者进入正式
  聚合并由重放层独立验证。Fight Back 保存派生 outcome，防守方只有达到更高成功等级
  才反击，平手由发起攻击者获胜；伪造反击 outcome 在 Event Store append 前失败。
- Fight Back 若使当前攻击者进入 `DYING` 或 `DEAD`，聚合与独立领域重放都会拒绝其
  在 `advance_turn` 前再次攻击；失败尝试不改变状态或版本。
- 攻击失败或 Dodge 成功不再作为错误丢弃；`ATTACK_MISSED` 正式转换保存服务端攻击/
  防御骰、保持 HP/condition 不变并推进聚合版本。miss 路径拒绝伤害骰，独立领域
  replay 会拒绝把成功命中伪装成 miss。
- 攻击命中或失败都会把当前回合动作标记为已消费；只有正式 `TurnAdvanced` 转换会
  重置该标记。规则聚合与独立领域 replay 均拒绝同一角色在推进前进行第二次攻击。
- Dodge/Fight Back 不再只检查防御骰 presence；目标若已为 `DYING/DEAD`，主动防御
  在规则层和独立 replay 层均失败，不能取消伤害或反击。
- Combat、Chase 与 Growth 的每个正式骰同时写入 `gameplay_roll_consumptions`
  全局唯一投影；新攻击、治疗尝试、chase segment 或成长检查在 append 前按 roll ID
  排序加锁，并检查本次
  内部重复及全局主键。服务端骰对象即使被 clone，也不能跨版本、跨 aggregate、
  跨 Campaign 或在 Combat/Chase/Growth 类型间再次产生正式结果。
- MajorWound 恢复不再接受调用方提交的任意 `medical_target`；API 要求当前治疗者和
  First Aid/Medicine 类型，从治疗者的持久化技能派生目标。失败尝试同样形成正式
  mutation、消费回合及 roll ID，而不会静默丢弃证据。
- Combat/Chase 正式写入不再只校验 Session 存在；同一投影事务锁定 Session 行并要求
  `ACTIVE`，因此 `SCHEDULED`、`PAUSED`、`ENDED` 均不能产生玩法正史。Tutorial 的
  结束态负例同时断言两类事件计数不变。
- Scenario encounter 在入口拒绝重复 participant ID，避免文档验证通过后才在正式
  Combat/Chase 聚合构造阶段失败。
- 原先可提交原始成长数值的路径已替换为不可反序列化、字段私有的服务端随机证据；持久层从当前角色卡重新计算结果。
- Growth 不再只验证角色卡里存在技能，还要求技能精确出现在所选 Ending 的 Scenario
  `growth_awards`；未授予技能在 append 前失败。
- Ending 与 Growth 的“先查再写”窗口已用事务级语义键 advisory lock 封闭；并发负例
  证明竞争失败方不会留下 canonical orphan。
- Fork 不再在 snapshot 构造、canonical commit 或 replay-page 解密期间长期占用
  projection pool connection；Event Store 的 child Campaign partial unique index
  与 projection 的 `UNIQUE(child_campaign_id)` 先保证唯一 lineage，最终投影再用短
  child/rebuild lock 事务原子落地。真实竞争仍只有一个成功，且
  `max_connections=1` 的回归证明不存在嵌套租用造成的池耗尽。
- Fork 的 Combat/Chase participant、initiative order、攻击/治疗/转换和 roll
  participant 引用都会映射到确定性 child character/NPC ID；来源 ID 不会留在子级
  gameplay state。
- Ending summary、Reconsideration review summary 与 resolution 在创建 canonical
  event 前只规范化一次；live projection 与 replay 使用相同值，带首尾空白的真实
  数据库用例在删除投影后仍逐字节一致。
- 复议请求不再只检查 Campaign membership 和 source sequence 存在；SQL 授权同时
  验证源事件 Visibility、subject、data subject 与请求者，并要求新事件 envelope
  精确继承。review/resolve 继续与上一条链事件三项一致，猜测 keeper/private sequence
  返回统一 NotFound 且不会追加事件。
- Tutorial 不使用手写事件字符串数组冒充 E2E；它连接独立 primary/Witness 数据库并检查 Event Store、Outbox、formal commits、HMAC 和 Witness。
- P08 投影重建在 campaign-scoped 锁内清除 Combat、Chase、Growth、Ending、
  Reconsideration、全局骰消费以及 Fork 专属物化读模型；非 Fork 的 Growth 先通过
  secret capability、精确 canonical target 和仅 Growth 后缀约束回退角色，再删除并
  重建成长角色卡。Fork 基础 scenario/character/sheet/session/scene 只按 immutable
  materialization 中的确定 ID 删除重放，fork 后由其他正式工作流新增的行保持不变。
- Fork exact retry 的 manifest 行数不再统计整个 child Campaign；它只把该 fork 的
  verified `CampaignForkMaterialized` projection targets 与实际行做精确交集，因此
  后续新增 Session/Scene/Character 不会让幂等重试误报。
- Fork source-session 事件归属不再假定所有 canonical payload 都使用 `data` 包装；
  字段读取同时支持 `PlayerActionSubmitted` 的顶层 shape 和领域事件的嵌套 shape。
  Tutorial 真实 E2E 逐项验证选中 Session 的 SAN action、依赖的
  `SanityLossApplied`、快照角色卡和 child materialization 均保留正确 SAN。
- Fork child emptiness 不再停留在可竞态的 projection preflight。Event Store 的
  BEFORE INSERT trigger 对同一 child 的全部 canonical write 获取同一事务锁，并在
  `CampaignForkRecorded` 插入线性化点重新检查 verified/formal 历史；确定性真库
  竞争先让普通 Scenario write 排队，再让已通过 preflight 的 Fork 排队，释放屏障后
  只允许普通正史和投影成功，Fork 正史保持为零。触发器 catalog 与函数完整正文/
  执行属性/所有权/安全 `search_path` 都进入精确指纹，不能靠保留关键字伪造门禁。
- 人为污染上述每类同版本行并插入 ghost 后，重建前后实际 JSON（包含角色卡和全局骰
  消费投影）一致，Event Store 行数不变；任一步失败则整笔事务回滚。

## 变更范围

- `trpg-ruleset-coc7`：持续 Combat/Chase 聚合、基础战斗/追逐规则和成长裁决。
- `trpg-shared-kernel`：不可构造的服务端 percentile/d10 随机证据。
- `trpg-domain-core`：严格 serialized-state 校验、复议事件、Fork scope/lineage。
- `trpg-runtime`：结束/成长状态机与独立成长结果校验。
- `trpg-data-eventing`：Combat/Chase/Reconsideration/Fork/Ending/Growth 的正式提交、投影和重放。
- `trpg-testing`、CI、Tutorial fixture、forward migrations、schema assertions 和本批次证据。

共享内核 RNG 与领域校验器是为修复生产依赖方向所需的最小接口调整。
`trpg-data-eventing -> trpg-ruleset-coc7` 和
`trpg-runtime -> trpg-ruleset-coc7` 均只保留为 dev dependency；没有添加
依赖白名单或弱化架构检查。

## 风险与回滚

规则边界固定为当前 COC7 基础战斗、追逐和技能成长。应用回滚应停止注册对应
command handler，保留已提交事件，并从同版本事件恢复投影；不得使用删除正史的
down migration。

`cargo audit --no-fetch` 仍以 exit `1` 报告基线已存在的
`RUSTSEC-2026-0194`、`RUSTSEC-2026-0195`（quick-xml 0.38.4）和
`RUSTSEC-2023-0071`（rsa 0.9.7）。P08 只增加已有版本的依赖边，不改变这两个包的
锁定版本，因此没有把该扫描伪报为通过。CodeRabbit CLI 的浏览器回调认证未完成，
未运行或冒充 CodeRabbit 结果。PR #9 的原 P08 提交曾通过 5/5 Hosted CI；第三轮
修复提交在 3/5 workflow 已通过时被第四轮审查阻断，第四轮修复提交
`ea760c1` 在 repository-truth 与 golden-scenarios 通过后又被第五轮审查阻断。
对应剩余 2 项和 3 项长任务均主动取消，未伪报为 5/5。第五轮提出的 Fork cutoff
扩大与失能攻击者重复行动问题已完成本地修复；对应提交 `fb3907e` 的
repository-truth、golden-scenarios、production-security 为 3/5 通过，第六轮审查
提出 miss 正史丢失、Fork child lineage 并发竞态、摘要事件/投影不一致 3 项后，
剩余 workspace/release 两项被主动取消。三项修复提交 `2ed9df2` 的
repository-truth、golden-scenarios、production-security 为 3/5 通过；第七轮精确
SHA 审查继续指出复议源事件可见性、单回合重复攻击、失能目标主动防御、调用方自报
医疗目标和骰 ID 跨版本复用 5 项，剩余 workspace/release 再次主动取消，未计为
5/5。对应修复提交 `56b648b` 的 repository-truth、golden-scenarios、
production-security 为 3/5 通过；第八轮精确 SHA 审查未重复上述五项，但指出
selected Session cutoff 仍会纳入交错 Session、同版本损坏/ghost 投影不能由 rebuild
修复、骰 ID 仍可跨 aggregate/Combat/Chase 复用 3 项，workspace/release 因阻断主动
取消，未计为通过。对应提交 `f1b0e70` 的 repository-truth、golden-scenarios、
production-security 为 3/5 通过；第九轮精确 SHA 审查确认上述三项未重复，但指出
其余 P08 投影未清除、Growth 未加入跨类型骰消费、Fork gameplay 保留 parent ID，
以及长期持有连接会耗尽 20-connection pool 共 4 项。对应修复提交 `3b90578` 的
repository-truth、golden-scenarios、production-security 为 3/5 通过；第十轮精确
SHA 审查确认上述四项未重复，但指出 rebuild 会删除 fork 后的正常 child 状态，且
exact retry 的 manifest 行数错误覆盖整个 child Campaign。workspace/release 因这
两个阻断主动取消，未计为通过。提交 `4250462` 按 fork-owned canonical target
修复后，repository-truth、golden-scenarios、production-security 为 3/5 通过；
第十一轮精确 SHA 审查确认第十轮两项未重复，但指出顶层
`PlayerActionSubmitted` 字段未被 source-session 回放读取，以及 projection
emptiness preflight 与普通 child canonical write 之间仍有 TOCTOU。workspace/release
随即取消，未计为通过。第十一轮两项现已以双 shape replay 和 Event Store 插入线性化
门禁修复，并从全新 PostgreSQL/Witness 连续通过两次。诊断性的全 workspace test
因未启动 CI 专用 P02 服务而失败，未计为通过；新的 Hosted CI 与精确 SHA 远端自动
复审仍须在合并前通过。P08 到此停止，未执行 P09。
