use crate::agent_runtime::{evaluate_prompt_injection, PromptInjectionReport};

pub const PROMPT_ID: &str = "CODEX-0447-04-AI-AGENT-SYSTEM-3497400719";

pub fn evaluate_golden_scenario_output(input: &str, generated_text: &str) -> PromptInjectionReport {
    evaluate_prompt_injection(input, generated_text)
}
