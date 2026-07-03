# Stage Start Prompt Template

你是 Codex。现在启动阶段 `{STAGE_ID} {STAGE_NAME}`。

必须读取：
- AGENTS.md
- docs/codex/00-index/codex-persistent-context.md
- docs/codex/00-index/codex-prompt-boundary.md
- 本阶段 START_PROMPT.md
- 本阶段相关 module code/test/review prompt
- 本阶段相关 execution batch 与 per-file prompt

请先输出：
1. 阶段理解与不可突破约束。
2. 计划修改的 crate、migration、schema、API/Event、测试、docs。
3. primary prompt 列表与 supplemental 归并策略。
4. 将运行的测试命令。
5. 风险与需要留给人工确认的事项。

随后按最小可验证切片施工。
