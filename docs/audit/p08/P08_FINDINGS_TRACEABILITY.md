# P08 Findings Traceability

记录日期：2026-07-27（Australia/Brisbane）
基线：`18825746082886a63aee10891860aedb749349e1`

| Finding | 根因 | 修复代码 | 负向/正向证据 | 状态 |
| --- | --- | --- | --- | --- |
| AUD-031 | 旧伤害函数仅按本次伤害计算 condition，可清除已有 MajorWound | `combat_state_machine.rs` 的 `CombatState`、`apply_damage_with_armor`、显式恢复；领域层 `canonical_gameplay_state.rs` 再验证完整前驱转换 | `combat_condition_sequence` 验证大伤后小伤/护甲吸收仍为 MajorWound；真实 DB 在持续伤害后仍为 MajorWound，只有绑定治疗者 First Aid 的成功正式事件将其恢复为 `ABLE` | CLOSED_PASS |
| AUD-032 | 旧追逐函数不绑定当前状态，终态可重新进入 Ongoing | `chase_state_machine.rs` 的 `ChaseState`；领域层对 ID、参与者、距离、segment、version 和 exact predecessor 做二次校验 | `chase_terminal` 验证 Escaped/Caught 均拒绝推进，新追逐使用新 ID；真实 DB 保存 `CAUGHT` v2 | CLOSED_PASS |
| AUD-036 | Fork 忽略关键字段，只写 parent snapshot 行，没有子 Campaign 实体；当前 projection 过滤会遗漏 cutoff 后又更新的角色；materialization 还会把 party/private 子行全部降为 keeper-only | `fork_canon_lineage.rs`；`preview_campaign_fork`、`record_campaign_fork`、source-session-scoped canonical replay、按 Visibility/数据主体/字节上限分批的 materialization/replay；内容寻址 snapshot；逐事件 request-hash/HMAC/Event Store/Outbox 主体绑定；P08 五个 migration | domain tests 拒绝 self-fork、坏/不匹配 hash 和未授权私密 scope；真实 DB 证明 `keeper_only` sentinel 被排除，后续或交错 Session 不会遗漏来源角色或污染旧快照，全部声明 scope 被实际物化，私密事件使用 owner 主体密钥，删除后重放一致且 source 不变 | CLOSED_PASS |
| AUD-043 | 只有权限枚举，缺少完整复议实体和追加式处理 | `ReconsiderationOutcome`、request/review/resolve 状态机、正式事件与 v2 projection | 精确重复 request 幂等；Upheld 与 Corrected 均为独立事件；完成后追加失败；原始 event sequence 保留 | CLOSED_PASS |

## P08 其他完成项

