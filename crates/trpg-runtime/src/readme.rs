pub const PROMPT_ID: &str = "CODEX-0358-03-RUNTIME-ORCHESTRATION-5626fcbd5c";
pub const SUPPLEMENTAL_PROMPT_IDS: &[&str] = &["CODEX-0374-03-RUNTIME-ORCHESTRATION-989f2ac19c"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeReadmeContract {
    pub module_prefix: &'static str,
    pub crate_name: &'static str,
    pub invariants: &'static [&'static str],
    pub non_goals: &'static [&'static str],
}

pub fn runtime_readme_contract() -> RuntimeReadmeContract {
    RuntimeReadmeContract {
        module_prefix: "runtime_orchestration",
        crate_name: "trpg-runtime",
        invariants: &[
            "Authority Contract is immutable",
            "AI can only propose tool calls or draft decisions",
            "Formal state is committed through Command -> Workflow -> Decision -> Event Store",
            "Visibility labels and fact provenance stay on events",
        ],
        non_goals: &[
            "direct LLM provider calls",
            "agent direct database writes",
            "projection as canon",
        ],
    }
}
