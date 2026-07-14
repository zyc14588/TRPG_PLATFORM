use crate::agent_runtime::{evaluate_prompt_injection, PromptInjectionReport};

pub fn evaluate_ai_golden_scenario(input: &str, generated_text: &str) -> PromptInjectionReport {
    evaluate_prompt_injection(input, generated_text)
}
