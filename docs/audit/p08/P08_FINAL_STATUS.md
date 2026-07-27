BATCH_STATUS: COMPLETE
BATCH_ID: P08

# P08 最终验收状态

记录日期：2026-07-28（Australia/Brisbane）

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
THIRD_PARTY_SEMGREP = BASELINE_PASS_ROUND_22_NOT_RUN_EXTERNAL_FETCH_REJECTED
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
GITHUB_ELEVENTH_AUTOMATED_REVIEW = 2_ACTIONABLE_FIXED_CONFIRMED_BY_TWELFTH_REVIEW
GITHUB_TWELFTH_AUTOMATED_REVIEW = 2_ACTIONABLE_FIXED_CONFIRMED_BY_THIRTEENTH_REVIEW
GITHUB_THIRTEENTH_AUTOMATED_REVIEW = 2_ACTIONABLE_FIXED_CONFIRMED_BY_FOURTEENTH_REVIEW
GITHUB_FOURTEENTH_AUTOMATED_REVIEW = 1_ACTIONABLE_FIXED_CONFIRMED_BY_FIFTEENTH_REVIEW
GITHUB_FIFTEENTH_AUTOMATED_REVIEW = 3_ACTIONABLE_FIXED_CONFIRMED_BY_SIXTEENTH_REVIEW
GITHUB_SIXTEENTH_AUTOMATED_REVIEW = 2_ACTIONABLE_FIXED_CONFIRMED_BY_SEVENTEENTH_REVIEW
GITHUB_SEVENTEENTH_AUTOMATED_REVIEW = 2_ACTIONABLE_FIXED_CONFIRMED_BY_EIGHTEENTH_REVIEW
GITHUB_EIGHTEENTH_AUTOMATED_REVIEW = 1_ACTIONABLE_FIXED_CONFIRMED_BY_NINETEENTH_REVIEW
GITHUB_NINETEENTH_AUTOMATED_REVIEW = 2_ACTIONABLE_FIXED_CONFIRMED_BY_TWENTIETH_REVIEW
GITHUB_TWENTIETH_AUTOMATED_REVIEW = 1_BLOCKING_FIXED_CONFIRMED_BY_TWENTY_FIRST_REVIEW_2_NONBLOCKING_DEFERRED
GITHUB_TWENTY_FIRST_AUTOMATED_REVIEW = 3_ACTIONABLE_FIXED_CONFIRMED_BY_TWENTY_SECOND_REVIEW
GITHUB_TWENTY_SECOND_AUTOMATED_REVIEW = 2_ACTIONABLE_FIXED_CONFIRMED_BY_TWENTY_THIRD_REVIEW
GITHUB_TWENTY_THIRD_AUTOMATED_REVIEW = 2_ACTIONABLE_FIXED_CONFIRMED_BY_TWENTY_FOURTH_REVIEW
GITHUB_TWENTY_FOURTH_AUTOMATED_REVIEW = 3_ACTIONABLE_FIXED_CONFIRMED_BY_TWENTY_FIFTH_REVIEW
GITHUB_TWENTY_FIFTH_AUTOMATED_REVIEW = 2_ACTIONABLE_FIXED_LOCALLY
GITHUB_LATEST_AUTOMATED_REVIEW = RERUN_PENDING
THIRD_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
FOURTH_REPAIR_HOSTED_CI = PASS_2_OF_5_3_CANCELED_AFTER_REVIEW_BLOCKERS
FIFTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
SIXTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
SEVENTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
EIGHTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
NINTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
TENTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
ELEVENTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
TWELFTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
THIRTEENTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
FOURTEENTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
TWENTY_FIRST_REPAIR_HOSTED_CI = PASS_5_OF_5
TWENTY_SECOND_REPAIR_HOSTED_CI = PASS_3_OF_5_2_RUNNING_AT_REVIEW_CUTOFF
TWENTY_THIRD_REPAIR_HOSTED_CI = PASS_3_OF_5_2_RUNNING_AT_REVIEW_CUTOFF
TWENTY_FOURTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_RUNNING_AT_REVIEW_CUTOFF
TWENTY_FIFTH_REPAIR_HOSTED_CI = PENDING_LOCAL_COMMIT
P09_IMPLEMENTATION = NOT_STARTED
```

P08 已完成 Combat、Chase、Reconsideration、Fork、Ending/Growth 和 Tutorial
完整流程。正式状态继续经过
`Command -> Workflow -> Decision -> Event Store -> Projection`，投影可从经过
HMAC 与 Witness 校验的正史事件重建。

## 验收矩阵

| 验收项 | 代码与真实证据 | 状态 |
| --- | --- | --- |
| MajorWound 持续 | 聚合保存 prior condition；后续小伤或护甲全吸收不会清除；医疗必须由当前可行动治疗者以持久化 First Aid/Medicine 目标和服务端骰尝试，失败也留痕并消费回合/骰；成功 First Aid 可把 0 HP `Dying` 稳定为 1 HP `MajorWound`，不能用 Medicine 跳过急救，也不能把幸存者直接伪造成 `Able` | PASS |
| 多角色战斗 | DEX 仅用于先攻；正式近战/射击/闪避分别绑定持久化的 Melee/Firearm/Dodge 技能；新遭遇必须显式接收经校验的 `CombatHealth(current_hp,max_hp,condition)`，可从上一 combatant 的 health snapshot 原样传递，不能隐式恢复满血或清除 MajorWound/Dying/Dead；每个参与者的选定近战/射击武器 ID 与伤害公式进入正式状态，伤害按实际命中者或反击者的对应武器重算，支持验收 fixture 的 `1d6+1` 且拒绝 action-kind 默认值冒充；Fight Back 实现与 Dodge 不同的平手规则和防守方反击伤害目标；miss/成功 Dodge 以 `ATTACK_MISSED` 无伤害转换保存攻击/防御骰且拒绝伤害骰；一次攻击即消费当前回合动作，`advance_turn` 前不能再次攻击；`DYING/DEAD` 目标不能 Dodge/Fight Back；跨轮次推进、伤害、护甲和终态均有测试；骰证据、outcome、serialized replay 与持久层逐项独立重算 | PASS |
| Chase 终态 | `Escaped`/`Caught` 后普通推进失败；新追逐必须使用新 ID；每名参与者结果由 opaque 服务端 percentile evidence 和 MOV 派生，调用方不能提交成功布尔值；全局消费投影拒绝跨 segment、跨 aggregate 以及 Combat/Chase/Growth 间复用骰 ID | PASS |
| 复议追加链 | Request → Review → Upheld/Corrected 均为正式事件；请求者必须能查看源事件，源事件与整条复议链的 Visibility/subject/data subject 完全一致；review/resolution 在事件创建前统一 trim，live projection 与删除后 replay 一致；精确重试幂等，原事件不删除 | PASS |
| Fork 范围与 Hash | 来源快照 hash 被重新计算并精确匹配请求；角色状态由截止序列前的 verified canonical events 重建；单事件只保存有界的内容寻址引用，实际数据按大小受限的正式事件批次物化；私密 scope 以及 `keeper_only` 角色/角色卡均被排除 | PASS |
| Fork 实体化与重放 | 子 Campaign 实际创建 scenario、character/sheet、ended session、scenes、public events、clues、NPC、combat、chase、conclusion 和 manifest；已实现结局的完整 `growth_awards` 与按来源角色/技能记录的消费标记被纳入内容寻址快照，materialization 将标记改写到 child-owned character；已消费的 Library Use 不能在 child 重复成长，未消费的 Psychology 仍可正式结算；参与者、先攻、转换及 roll 引用全部改写为确定性 child-owned ID；同版本污染与 ghost 行会先被删除，再从子 Campaign 正史逐字节重建；P08 rebuild 只清理 canonical tip 仍由 P08 拥有的共享投影，fork 复制角色后来发生的 SAN 角色/sheet/action 逐字节保持 | PASS |
| Fork child lineage、Authority 与连接池 | canonical Event Store 对每个 child Campaign 的新 v2 `CampaignForkRecorded` 建立 partial unique index，HMAC-bound materialization projection target 是 child-owned 判别，旧 parent-owned 多 child 历史不会在升级建索引时冲突；projection 另有 `UNIQUE(child_campaign_id)`；同一 child 的所有 canonical INSERT 还经过共享事务 advisory lock，Fork 插入时在锁内重新验证只存在创建/邀请基线，封闭 emptiness preflight TOCTOU；正式 lineage 还要求 child 的锁定/FORK_ONLY Authority Contract 为共享内核 `fork_for_child` 生成的确定性 ID、version 1、父级全部规则/安全/模型/角色卡快照一致及精确 `+1ms` 创建时间，另建 Campaign 不能冒充分支；snapshot/build/canonical commit/replay-page load 均不持有投影池连接；真实 legacy upgrade、并发竞争与 `max_connections=1` 均通过 | PASS |
| Fork cutoff 隔离 | `source_cutoff_event_sequence` 只用于确定上界；实际 base event set 由来源 Session ID、其 Scene/Action 归属和 Session 启动前 campaign baseline 组成；顶层字段与 `data` 包装两种 canonical payload 都能解析；即使第二 Session 的事件先写入、第一 Session 的 Ending/Growth 后写入，也不会把第二 Session 纳入旧快照；cutoff 后相关公开复议链仍单独加入 | PASS |
| 可见性保持 | Fork materialization 按 keeper、party 和 owner-bound private 行分批；每个事件自己的 Visibility、`data_subject_id` 与主体密钥进入 request hash、HMAC、Event Store 和 Outbox，投影触发器继续要求事件/行完全一致 | PASS |
| 幂等与语义唯一性 | Combat、Chase、Ending、Growth 的 exact retry 返回原 persisted commit；Campaign Fork 的 exact retry 从已记录 lineage/manifest/materialized batches 重建原命令与投影，不读取后来可能变化的 parent snapshot；Ending 的 Session 键与 Growth 的 Character 键在事务 advisory lock 下串行检查、append 和 projection；真实并发竞争各只产生一条正史 | PASS |
| 活跃会话边界 | Combat/Chase 在同一事务内对 Session 行持有 `FOR SHARE` 锁并要求状态精确为 `ACTIVE`；Session 终止路径的 `FOR UPDATE` 锁封闭状态检查与正式 append 间的 TOCTOU；结束态负例不增加 Event Store | PASS |
| Fork 结算边界 | 来源 Session 的快照预览与最终 materialization 都要求所有 Combat 已为 `ENDED`，所有 Chase 已为 `ESCAPED` 或 `CAUGHT`；仍在进行的玩法状态以 `fork_source_gameplay_not_terminal` 在写入 child 正史前拒绝，避免生成携带不可继续 ENDED Session 的死分支 | PASS |
| 场景结构唯一性 | Scenario 验证在接受 Combat/Chase encounter 前拒绝重复 participant ID，并要求参与者 ID 与状态机一致：仅 ASCII 字母数字、`_`、`-` 且不超过 128 字节；每个 Ending 还拒绝重复、空白或超过持久层 128 字节上限的 `growth_awards.skill_name`，保证入口接受的文档可构造正式聚合、fork conclusion snapshot 并可实际结算 | PASS |
| 结局与成长 | 活跃会话不能结局；`ending_id` 必须存在于会话绑定场景的 `endings`，并在事件创建前统一规范化后写入 canonical event 与 projection；Ending summary 同样规范化并与 replay 投影一致；没有 `growth_awards` 的合法结局可用空 settlement 完成，有奖励时成长技能必须存在于该 Ending 的 `growth_awards`，且 fork 继承的按角色/技能消费标记会在 append 前阻止重复领取；成长证据只能由一次性完整 OS CSPRNG 尝试生成，不能把独立 percentile/d10 拼装为挑选结果；新 Sheet、Character 与正式 Growth event 必须精确保持来源 Character/Sheet 的 Visibility envelope，任何扩大在 append 前失败 | PASS |
| Tutorial 完整闭环 | 真实 PostgreSQL 上完成角色、场景、调查、服务端骰、线索、SAN、战斗、追逐、结局、成长、复议和 Fork；另证明未终止 Combat/Chase 不能分叉、来源已消费成长不能在 child 重领而另一奖励仍可结算 | PASS |
| Schema/最小权限 | 八个 P08 forward migration、projection guards、受秘密 capability 与 canonical target 约束的 Growth/rebuild repair、可延迟外键、成长算术/证据约束、完整 Fork scope 表、canonical/projection child lineage 唯一约束、fork-empty trigger/function 完整 catalog 指纹、Combat/Chase/Growth 全局 gameplay roll 主键及 canonical 事务内 reservation、v2 Fork/Reconsideration 显式非空 shape 与行为探针、角色与函数执行权限断言 | PASS |
| 第三方检查 | Semgrep 1.171.0 的历史 34 目标基线与第二十一轮 5 changed targets 均为 13 rules/0 finding/0 error/0 skipped；第二十二轮因社区规则外联被安全审查拒绝且无本地缓存，明确 `NOT_RUN`。PR #9 的提交 `7acc802` 经第二十五轮精确 SHA review `4789695257` 确认第二十四轮三项未重复，并指出 legacy parent-owned 多分支会阻断 v2 唯一索引升级、场景可声明超过持久层上限的成长技能两项；均已完成本地根因修复和真库回归，等待新提交/CI/复审 | PASS_WITH_REMOTE_RERUN_PENDING_AND_CURRENT_SEMGREP_NOT_RUN |

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
- Combat participant 不再由只接收 `max_hp` 的构造器隐式初始化为满血/`ABLE`。
  `CombatHealth` 要求 current/max/condition 同时满足不变量，`CombatantState::health()`
  可把上一遭遇的持久化快照原样传入新遭遇；规则层与独立 serialized validator 都接受
  合法 MajorWound/Dying 初始态并拒绝零 HP + `ABLE` 等矛盾组合。
- 0 HP `Dying` 不再被唯一医疗入口无条件拒绝。只有 First Aid 可执行立即稳定；
  成功后由规则层和独立 replay 同时派生为 1 HP `MajorWound`，失败仍留在 `Dying`
  并消费动作/骰。Medicine 不能跳过该步骤，伪造为 `Able` 的状态无法 append。
- Combat 的命中目标不再错误复用 DEX；Melee、Firearm 与 Dodge 技能随参与者进入正式
  聚合并由重放层独立验证。Fight Back 保存派生 outcome，防守方只有达到更高成功等级
  才反击，平手由发起攻击者获胜；伪造反击 outcome 在 Event Store append 前失败。
- Combat 伤害公式不再按 `Melee=1d6`、`Firearm=1d6+5` 硬编码。选定武器 ID 与经
  范围校验的公式随参与者进入正式聚合；普通命中使用攻击者对应武器，Fight Back
  使用实际反击者的近战武器。规则前驱 replay 与独立领域 replay 都从持久化 loadout
  派生期望公式；验收 fixture 的 `1d6+1` 通过，使用另一方或 action 默认公式的骰证据
  在状态变更前失败。
- Fight Back 若使当前攻击者进入 `DYING` 或 `DEAD`，聚合与独立领域重放都会拒绝其
  在 `advance_turn` 前再次攻击；失败尝试不改变状态或版本。
- 攻击失败或 Dodge 成功不再作为错误丢弃；`ATTACK_MISSED` 正式转换保存服务端攻击/
  防御骰、保持 HP/condition 不变并推进聚合版本。miss 路径拒绝伤害骰，独立领域
  replay 会拒绝把成功命中伪装成 miss。
- 攻击命中或失败都会把当前回合动作标记为已消费；只有正式 `TurnAdvanced` 转换会
  重置该标记。规则聚合与独立领域 replay 均拒绝同一角色在推进前进行第二次攻击。
- Dodge/Fight Back 不再只检查防御骰 presence；目标若已为 `DYING/DEAD`，主动防御
  在规则层和独立 replay 层均失败，不能取消伤害或反击。
- Combat、Chase 与 Growth 的每个正式骰在 canonical event、audit、formal commit
  尚未提交的同一事务内写入 `gameplay_roll_consumptions` 全局 ownership reservation；
  HMAC-bound projection target、canonical-only `SECURITY DEFINER` 执行权及固定
  `search_path` 共同约束写入。普通状态 projection 失败或 task 取消后，正史中的骰
  ownership 仍不会释放；exact retry 可补齐状态且不重复事件。新攻击、治疗尝试、
  chase segment 或成长检查仍在 append 前按 roll ID 排序加锁并检查内部重复及全局
  主键，因此 clone 不能跨版本、aggregate、Campaign 或玩法类型再次进入正史。
- MajorWound 恢复不再接受调用方提交的任意 `medical_target`；API 要求当前治疗者和
  First Aid/Medicine 类型，从治疗者的持久化技能派生目标。失败尝试同样形成正式
  mutation、消费回合及 roll ID，而不会静默丢弃证据。
- Combat/Chase 正式写入不再只校验 Session 存在；同一投影事务锁定 Session 行并要求
  `ACTIVE`，因此 `SCHEDULED`、`PAUSED`、`ENDED` 均不能产生玩法正史。Tutorial 的
  结束态负例同时断言两类事件计数不变。
- Scenario encounter 在入口拒绝重复 participant ID，并使用与 Combat/Chase
  状态机一致的 ASCII 字母数字、`_`、`-`、最多 128 字节约束；每个 Ending 还拒绝
  重复或超过持久层 128 字节上限的 `growth_awards.skill_name`，避免文档验证通过后
  才在正式聚合、fork conclusion snapshot 构造或 Growth 持久化阶段失败。
- 原先可提交原始成长数值或把独立 percentile/d10 拼装成挑选结果的路径，已替换为
  不可反序列化、字段私有、一次性完整采样的服务端随机证据；原始 d10 生成和组合
  构造器均不公开，持久层仍从当前角色卡独立重算结果。
- Growth command 不再能用调用方 metadata 改写 Character/Sheet 的 Visibility。
  正式 append 前同时加载 Character 与当前 Sheet 的 label/subject；来源二者必须
  一致，命令还必须精确保持该 envelope。真实 public widening 负例证明 Event Store
  不增、私密来源和 current Sheet 不变，随后合法 owner-private 调用才成功。
- Growth 不再只验证角色卡里存在技能，还要求技能精确出现在所选 Ending 的 Scenario
  `growth_awards`；未授予技能在 append 前失败。
- Fork conclusion snapshot 不再只保留 ending ID/summary；实现结局的完整
  `growth_awards`、原因及来源角色已经消费的技能进入 canonical snapshot、hash 和
  child scenario，并拒绝空值、重复技能或畸形结构；消费标记随角色映射改写为
  child-owned ID。真实 Tutorial 证明已消费的 Library Use 在 child append 前拒绝，
  尚未结算的 Psychology 仍可使用 forked ending、character/sheet 与服务端骰完成。
- Ending 与 Growth 的“先查再写”窗口已用事务级语义键 advisory lock 封闭；并发负例
  证明竞争失败方不会留下 canonical orphan。
- Fork 不再在 snapshot 构造、canonical commit 或 replay-page 解密期间长期占用
  projection pool connection；Event Store 的 child Campaign partial unique index
  只接受带 HMAC-bound materialization target 的 child-owned v2 lineage，旧版把多个
  child 记录在同一 parent stream 的合法历史不会在 forward upgrade 时互相冲突；
  projection 的 `UNIQUE(child_campaign_id)` 继续保证读模型唯一，最终投影再用短
  child/rebuild lock 事务原子落地。真实 legacy 双分支升级通过，并发竞争仍只有一个
  成功，且 `max_connections=1` 的回归证明不存在嵌套租用造成的池耗尽。
- Fork lineage 不再只相信请求提供的 child Campaign ID。正式写入前同时读取父子
  Authority Contract，并要求 child 的确定性 contract ID、version、创建时间和全部
  immutable snapshot 字段精确符合共享内核 `fork_for_child`；独立创建的 Campaign
  即使空且调用者有权限，也不能冒充该父级的分支。
- Fork preview 与 materialization 都会检查来源 gameplay 终态；任何仍为
  `ONGOING` 的 Combat/Chase 都在 child 正史写入前拒绝，避免把活跃聚合复制进
  一个状态固定为 `ENDED`、无法继续推进的 child Session。
- Fork 的 Combat/Chase participant、initiative order、攻击/治疗/转换和 roll
  participant 引用都会映射到确定性 child character/NPC ID；来源 ID 不会留在子级
  gameplay state。
- Ending ID/summary、Reconsideration review summary 与 resolution 在创建 canonical
  event 前只规范化一次；live projection 与 replay 使用相同值，带首尾空白的真实
  数据库用例证明 canonical event、投影和 fork snapshot 使用同一 Ending ID。
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
- Fork exact retry 不再重新读取会随父 Campaign 后续 Growth/Reconsideration 改变的
  source snapshot。检测到 canonical lineage 后，它从 verified
  `CampaignForkRecorded`、materialization manifest 和全部 materialized batches
  重建原事件、Visibility、数据主体与 projection targets；真库回归在 parent hash
  已变化后重试，仍不新增正史并可补齐投影。
- Campaign conclusion 不再强制至少一条成长记录；合法的无奖励 Ending 可通过空
  settlement 从 `AwaitingGrowth` 进入 `Completed`，完成后的任何再次 settlement
  仍由状态转换门禁拒绝。
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
随即取消，未计为通过。第十一轮两项以双 shape replay 和 Event Store 插入线性化
门禁修复；对应提交 `453b063` 的 repository-truth、golden-scenarios、
production-security 为 3/5 通过。第十二轮精确 SHA review `4784953431` 确认第十一轮
问题未重复，但继续指出 2 个有效问题：Combat 按动作种类硬编码伤害公式而拒绝
`1d6+1` 等武器公式，以及 fork child scenario 丢失 ending `growth_awards`。
workspace/release 随即主动取消，未计为通过。第十二轮修复将选定武器/公式纳入正式
Combat 状态并按实际伤害方三层重算，同时把实现结局的 awards 纳入 snapshot/hash/
child scenario；真实 child 已成功结算未消费的 Psychology 奖励。修复从全新
PostgreSQL/Witness 连续通过两次，34 目标 Semgrep 为 0 finding。诊断性的完整 runtime
crate 命令因未提供既有 P02 专用 `P02_WORKFLOW_DATABASE_URL` 失败，未计为通过；
对应提交 `c11f82c` 的 repository-truth、golden-scenarios、production-security
为 3/5 通过；第十三轮精确 SHA review `4785113440` 确认第十二轮问题未重复，但继续
指出 canonical fork retry 依赖可变 parent snapshot，以及无奖励 Ending 无法完成
两项问题。workspace/release 随即主动取消，未计为 5/5。第十三轮修复改为从 child
canonical fork 事件重建 exact retry，并允许空成长 settlement；真实数据库明确证明
parent snapshot hash 改变后的重试成功且 Event Store 不增，全仓 check/Clippy 与
34 目标 Semgrep 复扫通过。对应提交 `99e3374` 的 repository-truth、
golden-scenarios、production-security 为 3/5 通过；第十四轮精确 SHA review
`4785291315` 确认第十三轮问题未重复，但指出新遭遇构造器会把持久化伤势重置为满血
共 1 项 P1。workspace/release 随即主动取消，未计为 5/5。第十四轮修复引入显式
`CombatHealth`、跨遭遇快照访问器和规则/独立领域双重初始态验证；MajorWound 与
Dying 连续性、矛盾健康状态负例、真实 PostgreSQL/Witness、全仓 check/Clippy 及
34 目标 Semgrep 均通过。对应提交 `7466745` 的 repository-truth、
golden-scenarios、production-security 为 3/5 通过；第十五轮精确 SHA review
`4785498371` 确认第十四轮问题未重复，但指出 Growth Visibility 可扩大、成长骰可由
独立骰拼装挑选、Dying 无法由 First Aid 稳定共 3 项 P1。workspace/release 随即
主动取消，未计为 5/5。第十五轮修复要求 Growth 精确保持来源 Character/Sheet
Visibility，删除原始成长骰组合入口并一次采样完整尝试，同时让规则与独立 replay
把成功 First Aid 派生为 1 HP `MajorWound`。真实 widening 攻击、原子骰跨玩法复用、
Medicine/伪造 Able 负例、primary/Witness、全仓 check/Clippy 与 34 目标 Semgrep
均通过。第十六至二十轮继续修复 Growth 后续 SAN/child Growth 重建连续性、Ending
时间范围、Growth sheet identity、生产 API role rebuild 权限、late-joining
character 与 padded scenario ID，以及 fork 复制角色后续 SAN 保持；后一项已由
第二十一轮确认。第二十轮另两个默认公开 fork 之外的扩展 P2 按用户门槛明确延期。
第二十一轮指出的 Fork/Reconsideration NULL CHECK 绕过与 dead-target 随机有效性
已由 forward-only `20260727000900`、4 个 schema 行为探针、规则层及独立 replay
共同修复；提交 `b611eab` 的 Hosted CI 5/5，第二十二轮精确 SHA review
`4786508911` 确认三项未重复。该轮新指出的 canonical append 后骰 ownership 释放
窗口与重复成长奖励，已由 forward-only `20260728000100` 的 canonical 事务内
reservation、故障后精确重试和 Scenario per-ending 去重完成本地修复。真实数据库、
迁移、规则、workspace check/Clippy 与锁定工具链门禁已通过；本轮 Semgrep 因社区
规则外联被安全审查拒绝且无本地缓存，明确记为未运行。提交 `bfdc6f4` 的第二十三轮
精确 SHA review `4789295455` 确认上述两项未重复，并指出请求的 padded Ending ID
只用于 trimmed 场景匹配、却以原值写入正史/投影，以及 Scenario encounter participant
可带空格/标点而运行时聚合拒绝。修复提交 `b165094` 在 `record_ending` 入口只规范化
一次并让 event/projection 共享该值，同时让场景校验复用运行时 ID 语法；其 Hosted CI
在第二十四轮意见到达时已明确 3/5 通过、2 项仍运行。精确 SHA review `4789452227`
确认第二十三轮两项未重复，并指出来源已消费成长在 child 可重复领取、任意独立
Campaign 可冒充 fork child、未结束 Combat/Chase 会被复制进不可继续的 ENDED Session。
当前修复把按角色/技能的成长消费标记纳入快照并改写为 child ID，严格验证 child
Authority Contract 为 `fork_for_child` 派生结果，同时在预览和 materialization
双重拒绝未终止玩法状态。规则全量、data-eventing lib `26/26`、真实 core-domain
`1/1`、Tutorial `2/2`、workspace check 与严格 Clippy 已通过。修复提交 `7acc802`
在第二十五轮意见到达时 Hosted CI 已明确 3/5 通过、2 项仍运行；精确 SHA review
`4789695257` 确认第二十四轮三项未重复，并指出旧 parent-owned fork 在同一 parent
拥有多个 child 时会使 v2 child 唯一索引无法创建，以及超过 128 字节的成长技能可被
场景接受却永远无法持久化。当前修复用 HMAC-bound materialization target 判别新
child-owned v2 lineage，使旧多分支历史安全升级；场景入口同步持久层的 128 字节限制。
legacy 双分支/B24/empty/repeat/drift 的真实迁移 `1/1`、场景 `5/5`、真实
core-domain `1/1` 与 Tutorial `2/2` 已通过；新提交、Hosted CI 与精确 SHA 复审仍须
在合并前通过。
P08 到此停止，未执行 P09。
