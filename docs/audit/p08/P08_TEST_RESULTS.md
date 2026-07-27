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
| `cargo test -p trpg-ruleset-coc7 --test combat_condition_sequence` | PASS，`3/3`，exit `0` |
| `cargo test -p trpg-ruleset-coc7 --test chase_terminal` | PASS，`2/2`，exit `0` |
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

- Combat v1→v4 后为 `ENDED`，MajorWound 仍存在；同 ID 异源状态未进入 Event Store。
- Chase v1→v2 后为 `CAUGHT`，终态不能再推进。
- Reconsideration 的 Request/Review/Upheld/Corrected 全部追加，原事件保留。
- Fork 来源 hash 精确匹配，记录事件保存有界内容寻址引用，物化批次受行数和字节数
  双重限制；私密 scope 不存在，`keeper_only` 角色与 sheet sentinel 不进入快照。
- Public events、Clues、NPC、Combat、Chase、Conclusion 与既有子实体均实际落入
  child projection；删除后从正史重建为相同 JSON，Event Store 行数不变。
- Fork 角色由 source cutoff 前的 verified events 重建；第二 Session 更新当前角色卡后，
  旧 Session fork 仍保留 cutoff sheet，且 child character/sheet 仍绑定原 owner。
- Fork materialization 分别产生 keeper、party、private 三类事件 envelope；每个私密
  事件使用玩家 `data_subject_id` 和对应有效主体密钥，projection guard 继续验证
  Visibility 与主体完全一致。
- Ending 只绑定 `ENDED` session，且 ID 必须来自该 Session 的 Scenario `endings`。
- Growth 从当前 sheet 与 opaque RNG evidence 重新计算；percentile 与可选 d10 ID、
  值和 presence 一致，新 locked sheet 成为 current，旧 sheet 保留。
- Combat 的命中、闪避与伤害，以及 Chase 的每名参与者结果，均由共享内核不可构造的
  OS CSPRNG 证据派生；领域层重算骰值、成功等级、固定伤害公式与状态转换，持久层再将
  serialized state 与同一批 opaque evidence 逐项比对。
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

## 第三方与依赖检查

| 门禁 | 结果 |
| --- | --- |
| Semgrep 1.171.0，`p/rust` + `p/security-audit` | PASS；30 targets、13 rules、0 finding、0 error、0 skipped |
| CodeRabbit 0.7.0 | CLI 登录浏览器回调未完成，`NOT_RUN_NOT_AUTHENTICATED`，未冒充结果 |
| GitHub PR #9 自动审查 | 第一轮 4 项、第二轮 5 项已修复；第三轮 5 项已在本地修复，最新提交/复审 pending |
| `cargo audit 0.22.2 --no-fetch --json` | exit `1`；381 dependencies、3 个基线 advisory |

Semgrep 扩展复扫最初对 `data_deletion_e2e.rs` 报告 2 个共享临时目录竞争问题；测试已
改用锁定版本的 `tempfile::Builder::tempdir()`，没有 suppress 规则。加入第三个 P08
migration 后，最终 30 目标复扫
最终为 0 finding。

第二轮远端自动审查真实指出：Ending/Growth 的语义唯一约束可能在正史 append 后才
失败、私密 fork 事件沿用 command-wide 主体与密钥、六类声明 scope 未实际物化，以及
无界完整 snapshot 可能超过单事件大小限制。第三轮又真实指出：Combat/Chase 仍可由
调用方决定正式结果、Growth 未绑定所选 Ending 的奖励，以及并发 Ending/Growth
仍可能各自追加孤儿正史。以上均已按问题根因修复；本报告在最新远端 CI/复审完成前
保持 pending，不以本地结果冒充远端通过。

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
