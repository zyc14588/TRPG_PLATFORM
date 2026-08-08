---
document_id: CODEX-{{BATCH}}-IMPLEMENT
schema_version: 1
document_kind: implementation-prompt
mode: IMPLEMENT
authority: execution-contract
status: ACTIVE
source_commit: "{{SOURCE_COMMIT}}"
---

# 施工入口

按有效路由和冻结契约完成当前单一目标。先验证范围与工作树；不得修改计划、契约、顶层和生成文件。完成后提交逻辑签名 Commit、运行验收命令并写 Handoff。
