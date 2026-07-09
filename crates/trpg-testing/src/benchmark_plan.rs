use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0089-10-TESTING-QUALITY-da28af3028";
pub const MODULE: &str = "testing_quality::benchmark_plan";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BenchmarkBudget {
    pub name: &'static str,
    pub max_ms: u64,
    pub required_gate: &'static str,
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/benchmark_plan.rs",
        "crates/trpg-testing/tests/benchmark_plan_contract_tests.rs",
        TestingQualityAction::ValidateBenchmarkPlan,
        &[
            "fixtures/stages/S11_stage_acceptance_fixture.v1.json.md",
            "test-data/provider_model_certification_cases.md",
        ],
        &[
            "benchmark_thresholds_are_explicit",
            "ci_gate_records_visibility_and_provenance",
            "no_benchmark_metric_uses_legacy_names",
        ],
    )
}

pub fn required_budgets() -> Vec<BenchmarkBudget> {
    vec![
        BenchmarkBudget {
            name: "golden_scenario_replay",
            max_ms: 2_000,
            required_gate: "golden_scenario_passes",
        },
        BenchmarkBudget {
            name: "visibility_export_diff",
            max_ms: 500,
            required_gate: "no_visibility_leakage",
        },
        BenchmarkBudget {
            name: "model_certification_eval",
            max_ms: 2_000,
            required_gate: "eval_report_written",
        },
    ]
}

pub fn sample_within_budget(name: &str, actual_ms: u64) -> bool {
    required_budgets()
        .into_iter()
        .find(|budget| budget.name == name)
        .map(|budget| actual_ms <= budget.max_ms)
        .unwrap_or(false)
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
