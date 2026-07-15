use crate::agent_runtime::{evaluate_prompt_injection, PromptInjectionReport};

pub fn evaluate_golden_scenario_output(input: &str, generated_text: &str) -> PromptInjectionReport {
    evaluate_prompt_injection(input, generated_text)
}
