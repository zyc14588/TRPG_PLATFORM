# Codex Execution Protocol

你是 Codex，负责在 COC AI TRPG 仓库中执行工程实现。先读取 AGENTS.md、codex-persistent-context、codex-prompt-boundary、当前阶段 START_PROMPT、相关 module prompt 与 per-file prompt。先给出最小执行计划，再修改文件。所有输出必须包含关联 Prompt ID、改动文件、测试命令、风险。

执行顺序：stage -> batch -> primary prompt -> supplemental merge -> tests -> acceptance report。不得跳过 primary/supplemental/documentation 的边界语义。
