// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

#[macro_export]
macro_rules! define_api_realtime_contract_module {
    (
        $module_name:literal,
        $event_type:literal,
        $event_schema_name:literal,
        $operation:expr
    ) => {
        pub const MODULE_NAME: &str = $module_name;
        pub const EVENT_TYPE: &str = $event_type;
        pub const EVENT_SCHEMA_NAME: &str = $event_schema_name;

        pub fn contract() -> $crate::contract_core::ApiRealtimeContract {
            $crate::contract_core::ApiRealtimeContract::new(
                MODULE_NAME,
                EVENT_TYPE,
                EVENT_SCHEMA_NAME,
                $operation,
            )
        }

        pub fn append_contract_event<T>(
            store: &mut trpg_shared_kernel::EventStore<
                $crate::contract_core::ApiRealtimeEventPayload,
            >,
            authority: &trpg_shared_kernel::AuthorityContract,
            command: &trpg_shared_kernel::CommandEnvelope<T>,
        ) -> trpg_shared_kernel::KernelResult<
            trpg_shared_kernel::EventEnvelope<$crate::contract_core::ApiRealtimeEventPayload>,
        > {
            $crate::contract_core::append_api_contract_event(store, authority, command, &contract())
        }
    };
}

include!("contract_core/01_module_prelude.rs");
include!("contract_core/02_build_openapi_contract_document.rs");
