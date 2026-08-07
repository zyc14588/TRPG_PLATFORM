---
document_id: CODEX-{{BATCH}}-REPAIR-{{ROUND}}
schema_version: 1
document_kind: repair-prompt
mode: REPAIR
authority: execution-contract
status: ACTIVE
source_commit: "{{SOURCE_COMMIT}}"
---

# 受限修复

只修复列出的 `ACC-*`，每个提交引用问题 ID。不得扩大范围、改变契约或混入无关重构。
