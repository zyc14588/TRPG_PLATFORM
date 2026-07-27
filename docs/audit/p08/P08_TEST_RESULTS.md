# P08 测试、失败记录与验收证据

记录日期：2026-07-27（Australia/Brisbane）
基线 HEAD：`18825746082886a63aee10891860aedb749349e1`

## 修改前基线

P08 实现前，三条强制命令均真实返回 Cargo exit `101`，原因是不存在
`combat_condition_sequence`、`chase_terminal` 和 `tutorial_complete_e2e` 测试目标。
既有 Combat/Chase/Character/Scenario 与 P07 slice 当时通过，因此该缺口未被写成成功。

## P08 强制命令

| 命令 | 最终结果 |
| --- | --- |
| `cargo test -p trpg-ruleset-coc7 --test combat_condition_sequence` | PASS，`5/5`，exit `0` |
| `cargo test -p trpg-ruleset-coc7 --test chase_terminal` | PASS，`3/3`，exit `0` |
| `cargo test -p trpg-testing --test tutorial_complete_e2e` | PASS，`2/2`，exit `0`；使用真实 PostgreSQL/Witness 环境 |

负向覆盖包括 MajorWound 非法恢复、终态 Chase 继续推进、失败转移不变更聚合、
同 ID 异源 Combat、self-fork、坏/不匹配 snapshot hash、私密 Fork scope、
默认 Fork 排除 `keeper_only` 角色及其当前角色卡、
复议终结后再追加、空成长技能/重复成长结果和未结束会话提前结局。
新增负向/恢复覆盖还包括场景未声明 `ending_id`、Combat/Chase/Ending/Growth exact
retry、来源 cutoff 后第二 Session 的 Growth，以及 Fork 子实体逐行 Visibility。
第二轮修复负例还覆盖同一 Session 的不同 Ending、同一
ending/character/skill 的不同 Growth、私密 fork 事件主体密钥错配，以及超过
1.2 MiB 的来源快照；这些失败均不会留下越权或语义重复的正史事件。
第三轮修复负例进一步覆盖 Combat 伪造/错配攻击与伤害骰证据、Chase
伪造/错配参与者骰证据、结局未授予的成长技能，以及两个真实并发 Ending
和两个共享来源角色卡的并发 Growth；失败方均在正式 append 前终止，正史只增加一条。
第四轮修复负例覆盖高 DEX/低 Firearm 不得借 DEX 命中、Fight Back 平手规则与
防守方更高成功等级反击、serialized outcome 篡改、结束态 Session 启动 Combat/Chase，
以及 Scenario encounter 的重复 participant；所有被拒绝的正式写入均不改变聚合或
Event Store。
第五轮修复负例覆盖：第二 Session/Ending/Growth 已发生后才完成对旧公开事件的复议，
旧 Session fork 仍单独包含完整复议链但不包含较新 Session/Ending 或第二轮角色卡；
Fight Back 令当前攻击者失能后，规则聚合与独立 serialized replay 都拒绝其再次攻击。
第六轮修复负例覆盖：攻击失败作为 `ATTACK_MISSED` 正式推进但 HP/condition 不变、
miss 不得携带伤害骰、成功命中不得伪装成 miss；同一空 child 上两个不同 fork ID 的
真实并发竞争只能产生一个 canonical/projection lineage；带首尾空白的 Ending summary、
Reconsideration review/resolution 在 canonical event、live projection 与删除后 replay
中保持同一规范值。
第七轮修复负例覆盖：Campaign member 猜测 `keeper_only` 源事件 sequence 发起复议
返回统一 NotFound 且不写事件，后续 review 也不能扩大链条可见范围；攻击命中或失败后
同一 actor 在推进回合前不能再次攻击；`DYING/DEAD` 目标不能 Dodge/Fight Back；
First Aid/Medicine 目标从当前治疗者的持久化技能派生而不能由调用方抬高，失败治疗仍
形成正式 mutation；Combat/Chase 的服务端 roll ID 在后续 aggregate version 中不能复用。
第八轮修复负例覆盖：第二 Session 的 start/end 交错写在第一 Session 的 Ending/Growth
之前，第一 Session fork 仍不含第二 Session；Combat 医疗骰对象被 clone 后用于 Chase
会在 canonical append 前返回 `gameplay_roll_reuse`；保持相同 version 的 Combat/Chase
`state_json` 与 provenance 污染会被 rebuild 覆盖，无 canonical history 的 ghost 行
会被删除，Event Store 行数保持不变。
第九轮修复负例继续覆盖：同一 Combat 医疗骰被 clone 给 Growth 时也在 canonical
append 前返回 `gameplay_roll_reuse`；forked Combat/Chase JSON 不含 parent character
或 NPC ID，participant/initiative/transition/roll 引用均为 child-owned ID；只有一个
projection connection 的仓库仍能完成 fork；Reconsideration、Ending、Growth、
Growth sheet/Character、Fork manifest/NPC/scenario/character/sheet/session/scene
的同版本污染和 ghost 均被全量重建清除。

