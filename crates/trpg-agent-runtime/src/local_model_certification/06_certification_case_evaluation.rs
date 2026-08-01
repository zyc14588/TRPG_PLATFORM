use crate::model_provider::{
    ModelChatRequest, ModelChatResponse, ModelMessage, ModelMessageRole, ModelOperation,
    ModelToolDefinition, ProviderCapabilities, ProviderExecution, StructuredOutputRequest,
};

struct CaseFailure {
    code: String,
    latency_ms: u64,
    retry_count: u32,
}

impl LocalModelCertificationRunner {
    async fn execute_case(
        &self,
        kind: CertificationCaseKind,
        cancellation: &ProviderCancellation,
    ) -> CertificationCaseEvidence {
        if kind == CertificationCaseKind::CapabilityProbe {
            return self.execute_capability_case(cancellation).await;
        }
        let request = certification_chat_request(kind);
        let request_bytes = match serde_json::to_vec(&request) {
            Ok(bytes) => bytes,
            Err(_) => {
                return failed_case(
                    kind,
                    kind.as_str().as_bytes(),
                    None,
                    0,
                    0,
                    "certification_request_encoding_failed",
                )
            }
        };
        let repetitions = if kind == CertificationCaseKind::ToolUseStability {
            3
        } else {
            1
        };
        let mut responses = Vec::with_capacity(repetitions);
        let mut latency_ms = 0_u64;
        let mut retry_count = 0;
        for _ in 0..repetitions {
            match self.chat_once(&request, cancellation).await {
                Ok((execution, elapsed, retries)) => {
                    latency_ms = latency_ms.saturating_add(elapsed);
                    retry_count += retries;
                    if !self.route_matches(&execution, ModelOperation::Chat) {
                        return failed_case(
                            kind,
                            &request_bytes,
                            None,
                            latency_ms,
                            retry_count,
                            "certification_route_mismatch",
                        );
                    }
                    responses.push(execution.output);
                }
                Err(failure) => {
                    return failed_case(
                        kind,
                        &request_bytes,
                        None,
                        failure.latency_ms,
                        failure.retry_count,
                        &failure.code,
                    )
                }
            }
        }
        let response_bytes = serde_json::to_vec(&responses).unwrap_or_default();
        let valid = validate_case_responses(
            kind,
            &responses,
            latency_ms,
            self.suite.maximum_latency_ms,
        );
        if !valid {
            return failed_case(
                kind,
                &request_bytes,
                Some(&response_bytes),
                latency_ms,
                retry_count,
                case_validation_error(kind),
            );
        }
        passed_case(
            kind,
            &request_bytes,
            &response_bytes,
            latency_ms,
            retry_count,
        )
    }

    async fn execute_capability_case(
        &self,
        cancellation: &ProviderCancellation,
    ) -> CertificationCaseEvidence {
        let kind = CertificationCaseKind::CapabilityProbe;
        let request_bytes = kind.as_str().as_bytes();
        let (execution, latency_ms, retry_count) = match self.probe_with_retry(cancellation).await {
            Ok(result) => result,
            Err(failure) => {
                return failed_case(
                    kind,
                    request_bytes,
                    None,
                    failure.latency_ms,
                    failure.retry_count,
                    &failure.code,
                )
            }
        };
        let response_bytes = serde_json::to_vec(&execution.output).unwrap_or_default();
        if !self.route_matches(&execution, ModelOperation::CapabilityProbe)
            || execution.output != ProviderCapabilities::v1_complete()
        {
            return failed_case(
                kind,
                request_bytes,
                Some(&response_bytes),
                latency_ms,
                retry_count,
                "capability_probe_failed",
            );
        }
        passed_case(
            kind,
            request_bytes,
            &response_bytes,
            latency_ms,
            retry_count,
        )
    }

