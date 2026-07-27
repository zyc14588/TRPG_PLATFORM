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
- Fork 来源 hash 精确匹配，子 materialization hash 单独封存子 ID/实体；私密 scope
  不存在，`keeper_only` 角色与 sheet sentinel 不进入快照；子实体删除后从正史重建为
  相同 JSON，Event Store 行数不变。
- Ending 只绑定 `ENDED` session。
- Growth 从当前 sheet 与 opaque RNG evidence 重新计算；percentile 与可选 d10 ID、
  值和 presence 一致，新 locked sheet 成为 current，旧 sheet 保留。
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
| Semgrep 1.171.0，`p/rust` + `p/security-audit` | PASS；23 targets、13 rules、0 finding、0 error、0 skipped |
| CodeRabbit 0.7.0 | `NOT_RUN_NOT_AUTHENTICATED`，未冒充结果 |
| `cargo audit 0.22.2 --no-fetch --json` | exit `1`；381 dependencies、3 个基线 advisory |

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
