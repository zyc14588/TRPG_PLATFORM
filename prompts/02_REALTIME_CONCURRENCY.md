# Codex Phase 3 — Realtime and Concurrency (Deferred; Not Current P2 Mainline)

本阶段不是当前 P2 主线；只有在 P2 Rules/RAG/Document Ingestion 验收通过后执行。

范围：WebSocket Hub、Redis Streams/PubSub、session event/snapshot、resume token、expected_version/idempotency_key、40P01/40001/55P03 分类与三次重试、Transactional Outbox replay。

验收：添加断线重连、版本冲突和死锁/序列化重试测试；不得在 P2 Rules/RAG/Ingestion gate 通过前启动本阶段实现。
