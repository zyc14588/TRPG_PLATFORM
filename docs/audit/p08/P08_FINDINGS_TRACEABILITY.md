# P08 Findings Traceability

记录日期：2026-07-27（Australia/Brisbane）
基线：`18825746082886a63aee10891860aedb749349e1`

| Finding | 根因 | 修复代码 | 负向/正向证据 | 状态 |
| --- | --- | --- | --- | --- |
| AUD-031 | 旧伤害函数仅按本次伤害计算 condition，可清除已有 MajorWound | `combat_state_machine.rs` 的 `CombatState`、`apply_damage_with_armor`、显式恢复；领域层 `canonical_gameplay_state.rs` 再验证完整前驱转换 | `combat_condition_sequence` 验证大伤后小伤/护甲吸收仍为 MajorWound、非法恢复失败、失败的回合推进不改变聚合；真实 DB 最终 JSON 保留 `MAJOR_WOUND` | CLOSED_PASS |
| AUD-032 | 旧追逐函数不绑定当前状态，终态可重新进入 Ongoing | `chase_state_machine.rs` 的 `ChaseState`；领域层对 ID、参与者、距离、segment、version 和 exact predecessor 做二次校验 | `chase_terminal` 验证 Escaped/Caught 均拒绝推进，新追逐使用新 ID；真实 DB 保存 `CAUGHT` v2 | CLOSED_PASS |
| AUD-036 | Fork 忽略关键字段，只写 parent snapshot 行，没有子 Campaign 实体；当前 projection 过滤会遗漏 cutoff 后又更新的角色；materialization 还会把 party/private 子行全部降为 keeper-only | `fork_canon_lineage.rs`；`preview_campaign_fork`、`record_campaign_fork`、canonical cutoff replay、按 Visibility 分批的 materialization/replay；逐事件 Visibility request-hash/HMAC/Outbox 绑定；P08 两个 migration | domain tests 拒绝 self-fork、坏/不匹配 hash 和未授权私密 scope；真实 DB 证明 `keeper_only` sentinel 被排除，后续 Session Growth 不会遗漏角色或污染旧快照，child scenario/party/private 行保留安全可见性，删除后重放一致且 source 不变 | CLOSED_PASS |
| AUD-043 | 只有权限枚举，缺少完整复议实体和追加式处理 | `ReconsiderationOutcome`、request/review/resolve 状态机、正式事件与 v2 projection | 精确重复 request 幂等；Upheld 与 Corrected 均为独立事件；完成后追加失败；原始 event sequence 保留 | CLOSED_PASS |

## P08 其他完成项

| 项目 | 证据 | 状态 |
| --- | --- | --- |
| 服务端正式骰 | `ServerPercentileRoll`、`ServerD10Roll` 和 `ServerGrowthRollEvidence` 字段私有且不可反序列化；DB 保存独立唯一 ID 与 `SERVER_OS_CSPRNG` | PASS |
| Combat/Chase 防 JSON 伪造 | command boundary 接受 serialized state，但领域校验器使用 `deny_unknown_fields` 并重算唯一合法下一状态；异源同 ID 在 append 前失败 | PASS |
| Fork 公开范围 | 默认 scope 明确包含 Character/Public events/Clues/World/NPC/Scene/Combat/Chase/Conclusion；Keeper notes/Hidden clues/Private messages/AI memory 明确排除；角色与当前 sheet 均只接受公开/队伍可见或 owner-bound 玩家私有标签 | PASS |
| Fork hash 语义 | `source_snapshot_hash` 与经重新计算的来源快照一致；`child_snapshot_hash` 必须与 child materialization JSON 一致且不冒充来源 hash | PASS |
| Ending/Growth | 未结束会话负例；当前 sheet 决定 `skill_before`；percentile/d10 presence 与规则结果在应用、事件 replay 和 DB constraint 三层校验 | PASS |
| P08 projection replay | Combat、Chase、Reconsideration、Fork、Ending、Growth 从 verified canonical events 重建；重建前后 JSON 相等且不写 Event Store | PASS |
| Exact retry | Combat、Chase、Ending、Growth 对同一 commit/command/idempotency/request 返回相同 receipt 且不重复投影；不同绑定保持 fail closed | PASS |
| Ending 场景绑定 | `record_ending` 读取 ended Session 所绑定 Scenario 的 canonical `document_json.endings`，未声明 ID 在正式提交前拒绝 | PASS |
| Fork 历史 cutoff | 在来源会话结束后创建第二 Session 并再次 Growth；旧 Session snapshot/child 仍包含第一轮 sheet，父 Campaign current sheet 保持第二轮结果 | PASS |
| Fork Visibility | Scenario 为 `keeper_only`、Session/Scenes 为 `party_visible`、Character/Sheet 为 owner-bound `private_to_player`；三类 `CampaignForkMaterialized` 事件 envelope 与投影一致 | PASS |
| Tutorial 完整流程 | `tutorial_complete_e2e` 两个用例，真实 DB/Witness 主流程及提前 Ending/私密 Fork 负例 | PASS |

所有正式写入保持 Authority、Visibility、Fact Provenance、formal commit、Event Store、
Outbox 和 projection guard 边界。没有删除或覆盖源事件，没有让 projection 成为正史，
没有执行 P09。
