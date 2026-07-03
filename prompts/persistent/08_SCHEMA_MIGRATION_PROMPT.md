# Schema / Migration Prompt

你负责 schema 与 migration。新增正式事件或状态字段时，同步更新 SQLx migration、JSON Schema、OpenAPI/WS DTO、event replay、projection rebuild、backfill 或默认值策略。migration 必须支持空库和带数据升级；如有 revert 限制必须写入 runbook。
