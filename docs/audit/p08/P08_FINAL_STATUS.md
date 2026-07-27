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
HOSTED_CI = NOT_RUN
P09_IMPLEMENTATION = NOT_STARTED
```

P08 已完成 Combat、Chase、Reconsideration、Fork、Ending/Growth 和 Tutorial
完整流程。正式状态继续经过
`Command -> Workflow -> Decision -> Event Store -> Projection`，投影可从经过
HMAC 与 Witness 校验的正史事件重建。

## 验收矩阵

| 验收项 | 代码与真实证据 | 状态 |
| --- | --- | --- |
| MajorWound 持续 | 聚合保存 prior condition；后续小伤或护甲全吸收不会清除；只有成功的服务端医疗骰恢复事件可清除 | PASS |
| 多角色战斗 | DEX 先攻、跨轮次推进、近战/射击、闪避、伤害、护甲、终态以及失败不变更聚合均有测试 | PASS |
| Chase 终态 | `Escaped`/`Caught` 后普通推进失败；新追逐必须使用新 ID | PASS |
| 复议追加链 | Request → Review → Upheld/Corrected 均为正式事件；精确重试幂等，原事件不删除 | PASS |
| Fork 范围与 Hash | 来源快照 hash 被重新计算并精确匹配请求；子 Campaign 保存相同的 `source_snapshot_hash`，另以不同的 `child_snapshot_hash` 封存子 ID 与实体；私密 scope 以及 `keeper_only` 角色/角色卡均被排除 | PASS |
| Fork 实体化与重放 | 子 Campaign 中实际创建 scenario、character/sheet、ended session、scenes 和 manifest；删除这些投影后可从子 Campaign 正史逐字节重建 | PASS |
| 结局与成长 | 活跃会话不能结局；成长从共享内核不可构造的 OS CSPRNG 证据计算，percentile 与可选 d10 各有唯一 ID，并生成新锁定角色卡版本 | PASS |
| Tutorial 完整闭环 | 真实 PostgreSQL 上完成角色、场景、调查、服务端骰、线索、SAN、战斗、追逐、结局、成长、复议和 Fork | PASS |
| Schema/最小权限 | 两个 forward migration、projection guards、可延迟外键、成长算术/证据约束及角色权限断言 | PASS |
| 第三方检查 | Semgrep 1.171.0 本机扫描 23 个 P08 Rust/SQL/CI 目标，13 条适用规则，0 finding、0 error、0 skipped | PASS |

## 反伪造修复

- 原先只验证一行 snapshot 的 Fork 已替换为子 Campaign 所有的正式事件批次、实际实体和可重放 manifest。
- 默认 Fork 的角色查询同时约束角色行和当前角色卡的 Visibility；真实数据库负例证明
  `keeper_only` 角色名称与角色卡 sentinel 均不会进入快照。
- 原先可由调用方提交的 Combat/Chase JSON 已替换为严格 shape 与前驱转换校验；同 ID 的异源聚合会在 Event Store append 前失败。
- 原先可提交原始成长数值的路径已替换为不可反序列化、字段私有的服务端随机证据；持久层从当前角色卡重新计算结果。
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
锁定版本，因此没有把该扫描伪报为通过。CodeRabbit 未认证，Hosted CI 未运行；
实际第三方代码检查由 Semgrep 完成。P08 到此停止，未执行 P09。
