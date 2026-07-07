crate::define_api_realtime_contract_module!(
    "CODEX-0069-07-API-REALTIME-CONTRACTS-3cc61a7d01",
    "openapi_index",
    "OpenApiIndexContractRecorded",
    "openapi_index.event_schema",
    crate::contract_core::ApiRealtimeOperation::RegisterSchema
);

pub fn document_index() -> crate::contract_core::OpenApiContractDocument {
    crate::contract_core::build_openapi_contract_document(&crate::batch_029_api_realtime_contracts())
}