| 项目 | 证据 | 状态 |
| --- | --- | --- |
| 服务端正式骰 | `ServerPercentileRoll`、`ServerD10Roll`、`ServerDamageRoll` 和 `ServerGrowthRollEvidence` 字段私有且不可反序列化；Combat/Chase/Growth 正式提交还必须携带与 state JSON 相同的 opaque evidence | PASS |
| Combat/Chase 防 JSON 伪造 | Combat API 不再接受原始伤害，Chase API 不再接受成功布尔值；规则 replay 与独立领域 replay 重算骰值、成功等级、固定伤害公式和唯一合法下一状态，持久层再绑定同一批 server evidence；错配证据与异源同 ID 均在 append 前失败 | PASS |
| Combat 技能绑定 | DEX 只决定先攻；每个正式参与者持久化 Melee、Firearm、Dodge 目标，攻击、防御、规则 replay 与领域 replay 都按对应技能重新验证；高 DEX/低 Firearm 负例按 Firearm 失败，记录无伤害 miss 且不改变 HP | PASS |
| Combat miss 正史 | 攻击失败或 Dodge 成功生成 `ATTACK_MISSED` mutation，保留攻击/防御服务端骰并推进 version，不保存或接受伤害骰；规则前驱 replay 与独立领域 replay 重算结果，成功命中不能伪装成 miss | PASS |
| Combat 回合动作消费 | attack/miss/医疗尝试均设置 `turn_action_consumed`，只有 `TURN_ADVANCED` 重置；同回合第二次攻击在规则层和独立 serialized replay 层拒绝 | PASS |
| Fight Back | `FightBack` 与 Dodge 分离；平手由发起攻击者获胜，防守方只有更高成功等级才把伤害施加给攻击者；派生 outcome 进入 mutation，篡改 outcome 在 append 前失败 | PASS |
| 失能攻击者 | Fight Back 后若当前攻击者为 `DYING/DEAD`，公开聚合、规则前驱 replay 与独立领域 serialized replay 都拒绝其再次攻击，必须先推进至可行动参与者 | PASS |
| 失能目标主动防御 | `DYING/DEAD` 目标只能承受无主动防御的攻击，不能 Dodge 或 Fight Back；规则与独立 replay 负例均拒绝 | PASS |
| 医疗目标绑定 | 调用方不再提交数值 target；指定当前治疗者与 First Aid/Medicine 后，从其持久化技能派生 target。失败尝试也作为正式 mutation 消费动作和服务端骰；真实 DB 成功恢复路径由三层重算 | PASS |
| 正式骰单次消费 | Combat/Chase 状态保留 aggregate-local ledger；持久层另以 `gameplay_roll_consumptions.roll_id` 全局主键、排序 advisory lock 和 canonical projection guard 阻止本次内部、后续 version、不同 aggregate、跨 Combat/Chase/Growth 及跨 Campaign 复用；真实 DB 分别把 Combat 医疗骰 clone 给 Chase 与 Growth，均在 append 前得到 `gameplay_roll_reuse` | PASS |
| 复议源事件可见性 | `request_reconsideration` 同时校验 source canonical integrity、请求者对 source Visibility/subject/data subject 的访问权及 envelope 精确继承；review/resolve 与上一链事件保持三项一致；猜测 keeper-only sequence 返回 NotFound 且不写事件 | PASS |
| Active Session 写入边界 | Combat/Chase 写入事务使用 `FOR SHARE` 锁定 Session 并要求精确 `ACTIVE`；Session 状态变更使用 `FOR UPDATE`，关闭状态检查与正式 append 间的竞态窗口；结束态负例不增加事件 | PASS |
| Encounter participant 唯一性 | Scenario parser 在接受 combat/chase encounter 前拒绝重复 participant ID；负例不再延迟到正式聚合构造阶段 | PASS |
| Fork 公开范围 | 默认 scope 明确包含 Character/Public events/Clues/World/NPC/Scene/Combat/Chase/Conclusion；Keeper notes/Hidden clues/Private messages/AI memory 明确排除；角色与当前 sheet 均只接受公开/队伍可见或 owner-bound 玩家私有标签 | PASS |
| Fork hash 与事件大小 | `source_snapshot_hash` 与经重新计算的来源快照一致；记录事件只保存内容寻址引用；materialization manifest 使用确定性 root，批次同时限制行数与序列化字节数，超过 1.2 MiB 的测试快照仍不会产生超限事件 | PASS |
| Fork 完整 scope | Public events、Clues、NPC、Combat、Chase、Conclusion 与 scenario/session/scene/character/sheet 均有正式 materialization 事件和受保护子投影；Combat/Chase 的 participant、initiative、transition 与 roll 引用均改写为 child-owned character/NPC ID | PASS |
| Fork child lineage 与连接池 | Event Store partial unique index 保证每个 child Campaign 只有一个 canonical `CampaignForkRecorded`，projection 保留 `UNIQUE(child_campaign_id)`；snapshot/build/canonical commit/replay-page load 不持有 projection connection，最终短事务才获取 child/rebuild lock；并发两个 fork ID 只产生一个 lineage，单连接 pool 也能完成 fork | PASS |
| Ending/Growth | 未结束会话负例；当前 sheet 决定 `skill_before`；技能必须存在于所选 Ending 的 `growth_awards`；percentile/d10 presence 与规则结果在应用、事件 replay 和 DB constraint 三层校验 | PASS |
| P08 projection replay | campaign-scoped rebuild lock 与所有 P08 writer 共用同一键；事务内清除 Combat/Chase/Reconsideration/Ending/Growth/全局骰消费/Fork 专属读模型后从 verified canonical events 重建；fork 基础实体只按 immutable materialization target ID 替换，保留后续 Scenario/Character/Session/Scene；Growth 角色回退仍需 secret capability、精确 canonical target 与仅 Growth 后缀；同版本污染和 ghost 均被替换，重建前后 JSON 相等且不写 Event Store | PASS |
| Exact retry | Combat、Chase、Ending、Growth 对同一 commit/command/idempotency/request 返回相同 receipt 且不重复投影；Fork retry 只核对该 fork 的 verified projection target，不把 child 后续实体计入 manifest；不同绑定保持 fail closed | PASS |
| Ending/Growth 语义唯一性 | Session/Character 事务 advisory lock 覆盖语义检查、canonical append 与 projection；同一 Session 的并发 Ending 和同一来源 sheet 的并发 Growth 各只允许一个成功，失败方不增加 Event Store | PASS |
| Ending 场景绑定 | `record_ending` 读取 ended Session 所绑定 Scenario 的 canonical `document_json.endings`，未声明 ID 在正式提交前拒绝 | PASS |
| Canonical 文本规范化 | Ending summary、Reconsideration review summary 与 resolution 在 event 构造前 trim；幂等比较、live projection 与 replay 使用同一规范值；带首尾空白的真实 DB 输入和投影重建一致 | PASS |
| Fork 历史 cutoff | verified `SessionStarted` 确定来源起点；base event set 由起点前 campaign baseline 与来源 Session ID、Scene/Action 归属事件构成；第二 Session start/end 即使插在第一 Session Ending/Growth 之前也不进入快照；相关公开复议链再按 reconsideration ID 单独加入，不抬高角色/Clue/全局 public-event 范围 | PASS |
| Fork Visibility 与主体密钥 | Scenario 为 `keeper_only`、Session/Scenes 为 `party_visible`、Character/Sheet 为 owner-bound `private_to_player`；每个私密 `CampaignForkMaterialized` 事件保存玩家 `data_subject_id` 并使用对应有效主体密钥，envelope、Event Store、Outbox 与投影一致 | PASS |
| Tutorial 完整流程 | `tutorial_complete_e2e` 两个用例，真实 DB/Witness 主流程及提前 Ending/私密 Fork 负例 | PASS |