## 真实数据库、重放与迁移

临时环境使用固定 digest 的 PostgreSQL/pgvector 镜像、localhost 端口和每次生成的
一次性随机数据库口令；测试结束自动删除容器。未使用或输出用户提供的系统密码。

| 门禁 | 结果 |
| --- | --- |
| `migration_upgrade` | PASS，`1/1`；empty/upgrade/repeat/drift/constraints |
| `decision_state_outbox_atomicity` | PASS，`1/1`；P07 SAN/角色版本/Event Store/Outbox 回归 |
| `core_domain_schema_integration`，primary + independent Witness | PASS，`1/1` |
| `tutorial_complete_e2e`，primary + independent Witness | PASS，`2/2` |
| 容器内 `scripts/ci/assert-schema.sql` | PASS；P06、P07、P08 schema assertions |

真实集成验证：

- Combat v1→v14 后为 `ENDED`；失败攻击以 `ATTACK_MISSED` 保存服务端攻击骰且消费
  当前动作，不生成伤害；正式回合推进、MajorWound、普通伤害、Fight Back 与玩家
  First Aid 自救均经过独立重放，只有成功医疗事件把 condition 恢复为 `ABLE`。
  伪造 outcome、伪造 miss、同 ID 异源状态、同回合第二次攻击、失能目标主动防御、
  自报医疗目标和跨版本复用 roll ID 均未进入 Event Store。
- Chase v1→v2 后为 `CAUGHT`，终态不能再推进；Combat 已消费的医疗 roll ID 被 clone
  后也不能用于 Chase。全局消费表保留首次 `COMBAT` 归属，失败尝试不追加 Chase 事件。
- Reconsideration 的 Request/Review/Upheld/Corrected 全部追加，原事件保留；不可见
  源事件不能被 Campaign member 通过猜测 sequence 引用，review/resolve 也不能扩大
  source/前驱事件的 Visibility、subject 或 data subject。
- Fork 来源 hash 精确匹配，记录事件保存有界内容寻址引用，物化批次受行数和字节数
  双重限制；私密 scope 不存在，`keeper_only` 角色与 sheet sentinel 不进入快照。
- Public events、Clues、NPC、Combat、Chase、Conclusion 与既有子实体均实际落入
  child projection；删除后从正史重建为相同 JSON，Event Store 行数不变。
- Fork 角色由 source cutoff 前的 verified events 重建；第二 Session 更新当前角色卡后，
  旧 Session fork 仍保留 cutoff sheet，且 child character/sheet 仍绑定原 owner。
- Fork 的 base 范围不再等同于单一全局 sequence cutoff；verified source Session
  start、Session ID、Scene 与 Action 归属形成事件集合，相关公开复议链按
  reconsideration ID 单独筛选。真实 E2E 把第二 Session start/end 交错放在第一
  Session Ending/Growth 之前，并在稍后写入第二 Session Ending/Growth；最终快照保留
  第一 Session 与其复议链，但不含这些无关状态。
- Fork materialization 分别产生 keeper、party、private 三类事件 envelope；每个私密
  事件使用玩家 `data_subject_id` 和对应有效主体密钥，projection guard 继续验证
  Visibility 与主体完全一致。
- Fork child lineage 由 Event Store `CampaignForkRecorded(campaign_id)` partial
  unique index 与 projection `UNIQUE(child_campaign_id)` 双重约束；snapshot/build、
  canonical commit 和 replay-page load 不持有 projection pool connection，最终投影
  才使用短 child/rebuild lock 事务。两个不同 fork ID 的真实 `tokio::join!` 竞争仅
  一个成功，`max_connections=1` 的专门回归也在 30 秒内完成。
