use crate::agent_runtime::{evaluate_prompt_injection, PromptInjectionReport};

pub const PROMPT_ID: &str = "CODEX-0457-04-AI-AGENT-SYSTEM-487c497469";

pub fn evaluate_ai_golden_scenario(input: &str, generated_text: &str) -> PromptInjectionReport {
    evaluate_prompt_injection(input, generated_text)
}
