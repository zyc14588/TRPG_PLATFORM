# Design Constraints Prompt

在任何实现方案中强制保持：Authority Contract 不可变；HUMAN_KP/AI_KP Campaign 级互斥；AI 不直接写正式状态；Event Store 是正史；Visibility 与 Fact Provenance 全链路传播；Agent 通过 Tool Permission Gate；业务层不直接调用 LLM；本地模型无静默云端 fallback；玩家版导出不含 keeper_only/private/ai_internal。
