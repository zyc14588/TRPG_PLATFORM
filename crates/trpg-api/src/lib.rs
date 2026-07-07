pub mod api;
pub mod api_and_transport;
pub mod api_contracts;
pub mod api_web_socket;
pub mod api_web_socket_g_rpc_schema;
pub mod contract_core;
pub mod external_provider_contracts;
pub mod nats_subject_contracts;
pub mod openapi;
pub mod openapi_contract;
pub mod openapi_index;
pub mod provider;
pub mod realtime_room_sync;
pub mod realtime_sync;
pub mod request_idempotency_contract;
pub mod websocket_protocol;

pub use trpg_shared_kernel::{
    ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope, EntityId, EventEnvelope,
    EventStore, FactProvenance, FormalWritePath, PrincipalScope, ProvenanceKind, TrpgError,
    Visibility, VisibilityLabel,
};

pub fn batch_029_api_realtime_contracts() -> Vec<contract_core::ApiRealtimeContract> {
    vec![
        api_and_transport::contract(),
        external_provider_contracts::contract(),
        nats_subject_contracts::contract(),
        openapi_index::contract(),
        realtime_sync::contract(),
        request_idempotency_contract::contract(),
        websocket_protocol::contract(),
        api_web_socket::contract(),
        realtime_room_sync::contract(),
        api_contracts::contract(),
        api_web_socket_g_rpc_schema::contract(),
        openapi::contract(),
        provider::contract(),
        api::contract(),
        openapi_contract::contract(),
    ]
}
