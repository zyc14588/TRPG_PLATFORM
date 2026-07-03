# Agent Governance Review Prompt

审查 Agent Gateway、Orchestrator、Runtime、Tool Permission Gate、Context Assembler、Provider Adapter。确认所有 AI 调用经统一路径；表达 Agent 不新增事实；Memory/Summary/Export Agent 不调用裁定工具；Safety/Moderator Agent 不调用游戏裁定工具；本地模型认证和 no silent fallback 有测试。
