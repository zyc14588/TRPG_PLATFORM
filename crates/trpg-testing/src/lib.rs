pub mod ai_evaluation_golden_scenario;
pub mod benchmark_plan;
pub mod contract_test_matrix;
pub mod decision_trace_map;
pub mod golden_ci_test_matrix;
pub mod golden_scenario_ci;
pub mod golden_scenarios_ci_impl;
pub mod implementation_acceptance_checklist;
pub mod implementation_acceptance_checklist_source_contract;
pub mod model_certification_tests;
pub mod principle_to_doc_trace;
pub mod readme;
pub mod replay_consistency_tests;
pub mod requirement_to_test_trace;
pub mod runtime_pending_decision;
pub mod test_strategy;
pub mod test_strategy_impl;
pub mod testing_golden_ci;
pub mod testing_golden_scenarios_ci;
pub mod top_level_principle_trace;
pub mod visibility_leakage_tests;

use trpg_shared_kernel::{
    ActorRole, AuthorityMode, CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError,
};

pub const TESTING_QUALITY_DECISION_RECORDED_EVENT: &str = "testing_quality.decision_recorded";
pub const TESTING_QUALITY_METRIC_MODULE: &str = "testing_quality";
pub const TESTING_QUALITY_REQUIRED_METRICS: &[&str] = &[
    "trpg_testing_contract_total",
    "trpg_testing_golden_scenario_total",
    "trpg_testing_visibility_leakage_total",
    "trpg_testing_export_diff_total",
    "trpg_testing_model_certification_total",
];

pub const REQUIRED_COMMAND_FIELDS: &[&str] = &[
    "idempotency_key",
    "expected_version",
    "actor",
    "authority_mode",
    "visibility",
    "fact_provenance",
    "correlation_id",
    "causation_id",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestingQualityAction {
    ValidateBenchmarkPlan,
    ValidateContractTestMatrix,
    VerifyModelCertification,
    VerifyReplayConsistency,
    VerifyTestStrategy,
    VerifyGoldenCi,
    VerifyVisibilityLeakage,
    VerifyDecisionTraceMap,
    VerifyGoldenScenarioSuite,
    VerifyGoldenScenarioCi,
    VerifyImplementationAcceptance,
    VerifyReadme,
    VerifyGoldenCiTestMatrix,
    VerifyImplementationAcceptanceChecklistSourceContract,
    VerifyTopLevelPrincipleTrace,
    VerifyRuntimePendingDecision,
    VerifyAiEvaluationGoldenScenario,
    VerifyRequirementToTestTrace,
    VerifyPrincipleToDocTrace,
    VerifyGoldenScenariosCiImpl,
    VerifyTestStrategyImpl,
}

impl TestingQualityAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ValidateBenchmarkPlan => "validate_benchmark_plan",
            Self::ValidateContractTestMatrix => "validate_contract_test_matrix",
            Self::VerifyModelCertification => "verify_model_certification",
            Self::VerifyReplayConsistency => "verify_replay_consistency",
            Self::VerifyTestStrategy => "verify_test_strategy",
            Self::VerifyGoldenCi => "verify_golden_ci",
            Self::VerifyVisibilityLeakage => "verify_visibility_leakage",
            Self::VerifyDecisionTraceMap => "verify_decision_trace_map",
            Self::VerifyGoldenScenarioSuite => "verify_golden_scenario_suite",
            Self::VerifyGoldenScenarioCi => "verify_golden_scenario_ci",
            Self::VerifyImplementationAcceptance => "verify_implementation_acceptance",
            Self::VerifyReadme => "verify_readme",
            Self::VerifyGoldenCiTestMatrix => "verify_golden_ci_test_matrix",
            Self::VerifyImplementationAcceptanceChecklistSourceContract => {
                "verify_implementation_acceptance_checklist_source_contract"
            }
            Self::VerifyTopLevelPrincipleTrace => "verify_top_level_principle_trace",
            Self::VerifyRuntimePendingDecision => "verify_runtime_pending_decision",
            Self::VerifyAiEvaluationGoldenScenario => "verify_ai_evaluation_golden_scenario",
            Self::VerifyRequirementToTestTrace => "verify_requirement_to_test_trace",
            Self::VerifyPrincipleToDocTrace => "verify_principle_to_doc_trace",
            Self::VerifyGoldenScenariosCiImpl => "verify_golden_scenarios_ci_impl",
            Self::VerifyTestStrategyImpl => "verify_test_strategy_impl",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestingQualityCommand {
    pub module: &'static str,
    pub action: TestingQualityAction,
    pub required_command_fields: &'static [&'static str],
    pub fixture_paths: &'static [&'static str],
    pub required_assertions: &'static [&'static str],
}

