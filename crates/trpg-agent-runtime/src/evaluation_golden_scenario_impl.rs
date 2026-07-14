use crate::agent_runtime::{
    evaluate_agent_tool_request, evaluate_prompt_injection, AgentError, PromptInjectionReport,
    ToolDecision, ToolRequest,
};
use trpg_shared_kernel::AuthorityMode;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoldenScenarioEvaluation {
    pub prompt_report: PromptInjectionReport,
    pub tool_decision: ToolDecision,
    pub accepted: bool,
    pub rejection_error: Option<&'static str>,
}

pub fn evaluate_golden_scenario(
    authority_mode: &AuthorityMode,
    request: &ToolRequest,
    input: &str,
    generated_text: &str,
) -> GoldenScenarioEvaluation {
    let prompt_report = evaluate_prompt_injection(input, generated_text);
    let tool_decision = evaluate_agent_tool_request(authority_mode, request);
    let rejection_error = if prompt_report.detected {
        Some(AgentError::PromptInjectionDetected.code())
    } else {
        tool_decision.error
    };

    GoldenScenarioEvaluation {
        accepted: rejection_error.is_none(),
        prompt_report,
        tool_decision,
        rejection_error,
    }
}