- Ending 只绑定 `ENDED` session，且 ID 必须来自该 Session 的 Scenario `endings`。
- Ending summary 与 Reconsideration review/resolution 在 canonical event 创建前
  规范化；带空白输入的 live projection 和删除后 replay 逐字节一致。
- Growth 从当前 sheet 与 opaque RNG evidence 重新计算；percentile 与可选 d10 ID、
  值和 presence 一致，新 locked sheet 成为 current，旧 sheet 保留。
- Combat 的命中、闪避与伤害，以及 Chase 的每名参与者结果，均由共享内核不可构造的
  OS CSPRNG 证据派生；领域层重算骰值、成功等级、固定伤害公式与状态转换，持久层再将
  serialized state 与同一批 opaque evidence 逐项比对。
- Combat 的攻击/防御 target 分别来自持久化的 Melee、Firearm、Dodge，而 DEX
  只用于 initiative。Fight Back 与 Dodge 使用不同平手规则；防守方反击时伤害目标
  为原攻击者，mutation outcome 不能被 JSON 篡改。
- Combat 的攻击命中、攻击失败与医疗尝试均消费当前回合动作，只有正式
  `TurnAdvanced` 重置；`DYING/DEAD` 防守者不能 Dodge/Fight Back。First Aid/Medicine
  target 从当前治疗者的持久化技能派生，失败治疗同样保存正式证据且不清除 MajorWound。
- Combat 与 Chase 状态持久化 aggregate-local roll ledger；持久层另把 Combat、
  Chase 与 Growth 的每个正式骰写入
  以 `roll_id` 为全局主键的消费投影，并在 append 前持有排序 advisory lock。本次内部、
  后续 version、不同 aggregate 和 Combat/Chase/Growth 间复用都被拒绝。
- P08 rebuild 在 campaign-scoped 锁和单笔事务内清除 Combat、Chase、
  Reconsideration、Ending、Growth、全局骰消费与全部 Fork materialization 后重放
  verified canonical events；Growth 角色回退由 secret capability、精确 canonical
  target、仅 Growth 后缀以及 canonical 派生版本共同限制。真实 DB 对各类投影注入
  同版本污染/ghost 后，重建恢复原 JSON/provenance、删除 ghost，并保持 Event Store
  不变。
- Fight Back 使当前攻击者进入 `DYING/DEAD` 后，再次攻击返回
  `combat_actor_incapacitated` 且聚合不变；独立领域 validator 对手工伪造的同类
  serialized successor 返回 `InvalidTransition`。
- Combat/Chase 写入在同一事务中锁定 Session 行并要求 `ACTIVE`；会话结束后尝试创建
  新 Combat/Chase 均返回 `gameplay_session_state`，而结束前成功写入的 exact retry
  仍返回原 receipt；两种情况都不增加 Event Store。
- Growth 技能必须精确存在于已选 Ending 的 Scenario `growth_awards`；未授予的
  `Dodge` 在正史 append 前被拒绝。
- Combat、Chase、Ending、Growth exact retry 均返回原 receipt，不追加事件或重复投影；
  Ending/Growth 的第二个语义 ID 在 append 前失败且 Event Store 计数不变。
- Ending 按 Campaign/Session、Growth 按 Campaign/Character 使用事务级 advisory
  lock；真实 `tokio::join!` 竞争中各自只有一个成功，Event Store 对应类型只增加一条。
- 全部 Tutorial 正史和 Outbox payload 使用 integrity v3 protected payload；
  formal commits 均 committed，primary HMAC 链和 Witness binding 完整。

## 回归与结构门禁

