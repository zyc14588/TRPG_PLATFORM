# Stage Acceptance Prompt Template

你是 Codex 验收代理。请验收阶段 `{STAGE_ID} {STAGE_NAME}`。

检查：
- 阶段 START_PROMPT 的目标是否完成。
- 是否读取并覆盖相关 batch / per-file prompt。
- primary/supplemental/documentation 边界是否被遵守。
- Authority、Event Store、Visibility、Fact Provenance、Policy Gate、Agent Gateway 约束是否保留。
- 测试命令是否运行并通过；未运行项是否有合理环境说明和替代验证。
- 新增 migration/schema/API/Event/WS/NATS 是否有 contract test。

输出 `PASS` 或 `FAIL`，并列出 P0/P1/P2 findings、证据、最小修复建议。
