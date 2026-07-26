# P07 主责问题修复追溯

记录日期：2026-07-27（Australia/Brisbane）
基线 HEAD：`b2793988c5e2e021d556635d19e8d110a99ece8a`

## 结论

| Finding | 代码证据 | 测试证据 | 状态 |
| --- | --- | --- | --- |
| `AUD-006` | `trpg-api` 的玩家行动 contract 接到 `trpg-runtime::HumanKpPlayerActionWorkflow`；生产 `RepositoryPlayerActionPort` 接到 COC7 executor 与 `CoreDomainRepository`；API server 用 Identity、锁定 Authority、OpenFGA/OPA 和 Formal Audit 构造不可伪造的 command context；canonical transaction 写 Event/Audit/Outbox/Projection；生产 JetStream Outbox publisher 投递 Realtime | `player_action_http_integration` 用真实 TCP HTTP、primary/witness PostgreSQL、OpenFGA、OPA、NATS JetStream 跑通完整链；依赖方向检查通过 | `CLOSED_PASS` |
| `AUD-008` | Agent 与通用 Runtime 将 `tool_authorized` 和 `tool_executed` 分离；默认 executor 拒绝；只有真实 executor 成功且结果验证通过后才追加 `ToolExecutionSucceeded` 和 `DecisionCommitted`。P07 玩家行动同样先执行规则工具，再原子提交 Decision/Dice/State/Outbox | Agent/Runtime 全套相关 contract 回归通过；`human_kp_investigation_flow` 证明失败不提交、成功后提交、精确重试不重掷；`decision_state_outbox_atomicity` 证明故障回滚 | `CLOSED_PASS` |
| `AUD-013` | `Coc7PlayerActionToolExecutor` 使用操作系统 CSPRNG 产生正式百分骰；Rules 计算成功等级；持久化层重新校验 DiceRoll、Decision 关联和规则结果；核心线索失败时 fail-forward | `server_dice_and_sanity_sequence` `4/4`；HTTP 集成验证 `SERVER_OS_CSPRNG`、DiceRoll/Decision/Clue 关联及客户端骰点拒绝 | `CLOSED_PASS` |
| `AUD-030` | `apply_sanity_loss` 显式接收并保存 `day_start_sanity`，不定性疯狂阈值固定为日初 SAN 的五分之一；持久 SAN 状态保存 day key、日初基准和累计损失 | 规则分组不变量、持久 SAN transaction、Tutorial 纵向分组不变量均通过 | `CLOSED_PASS` |
| `AUD-042` | 新增 submit/confirm API DTO、HUMAN_KP 权威确认 workflow、调查/SAN executor、SQLx repository、player action/decision/dice/clue/sanity projections、生产 HTTP route 和 Outbox-backed Realtime receipt | 五条 P07 强制命令全部通过；HTTP 精确重试返回原 event range，Dice/Decision/Clue 各一份，不重掷、不追加重复事件 | `CLOSED_PASS` |

## 纵向链边界

```text
TCP HTTP
  -> bearer authentication / membership / locked Authority Contract
  -> OpenFGA + OPA + tamper-evident policy audit
  -> HumanKpPlayerActionWorkflow
  -> server-only COC7 RNG/rules tool execution
  -> one PostgreSQL transaction:
       Event Store + Formal Commit + Audit + Outbox + guarded state projection
  -> API receipt with outbox-backed realtime_delta_id
  -> production Outbox publisher
  -> JetStream ACK + canonical realtime envelope
```

正式写入仍只经过 `Command -> Workflow -> Decision -> Event Store -> Projection`。Agent、
API、前端和 KP 服务均未获得直接数据库写权限；客户端 DTO 使用
`deny_unknown_fields`，包含骰点字段的请求在进入 workflow 前被拒绝。

## 关键负例

- 客户端在调查 intent 中加入 `roll`：HTTP `400`，Event/Dice 均不增加。
- 非 Authority owner 的 human keeper 确认：HTTP `403`，不执行 RNG、不写 Dice。
- AI 或非 owner 尝试确认 HUMAN_KP pending action：Runtime 拒绝且无正式提交。
- Tool executor 失败或返回不可信结果：不产生 `DecisionCommitted`。
- projection 故障注入：Decision、Dice、Clue/SAN、Event、Outbox 全部回滚。
- API 数据库角色直接写玩家行动业务表：权限拒绝；canonical role 只能调用受 capability
  与 policy/audit 绑定的 guarded projection function。
- 同一 confirmation 精确重试：返回原 event range，不重掷且不追加第二份状态。

P07 没有实现 Combat、Chase、Major Wound、Pushed Roll 或 Reconsideration fork；这些属于
P08，当前批次没有提前施工。
