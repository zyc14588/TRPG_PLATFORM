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
GITHUB_SEVENTH_AUTOMATED_REVIEW = 5_ACTIONABLE_FIXED_LOCALLY
GITHUB_LATEST_AUTOMATED_REVIEW = RERUN_PENDING
THIRD_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
FOURTH_REPAIR_HOSTED_CI = PASS_2_OF_5_3_CANCELED_AFTER_REVIEW_BLOCKERS
FIFTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
SIXTH_REPAIR_HOSTED_CI = PASS_3_OF_5_2_CANCELED_AFTER_REVIEW_BLOCKERS
REPAIR_HOSTED_CI = PENDING
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
| Chase 终态 | `Escaped`/`Caught` 后普通推进失败；新追逐必须使用新 ID；每名参与者结果由 opaque 服务端 percentile evidence 和 MOV 派生，调用方不能提交成功布尔值；持久化 roll ledger 拒绝跨 segment 复用骰 ID | PASS |
| 复议追加链 | Request → Review → Upheld/Corrected 均为正式事件；请求者必须能查看源事件，源事件与整条复议链的 Visibility/subject/data subject 完全一致；review/resolution 在事件创建前统一 trim，live projection 与删除后 replay 一致；精确重试幂等，原事件不删除 | PASS |
| Fork 范围与 Hash | 来源快照 hash 被重新计算并精确匹配请求；角色状态由截止序列前的 verified canonical events 重建；单事件只保存有界的内容寻址引用，实际数据按大小受限的正式事件批次物化；私密 scope 以及 `keeper_only` 角色/角色卡均被排除 | PASS |
| Fork 实体化与重放 | 子 Campaign 实际创建 scenario、character/sheet、ended session、scenes、public events、clues、NPC、combat、chase、conclusion 和 manifest；删除投影后可从子 Campaign 正史逐字节重建 | PASS |
| Fork child lineage 唯一性 | `record_campaign_fork` 在任何空状态检查和 materialization 前获取 child-scoped transaction advisory lock，并持有到 canonical commit 与 projection 完成；锁内以 verified Event Store 识别既有 lineage，数据库另有 `UNIQUE(child_campaign_id)`；真实 `tokio::join!` 竞争只产生一个成功、一个正史 lineage 和一个投影 lineage | PASS |
| Fork cutoff 隔离 | `source_cutoff_event_sequence` 只由来源 Session 的玩法状态决定；cutoff 后完成的相关公开复议链单独加入 Public events，不会把其间较新 Session、Ending 或 Growth 纳入旧 Session 快照 | PASS |
| 可见性保持 | Fork materialization 按 keeper、party 和 owner-bound private 行分批；每个事件自己的 Visibility、`data_subject_id` 与主体密钥进入 request hash、HMAC、Event Store 和 Outbox，投影触发器继续要求事件/行完全一致 | PASS |
| 幂等与语义唯一性 | Combat、Chase、Ending、Growth 的 exact retry 返回原 persisted commit；Ending 的 Session 键与 Growth 的 Character 键在事务 advisory lock 下串行检查、append 和 projection；真实并发竞争各只产生一条正史 | PASS |
| 活跃会话边界 | Combat/Chase 在同一事务内对 Session 行持有 `FOR SHARE` 锁并要求状态精确为 `ACTIVE`；Session 终止路径的 `FOR UPDATE` 锁封闭状态检查与正式 append 间的 TOCTOU；结束态负例不增加 Event Store | PASS |
| 场景参与者唯一性 | Scenario 验证在接受 Combat/Chase encounter 前拒绝重复 participant ID，保证通过验证的 encounter 可构造正式聚合 | PASS |
| 结局与成长 | 活跃会话不能结局；`ending_id` 必须存在于会话绑定场景的 `endings`；Ending summary 在事件创建前规范化并与 replay 投影一致；成长技能还必须存在于该 Ending 的 `growth_awards`；结果从共享内核不可构造的 OS CSPRNG 证据计算，并生成新锁定角色卡版本 | PASS |
| Tutorial 完整闭环 | 真实 PostgreSQL 上完成角色、场景、调查、服务端骰、线索、SAN、战斗、追逐、结局、成长、复议和 Fork | PASS |
| Schema/最小权限 | 四个 forward migration、projection guards、可延迟外键、成长算术/证据约束、完整 Fork scope 表、child lineage 唯一约束及角色权限断言 | PASS |
| 第三方检查 | Semgrep 1.171.0 本机复扫 33 个 P08 Rust/SQL/CI 目标，13 条适用规则，0 finding、0 error、0 skipped；PR #9 七轮远端自动审查先后提出 4、5、5、4、2、3、5 项真实问题，均已修复或完成本地验证，最新提交等待复审 | PASS_WITH_REMOTE_RERUN_PENDING |

## 反伪造修复

- 原先只验证一行 snapshot 的 Fork 已替换为子 Campaign 所有的正式事件批次、实际实体和可重放 manifest。
- 原先塞入单个事件的完整 snapshot 已替换为内容寻址引用；实际物化事件同时限制每批
  行数与序列化字节数，防止超过 Event Store 的 1 MiB 事件上限。
- Fork 角色快照不再按当前 projection 的 `last_event_sequence` 过滤；它从经过完整
  HMAC/Witness 校验的 Event Store 回放到 source cutoff，因此角色在后续 Session
  发生 SAN/Growth 后不会从旧快照消失，也不会把新状态倒灌进旧分支。
- 相关复议的 resolution sequence 不再用 `GREATEST` 抬高全局 source cutoff；base
  snapshot 始终停在来源 Session 的玩法边界，cutoff 后完成且原事件已在 base 中的
  公开复议链从 verified Event Store 的 request payload 按 ID 单独加入。真实负例
  证明复议前发生的较新 Session/Ending/Growth 不会因此泄漏进旧分支。
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
- Combat 与 Chase 状态持久化全部已消费 roll ID；新攻击、治疗尝试或 chase segment
  在应用前同时检查本次内部重复和历史账本重复。服务端骰对象即使被 clone，也不能
  在同一正式聚合的后续版本再次产生结果。
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
- Fork 的 child 空状态检查不再是无锁快照；child-scoped transaction advisory lock
  覆盖 canonical lineage 检查、materialization、Event Store commit 和 projection，
  verified Event Store 与数据库 `UNIQUE(child_campaign_id)` 共同保证每个 child
  只有一个 lineage。真实并发竞争证明失败方不会追加第二条正史。
- Ending summary、Reconsideration review summary 与 resolution 在创建 canonical
  event 前只规范化一次；live projection 与 replay 使用相同值，带首尾空白的真实
  数据库用例在删除投影后仍逐字节一致。
- 复议请求不再只检查 Campaign membership 和 source sequence 存在；SQL 授权同时
  验证源事件 Visibility、subject、data subject 与请求者，并要求新事件 envelope
  精确继承。review/resolve 继续与上一条链事件三项一致，猜测 keeper/private sequence
  返回统一 NotFound 且不会追加事件。
- Tutorial 不使用手写事件字符串数组冒充 E2E；它连接独立 primary/Witness 数据库并检查 Event Store、Outbox、formal commits、HMAC 和 Witness。
- 投影重放前后比较实际 JSON，且断言 Event Store 行数不变。

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
5/5。五项现已完成本地根因修复，并通过真实 PostgreSQL/Witness、工作区
编译/Clippy/回归与 33 目标 Semgrep 复扫；新修复提交的 Hosted CI 与精确 SHA
远端自动复审仍须在合并前通过。P08 到此停止，未执行 P09。
