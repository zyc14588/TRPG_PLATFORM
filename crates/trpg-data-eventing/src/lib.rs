// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

pub mod adr_0002_event_sourcing_cqrs;
pub mod adr_0002_event_sourcing_cqrs_event_sourcing_cqrs;
pub mod adr_0004_nats_jetstream;
pub mod adr_0005_postgres_pgvector;
pub mod adr_0005_postgres_pgvector_postgre_sql_pgvector;
pub mod adr_0010_rag_snapshot_rag_snapshot;
pub mod api_websocket_nats_contracts;
pub mod cache_redis;
pub mod cache_redis_impl;
pub mod database_schema_index;
pub mod domain_event_sourcing_projection;
pub mod event_bus_nats;
pub mod event_bus_nats_impl;
pub mod event_command_json_schema;
pub mod event_json_schema;
pub mod event_json_schema_source_contract;
pub mod event_schema_index;
pub mod event_sourcing_snapshot_projection;
pub mod event_store_projections;
pub mod event_store_sqlx_outbox_projection;
pub mod nats_jet_stream;
pub mod nats_subject_contracts;
pub mod nats_subjects;
pub mod nats_subjects_source_contract;
pub mod outbox_projection_workers;
pub mod persistence;
pub mod persistence_migrations;
pub mod persistence_postgresql;
pub mod persistence_postgresql_impl;
pub mod postgre_sql_sq_lx_pgvector;
pub mod rag_snapshot;
pub mod readme;
pub mod realtime_identity;
pub mod realtime_resume;
pub mod redis_cache_presence;
pub mod redis_presence;
pub mod schema;
pub mod snapshot;
pub mod snapshot_strategy;
pub mod sqlx_migrations;
pub mod sqlx_migrations_contract;

#[macro_export]
macro_rules! define_data_event_module {
    (
        $command:ident,
        $operation_ty:ident,
        $append_fn:ident,
        $module_name:literal,
        $event_type:literal,
        $schema_name:literal,
        $operation_kind:expr,
        [$($read_model:literal),* $(,)?]
    ) => {
        pub const MODULE_NAME: &str = $module_name;
        pub const EVENT_TYPE: &str = $event_type;
        pub const EVENT_SCHEMA_NAME: &str = $schema_name;
        pub const READ_MODELS: &[&str] = &[$($read_model),*];

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $command {
            pub operation: $operation_ty,
            pub reason: &'static str,
        }

        impl $command {
            pub fn record(reason: &'static str) -> Self {
                Self {
                    operation: $operation_ty::RecordGovernedChange,
                    reason,
                }
            }
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $operation_ty {
            RecordGovernedChange,
        }

        pub fn $append_fn<T>(
            store: &mut trpg_shared_kernel::EventStore<$crate::DataEventPayload>,
            contract: &trpg_shared_kernel::AuthorityContract,
            command: &trpg_shared_kernel::CommandEnvelope<T>,
        ) -> $crate::DataEventResult<trpg_shared_kernel::EventEnvelope<$crate::DataEventPayload>> {
            $crate::append_data_event(
                store,
                contract,
                command,
                $crate::DataEventWrite::new(
                    MODULE_NAME,
                    EVENT_TYPE,
                    $operation_kind,
                    READ_MODELS,
                ),
            )
        }

        pub fn contract() -> $crate::DataEventContract {
            $crate::DataEventContract::new(
                MODULE_NAME,
                EVENT_TYPE,
                $operation_kind,
                MODULE_NAME,
                EVENT_SCHEMA_NAME,
            )
        }
    };
}

#[macro_export]
macro_rules! define_data_event_artifacts {
    (
        $service:ident,
        $repository:ident,
        $event:ident,
        $error:ident,
        $event_type:ident,
        $schema_name:ident
    ) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $event {
            pub event_type: &'static str,
            pub schema_name: &'static str,
        }

        impl $event {
            pub const fn new() -> Self {
                Self {
                    event_type: $event_type,
                    schema_name: $schema_name,
                }
            }
        }

        impl Default for $event {
            fn default() -> Self {
                Self::new()
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum $error {
            GovernanceViolation(&'static str),
        }
    };
}

include!("lib_sections/01_module_prelude.rs");
include!("lib_sections/02_is_current_safe_name.rs");
