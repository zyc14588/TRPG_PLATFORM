use crate::agent_runtime::{AgentError, AgentResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LocalModelLevel {
    Level0,
    Level1,
    Level2,
    Level3,
    Level4,
}

impl LocalModelLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Level0 => "LOCAL_MODEL_LEVEL_0",
            Self::Level1 => "LOCAL_MODEL_LEVEL_1",
            Self::Level2 => "LOCAL_MODEL_LEVEL_2",
            Self::Level3 => "LOCAL_MODEL_LEVEL_3",
            Self::Level4 => "LOCAL_MODEL_LEVEL_4",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CertificationInput {
    pub model_id: String,
    pub json_schema_support: bool,
    pub tool_call_support: bool,
    pub visibility_tests_pass: bool,
    pub prompt_injection_tests_pass: bool,
    pub rules_eval_pass: bool,
    pub latency_ms: u64,
}

pub fn certify_local_model(input: &CertificationInput) -> LocalModelLevel {
    if input.json_schema_support
        && input.tool_call_support
        && input.visibility_tests_pass
        && input.prompt_injection_tests_pass
        && input.rules_eval_pass
        && input.latency_ms <= 2_000
    {
        LocalModelLevel::Level4
    } else if input.json_schema_support && input.tool_call_support && input.visibility_tests_pass {
        LocalModelLevel::Level3
    } else if input.json_schema_support || input.tool_call_support {
        LocalModelLevel::Level2
    } else if !input.model_id.trim().is_empty() {
        LocalModelLevel::Level1
    } else {
        LocalModelLevel::Level0
    }
}

pub fn ensure_ai_keeper_model(level: LocalModelLevel) -> AgentResult<()> {
    if level == LocalModelLevel::Level4 {
        Ok(())
    } else {
        Err(AgentError::LocalModelNotCertifiedForAiKp)
    }
}