| 门禁 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace --all-targets --all-features --locked` | PASS |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS |
| `cargo test -p trpg-ruleset-coc7 --all-features --locked` | PASS |
| `cargo test -p trpg-domain-core --all-features --locked` | PASS |
| `cargo test -p trpg-data-eventing --lib --locked` | PASS，`26/26` |
| `cargo test -p trpg-testing --test vertical_human_kp_tutorial_slice --locked` | PASS，`2/2` |
| `cargo test -p trpg-ruleset-coc7 --test growth_resolution` | PASS，`2/2` |
| `cargo test -p trpg-runtime --test conclusion_growth_state_machine` | PASS，`2/2` |
| `python3 scripts/ci/check_dependency_directions.py` 及自测 | PASS；未添加例外 |
| `python3 scripts/ci/check_product_boundaries.py` 及自测 | PASS |
| workflow、test discovery/inventory、evidence schema、Compose security | PASS；发现 235 个 Rust test targets |
| 修改过的 CI shell `bash -n` | PASS |
| `git diff --check` | PASS |

一次诊断性 `cargo test -p trpg-runtime --all-features --locked` 在 P08 专属测试通过后，
运行到既有 `durable_workflow_postgres` 时因缺少其强制
`P02_WORKFLOW_DATABASE_URL` 返回 exit `101`。该命令不是 P08 强制命令，也没有被计为
package regression PASS；P08 对应的 `conclusion_growth_state_machine` 已单独真实通过。
第九轮另一次诊断性 `cargo test --workspace --all-targets --locked` 在普通单元测试
通过后，运行到 API production custody gate 时因本地未启动 CI 专用
`P02_FORMAL_COMMIT_DATABASE_URL` 返回 exit `101`；相关全服务门禁保留给 Hosted
workspace CI，这次本地命令没有被记为 workspace test PASS。与 CI 参数一致的
all-features workspace check 与 Clippy 则独立 exit `0`。

第四轮修复的真实数据库回归也保留两次失败记录：第一次因新加的 Chase 结束态负例
多传一个旧签名参数而编译失败；修正后，在加入正式 Fight Back 事件的下一次运行中，
旧测试仍按 DEX=80 生成 Melee 骰，规则正确地以 Melee=60 返回
`combat_attack_missed`。测试改为按真实技能生成证据后，完整双数据库套件才获得上述
最终通过；两次中间失败均未计作 PASS。

第六轮修复的双数据库回归前两次失败均来自新增证据断言误用 Event Store 列/受保护
payload JSON 路径；产品迁移与前置原子性测试当时已通过，但整套结果没有计为 PASS。
断言改为通过 canonical replay API 读取解密后的正式事件后，完整套件重新从空库运行
并全部通过。Semgrep 第一次受限于网络而停滞，第二次取得规则后因默认并行
`io_uring_queue_init` 资源错误 exit `2`；最终固定 `--jobs 1` 后才取得 0 error 的
正式结果，前两次均未冒充成功。

第七轮修复的真实双数据库套件最终从空 primary/Witness 数据库连续运行两次并全部
通过；第二次包含正式 `TurnAdvanced` 和成功 First Aid 事件，避免用测试端直接改状态
冒充 MajorWound 恢复。规则/领域新增负例分别验证动作消费、失能防御、医疗目标绑定和
跨版本 roll ledger；扩展 Semgrep 仍以相同 33 个目标、13 条规则和 `--jobs 1` 得到
0 finding、0 error。RustSec 则保持下述 exit `1`，没有被静态扫描结果覆盖。

第八轮修复的首个真实双数据库运行在新 source-session 过滤器错误读取 canonical
envelope 顶层字段时失败，修正为读取 `payload.data` 后继续运行；下一次污染测试又因
同一事务尚有 deferred trigger event 而不能恢复 trigger。加入
`SET CONSTRAINTS ALL IMMEDIATE` 后，最终状态从全新 primary/Witness 数据库连续运行
两次并全部通过。两次中间失败均保留为失败，未计作 PASS。Semgrep 把第五个 migration
加入范围，以 34 个目标、13 条规则、`--jobs 1` 得到 0 finding、0 error。

第九轮真实双数据库修复过程保留四次未通过记录：第一次的 child ghost 注入事务在
恢复 trigger 前尚有 deferred event；第二次证明通用 projection guard 正确拒绝
Growth 角色事件序号回退；第三次是新增 schema assertion 对函数格式匹配过严；第四次
是已由测试手工回退的角色再次执行 rewind，不满足幂等条件。分别结算约束、加入受
secret capability/canonical target/仅 Growth 后缀约束的窄 rewind、改为语义断言并
识别 already-rewound 状态后，完整套件才从全新 primary/Witness 连续通过两次。受限
网络中的 Semgrep 首次等待 registry 后被主动终止，联网重试才得到 34 targets、
13 rules、0 finding、0 error、0 skipped；中间状态均未计为 PASS。

## 第三方与依赖检查

| 门禁 | 结果 |
| --- | --- |
| Semgrep 1.171.0，`p/rust` + `p/security-audit` | PASS；34 targets、13 rules、0 finding、0 error、0 skipped |
| CodeRabbit 0.7.0 | CLI 登录浏览器回调未完成，`NOT_RUN_NOT_AUTHENTICATED`，未冒充结果 |
| GitHub PR #9 自动审查 | 前九轮为 4、5、5、4、2、3、5、3、4 项；第八轮修复已由第九轮确认未重复，第九轮 4 项已本地修复，最新提交/复审 pending |
| `cargo audit 0.22.2 --no-fetch` | exit `1`；381 dependencies、3 个基线 advisory |

Semgrep 扩展复扫最初对 `data_deletion_e2e.rs` 报告 2 个共享临时目录竞争问题；测试已
改用锁定版本的 `tempfile::Builder::tempdir()`，没有 suppress 规则。加入第三个 P08
migration 后，第三轮 30 目标复扫
最终为 0 finding。

第二轮远端自动审查真实指出：Ending/Growth 的语义唯一约束可能在正史 append 后才
失败、私密 fork 事件沿用 command-wide 主体与密钥、六类声明 scope 未实际物化，以及
无界完整 snapshot 可能超过单事件大小限制。第三轮又真实指出：Combat/Chase 仍可由
调用方决定正式结果、Growth 未绑定所选 Ending 的奖励，以及并发 Ending/Growth
仍可能各自追加孤儿正史。第四轮继续真实指出：Combat 错用 DEX、Fight Back 缺失、
非 ACTIVE Session 可写玩法正史、Scenario encounter 接受重复 participant。第五轮
又指出相关复议扩大全局 Fork cutoff，以及失能的当前攻击者仍可行动。第六轮继续指出
miss 正史丢失、Fork child lineage 并发竞态，以及 Ending/Reconsideration 文本的
event/projection 不一致。第七轮继续指出不可见复议源事件、回合动作未消费、失能目标
主动防御、调用方自报医疗 target 和服务端骰跨版本复用。第八轮确认这五项未重复，
并继续指出交错 Session 污染 fork、同版本/ghost 投影无法重建，以及骰证据可跨
aggregate/Combat/Chase 复用。第九轮确认这三项未重复，又指出 P08 其余投影仍可能
保留损坏/ghost、Growth 未加入全局骰消费、fork gameplay 未重写参与者 ID，以及
长期持有 projection connection 会耗尽连接池。以上均已按问题根因修复；扩展到 34
目标的 Semgrep
复扫仍为 0 finding。本报告在最新远端 CI/复审完成前保持 pending，不以本地结果冒充
远端通过。
第三轮修复提交仅有 3/5 workflow 完成通过后取消 2 项；第四轮修复提交 `ea760c1`
仅有 2/5 完成通过后取消 3 项；第五轮修复提交 `fb3907e` 仅有 3/5 完成通过后取消
workspace/release 两项；第六轮修复提交 `2ed9df2` 也只有 repository-truth、
golden-scenarios、production-security 3/5 通过，第七轮阻断出现后取消
workspace/release 两项。第七轮修复提交 `56b648b` 同样只有上述 3/5 通过，第八轮
阻断出现后取消 workspace/release 两项。第八轮修复提交 `f1b0e70` 同样只有上述
3/5 通过，第九轮阻断出现后取消 workspace/release 两项。以上均未记为 5/5。

RustSec 报告：

- `RUSTSEC-2026-0194`、`RUSTSEC-2026-0195`：quick-xml `0.38.4`；
- `RUSTSEC-2023-0071`：rsa `0.9.7`。

这两个版本已存在于基线 `Cargo.lock`。P08 的 lockfile 差异只增加 workspace crate
已有版本依赖边，不改变 quick-xml 或 rsa 版本，因此明确保留失败状态。

P08 migration SHA-384：

- `20260727000300`：
  `acda958b3447b5ec2e4038fede4f582567e9e8fbd9c121ae4b4d325dd347a2219241e2ca18f94aed11ca846c673f0b21`
- `20260727000400`：
  `9468f0016859b00550a44290c832fc012702c74ef0837c30dfb28464b96d82e28ef83a055fe194d95c1fd49639e9027e`
- `20260727000500`：
  `b9b54659f4e5189ca17fd38734934baf38fd12367bcf9d0bacf9ff754a4e9e234c6b6cee9bf4e068311565f471e7b644`
- `20260727000600`：
  `4aa250ec0b9020e80194bb87cf86891d06c26cc07f5400fc547ac6860ecc9f4443193e6ef56d4ba2ac438f9863407859`
- `20260727000700`：
  `e2bf932ee689991df816a52fe00dfb669dda5027d193114d04a78f9e169e3c49cdfd8f288416f7b530058b60ea9fa276`