## PR 自动审查修复追踪

| 轮次 | 远端发现 | 修复与验证 | 状态 |
| --- | --- | --- | --- |
| 第一轮，4 项 | historical cutoff、子实体 Visibility、exact retry、Ending scenario 绑定 | verified replay、逐事件 Visibility、persisted receipt retry、append 前场景绑定校验；真实 PostgreSQL/Witness 回归 | FIXED |
| 第二轮，5 项 | Ending/Growth 语义重复可能先写正史；私密事件主体/密钥错误；声明 scope 未全部物化；单事件嵌入无界 snapshot | append 前语义键检查；逐事件主体加密；六类遗漏 scope 的正式物化/投影/重放；内容寻址 snapshot 与双重批次上限 | FIXED |
| 第三轮，5 项 | Combat 可提交任意伤害、Chase 可提交成功布尔值、Growth 未绑定所选 Ending 奖励、并发 Ending/Growth 仍可能先后追加孤儿正史 | opaque 攻击/防御/伤害/参与者骰证据及三层重算；`growth_awards` 精确绑定；Session/Character 事务 advisory lock；错配与真实并发负例 | FIXED_CONFIRMED_BY_FOURTH_REVIEW |
| 第四轮，4 项 | Combat 错用 DEX 而非战斗技能；缺少 Fight Back；非 ACTIVE Session 仍可写玩法正史；Scenario 接受重复 participant | 持久化 Melee/Firearm/Dodge 并按动作重算；Fight Back tie/counterattack/outcome；事务 Session 行锁与 ACTIVE 检查；入口去重；真实 DB、单元、Clippy 与 Semgrep 回归 | FIXED_CONFIRMED_BY_FIFTH_REVIEW |
| 第五轮，2 项 | 相关复议用 resolution sequence 扩大全局 fork cutoff，可能带入较新 Session；Fight Back 令当前攻击者失能后仍可继续攻击 | gameplay base cutoff 与补充复议链分离；按原事件可见性与 reconsideration ID 选择链；三层 `can_act` 检查；晚期复议泄漏和失能重复攻击负例 | FIXED_CONFIRMED_BY_SIXTH_REVIEW |
| 第六轮，3 项 | miss/成功 Dodge 被当作错误而丢失骰证据；同一空 child 的不同 fork ID 可并发通过检查；Ending/Reconsideration 文本在 canonical event 与 live projection 间不一致 | `ATTACK_MISSED` 无伤害正式转换与三层重放；child-scoped 锁、verified canonical lineage 检查及 DB unique；event 构造前单次规范化；单元、真实并发双库、删除后重放、Clippy 与 33 目标 Semgrep | FIXED_CONFIRMED_BY_SEVENTH_REVIEW |
| 第七轮，5 项 | Campaign member 可猜测不可见 source sequence 发起复议；攻击不消费回合；失能目标仍可主动防御；医疗 target 由调用方自报；服务端骰可跨版本复用 | source event 级 Visibility/subject/data-subject 授权与全链精确继承；持久化动作消费标记；防御者 `can_act`；治疗者 First Aid/Medicine 派生及失败留痕；Combat/Chase 已消费 roll ledger；规则、独立领域、真实双库与 33 目标 Semgrep | FIXED_CONFIRMED_BY_EIGHTH_REVIEW |
| 第八轮，3 项 | source cutoff 会纳入同一 campaign 中交错写入的其他 Session；rebuild 跳过同版本损坏行且保留 ghost；aggregate-local roll ledger 可被跨 aggregate/Combat/Chase 绕过 | verified source Session/Scene/Action event set；锁内删除并重放 Combat/Chase/全局消费投影；全局 roll 主键、排序锁和 projection guard；交错 Session、同版本污染/ghost、Combat 医疗骰复用于 Chase 的真实双库负例；34 目标 Semgrep | FIXED_CONFIRMED_BY_NINTH_REVIEW |
| 第九轮，4 项 | rebuild 保留 Reconsideration/Fork/Ending/Growth 等损坏或 ghost；Growth roll 可与 Combat/Chase 跨类型复用；forked Combat/Chase 保留 parent participant ID；长期持有 projection connection 会在 20 个并发 fork 时耗尽池 | 清除并重放全部 P08/Fork 物化投影，Growth 以受限 capability 安全回退角色；Growth 加入全局 roll ownership；递归 child ID 重写及合成 NPC 投影；canonical uniqueness + 短投影事务，单连接 pool 回归；连续两次真实双库、all-features check/Clippy 与 34 目标 Semgrep | FIXED_CONFIRMED_BY_TENTH_REVIEW |
| 第十轮，2 项 | fork child 正常继续后，P08 rebuild 会删除后续 Scenario/Character/Session/Scene；exact fork retry 把整个 child Campaign 行数误当成 immutable manifest 行数 | 从 canonical materialization payload 提取 fork-owned 基础 ID 并仅替换这些行；retry 从 verified Event Store projection targets 精确计算 fork-owned 行；真实双库创建后续 Scenario、Character/Sheet、Session/Scene，再执行 exact retry 与 rebuild，逐字节保持后续投影且 Event Store 计数不变 | FIXED_CONFIRMED_BY_ELEVENTH_REVIEW |
| 第十一轮，2 项 | source-session event selector 只读取 `payload.data`，漏掉顶层字段的 `PlayerActionSubmitted` 及依赖 SAN；child emptiness preflight 未与普通 canonical write 串行，仍可先后写入两套初始化历史 | replay 字段同时接受顶层与 `data` shape，Tutorial 断言 snapshot/child SAN；Event Store 全 campaign INSERT trigger 共用事务 advisory lock，Fork 插入时重查 verified/formal 非基线历史；trigger/function 完整 catalog 指纹；确定性屏障让普通写先排队、Fork 后排队，证明只能普通写成功；两次全新 primary/Witness、all-features check/Clippy 与 34 目标 Semgrep | FIXED_LOCALLY_RERUN_PENDING |

所有正式写入保持 Authority、Visibility、Fact Provenance、formal commit、Event Store、
Outbox 和 projection guard 边界。没有删除或覆盖源事件，没有让 projection 成为正史，
没有执行 P09。
