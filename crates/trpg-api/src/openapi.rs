crate::define_api_realtime_contract_module!(
    "CODEX-0689-07-API-REALTIME-CONTRACTS-4b17a0fb09",
    "openapi",
    "OpenApiContractRecorded",
    "openapi.event_schema",
    crate::contract_core::ApiRealtimeOperation::RegisterSchema
);

pub fn document() -> crate::contract_core::OpenApiContractDocument {
    crate::contract_core::build_openapi_contract_document(&crate::batch_029_api_realtime_contracts())
}