    async fn probe_with_retry(
        &self,
        cancellation: &ProviderCancellation,
    ) -> Result<(ProviderExecution<ProviderCapabilities>, u64, u32), CaseFailure> {
        let started = Instant::now();
        let mut retries = 0;
        loop {
            match tokio::time::timeout(
                self.case_timeout,
                self.provider.probe_capabilities(cancellation),
            )
            .await
            {
                Ok(Ok(execution)) => {
                    return Ok((execution, elapsed_ms(started), retries));
                }
                Ok(Err(error)) if error.retryable() && retries < self.maximum_probe_retries => {
                    retries += 1;
                }
                Ok(Err(error)) => {
                    return Err(CaseFailure {
                        code: error.code().to_owned(),
                        latency_ms: elapsed_ms(started),
                        retry_count: retries,
                    });
                }
                Err(_) if retries < self.maximum_probe_retries => {
                    retries += 1;
                }
                Err(_) => {
                    return Err(CaseFailure {
                        code: "certification_case_timeout".to_owned(),
                        latency_ms: elapsed_ms(started),
                        retry_count: retries,
                    });
                }
            }
        }
    }

    async fn chat_once(
        &self,
        request: &ModelChatRequest,
        cancellation: &ProviderCancellation,
    ) -> Result<(ProviderExecution<ModelChatResponse>, u64, u32), CaseFailure> {
        let started = Instant::now();
        match tokio::time::timeout(
            self.case_timeout,
            self.provider.chat(request, cancellation),
        )
        .await
        {
            Ok(Ok(execution)) => Ok((execution, elapsed_ms(started), 0)),
            Ok(Err(error)) => Err(CaseFailure {
                code: error.code().to_owned(),
                latency_ms: elapsed_ms(started),
                retry_count: 0,
            }),
            Err(_) => Err(CaseFailure {
                code: "certification_case_timeout".to_owned(),
                latency_ms: elapsed_ms(started),
                retry_count: 0,
            }),
        }
    }

    fn route_matches<T>(
        &self,
        execution: &ProviderExecution<T>,
        operation: ModelOperation,
    ) -> bool {
        execution.route.provider_id == *self.provider.provider_id()
            && execution.route.provider_type == self.provider.provider_type()
            && execution.route.model_id == self.provider.model_id()
            && execution.route.operation == operation
    }
}

fn certification_chat_request(kind: CertificationCaseKind) -> ModelChatRequest {
    let user_prompt = match kind {
        CertificationCaseKind::Golden => GOLDEN_PROMPT.to_owned(),
        CertificationCaseKind::ToolUseStability => TOOL_STABILITY_PROMPT.to_owned(),
        CertificationCaseKind::VisibilityLeakage => VISIBILITY_PROMPT.to_owned(),
        CertificationCaseKind::PromptInjection => PROMPT_INJECTION_PROMPT.to_owned(),
        CertificationCaseKind::CocRulesMiniEval => RULES_PROMPT.to_owned(),
        CertificationCaseKind::Latency => LATENCY_PROMPT.to_owned(),
        CertificationCaseKind::ContextStress => format!(
            "{CONTEXT_PROMPT_PREFIX}; {} {CONTEXT_TAIL_CANARY}",
            "bounded-context ".repeat(CONTEXT_STRESS_REPETITIONS)
        ),
        CertificationCaseKind::CapabilityProbe => unreachable!("probe is not a chat case"),
    };
    let tools = if kind == CertificationCaseKind::ToolUseStability {
        vec![ModelToolDefinition {
            name: CERTIFICATION_TOOL_NAME.to_owned(),
            description: CERTIFICATION_TOOL_DESCRIPTION.to_owned(),
            input_schema: serde_json::from_str(CERTIFICATION_TOOL_SCHEMA)
                .expect("built-in certification tool schema must be valid"),
        }]
    } else {
        Vec::new()
    };
    ModelChatRequest {
        messages: vec![
            ModelMessage {
                role: ModelMessageRole::System,
                content: CERTIFICATION_SYSTEM_PROMPT.to_owned(),
            },
            ModelMessage {
                role: ModelMessageRole::User,
                content: user_prompt,
            },
        ],
        structured_output: (kind != CertificationCaseKind::ToolUseStability).then(|| {
            StructuredOutputRequest {
                name: format!("{}_result", kind.as_str()),
                schema: serde_json::json!({"type": "object"}),
            }
        }),
        tools,
    }
}
