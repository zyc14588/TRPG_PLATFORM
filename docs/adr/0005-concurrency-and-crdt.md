# ADR-0005：乐观锁、死锁检测与 CRDT 边界

- 状态：Accepted
- 决策：战斗、回合、角色卡使用 expected_version + idempotency_key；捕获 PostgreSQL 40P01/40001/55P03 并有界重试。
- 约束：外部副作用通过 Outbox；重试耗尽返回 409 并同步快照。CRDT 仅用于协作笔记和线索布局。