impl TestingQualityCommand {
    pub fn from_contract(contract: &TestingQualityModuleContract) -> Self {
        Self {
            module: contract.module,
            action: contract.action,
            required_command_fields: REQUIRED_COMMAND_FIELDS,
            fixture_paths: contract.fixture_paths,
            required_assertions: contract.required_assertions,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TestingQualityEvent {
    ContractValidated {
        module: &'static str,
        action: TestingQualityAction,
        fixture_count: usize,
        assertion_count: usize,
    },
}

pub type TestingQualityEventEnvelope = EventEnvelope<TestingQualityEvent>;
pub type TestingQualityRepository = EventStore<TestingQualityEvent>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestingQualityModuleContract {
    pub prompt_id: &'static str,
    pub module: &'static str,
    pub source_file: &'static str,
    pub test_file: &'static str,
    pub output_role: &'static str,
    pub action: TestingQualityAction,
    pub fixture_paths: &'static [&'static str],
    pub required_assertions: &'static [&'static str],
    pub allows_rust_output: bool,
}

pub fn standard_contract(
    prompt_id: &'static str,
    module: &'static str,
    source_file: &'static str,
    test_file: &'static str,
    action: TestingQualityAction,
    fixture_paths: &'static [&'static str],
    required_assertions: &'static [&'static str],
) -> TestingQualityModuleContract {
    TestingQualityModuleContract {
        prompt_id,
        module,
        source_file,
        test_file,
        output_role: "primary-implementation",
        action,
        fixture_paths,
        required_assertions,
        allows_rust_output: true,
    }
}

pub fn command_for_contract(
    contract: &TestingQualityModuleContract,
) -> CommandEnvelope<TestingQualityCommand> {
    CommandEnvelope::governed(
        TestingQualityCommand::from_contract(contract),
        ActorRole::Workflow,
        AuthorityMode::HumanKp,
    )
}

pub fn validate_contract_baseline(contract: &TestingQualityModuleContract) -> KernelResult<()> {
    if contract.prompt_id.trim().is_empty()
        || contract.module.trim().is_empty()
        || contract.source_file.trim().is_empty()
        || contract.test_file.trim().is_empty()
    {
        return Err(TrpgError::InvalidConfiguration(
            "testing_contract_missing_field",
        ));
    }
    if !contract.module.starts_with("testing_quality::") {
        return Err(TrpgError::InvalidConfiguration(
            "testing_contract_module_not_current_safe",
        ));
    }
    if contains_legacy_token(contract.module)
        || contains_legacy_token(contract.source_file)
        || contains_legacy_token(contract.test_file)
    {
        return Err(TrpgError::InvalidConfiguration(
            "testing_contract_legacy_token",
        ));
    }
    if contract.fixture_paths.is_empty() || contract.required_assertions.is_empty() {
        return Err(TrpgError::InvalidConfiguration(
            "testing_contract_missing_assertion",
        ));
    }
    Ok(())
}

pub fn evaluate_testing_quality(
    module: &'static str,
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    if module != command.payload.module {
        return Err(TrpgError::InvalidConfiguration(
            "testing_contract_module_mismatch",
        ));
    }
    for field in REQUIRED_COMMAND_FIELDS {
        if !command.payload.required_command_fields.contains(field) {
            return Err(TrpgError::InvalidConfiguration(
                "testing_contract_missing_required_command_field",
            ));
        }
    }

    repository.append(
        command,
        TESTING_QUALITY_DECISION_RECORDED_EVENT,
        TestingQualityEvent::ContractValidated {
            module,
            action: command.payload.action,
            fixture_count: command.payload.fixture_paths.len(),
            assertion_count: command.payload.required_assertions.len(),
        },
    )
}

pub fn record_contract_decision(
    contract: &TestingQualityModuleContract,
) -> KernelResult<(
    TestingQualityRepository,
    CommandEnvelope<TestingQualityCommand>,
    TestingQualityEventEnvelope,
)> {
    validate_contract_baseline(contract)?;
    let command = command_for_contract(contract);
    let mut repository = TestingQualityRepository::default();
    let event = evaluate_testing_quality(contract.module, &mut repository, &command)?;
    Ok((repository, command, event))
}

pub fn primary_contracts() -> Vec<TestingQualityModuleContract> {
    vec![
        benchmark_plan::contract(),
        model_certification_tests::contract(),
        replay_consistency_tests::contract(),
        test_strategy::contract(),
        testing_golden_ci::contract(),
        visibility_leakage_tests::contract(),
        decision_trace_map::contract(),
        contract_test_matrix::contract(),
        testing_golden_scenarios_ci::contract(),
        golden_scenario_ci::contract(),
        implementation_acceptance_checklist::contract(),
        readme::contract(),
        golden_ci_test_matrix::contract(),
        implementation_acceptance_checklist_source_contract::contract(),
        top_level_principle_trace::contract(),
        runtime_pending_decision::contract(),
        ai_evaluation_golden_scenario::contract(),
        requirement_to_test_trace::contract(),
        principle_to_doc_trace::contract(),
        golden_scenarios_ci_impl::contract(),
        test_strategy_impl::contract(),
    ]
}

fn contains_legacy_token(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    ["v3", "v4", "v5", "v6", "legacy", "fix-history"]
        .iter()
        .any(|token| lower.contains(token))
}
