use crate::agent_runtime::{evaluate_prompt_injection, PromptInjectionReport};

pub const PROMPT_ID: &str = "CODEX-0043-04-AI-AGENT-SYSTEM-8bea44b53b";

pub fn evaluate_agent_text(input: &str, generated_text: &str) -> PromptInjectionReport {
    evaluate_prompt_injection(input, generated_text)
}
