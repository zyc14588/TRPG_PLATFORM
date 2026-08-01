use std::env;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxPublisher;
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    AtomicCommitDraft, CanonicalEventDraft, PolicyAuditDraft, PostgresCanonicalStore,
};
use trpg_data_eventing::realtime_identity::{PersistentRealtimeIdentity, RealtimeIdentitySession};
use trpg_identity::{CampaignRole, GlobalRole, IdentityService};
use trpg_shared_kernel::{
    AuthorityContract, AuthorityContractDraft, AuthorityMode, AuthorityVersionSnapshotDraft,
    EntityId, EventActorOriginWire, Visibility,
};

const CAMPAIGN: &str = "campaign_ar07_real";
const GROUP: &str = "group_ar07_red";
const OWNER: &str = "owner_ar07_real";
const KEEPER: &str = "keeper_ar07_real";
const PLAYER_A: &str = "player_a_ar07_real";
const PLAYER_B: &str = "player_b_ar07_real";
const SPECTATOR: &str = "spectator_ar07_real";
const PASSWORD: &str = "AR07 real service password";
const IDENTITY_KEY: [u8; 32] = [0x31; 32];
const INTEGRITY_KEY: [u8; 32] = [0x42; 32];
const PAYLOAD_KEY: [u8; 32] = [0x53; 32];

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} is required for the AR07 real-service gate"))
}

fn now_unix_ms() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_millis(),
    )
    .expect("current timestamp fits u64")
}

fn authority(now: u64) -> AuthorityContract {
    AuthorityContract::new_locked(AuthorityContractDraft {
        contract_id: "authority_ar07_real_1".to_owned(),
        campaign_id: CAMPAIGN.to_owned(),
        mode: AuthorityMode::HumanKp,
        authority_owner: KEEPER.to_owned(),
        version: 1,
        snapshot: AuthorityVersionSnapshotDraft {
            ruleset_version: "coc7_rules_ar07".to_owned(),
            house_rules_version: "house_rules_ar07".to_owned(),
            scenario_version: "scenario_ar07".to_owned(),
            prompt_version: "prompt_ar07".to_owned(),
            agent_pack_version: "agent_pack_ar07".to_owned(),
            tool_schema_version: "tool_schema_ar07".to_owned(),
            safety_profile_version: "safety_ar07".to_owned(),
            ai_provider_snapshot: "provider_ar07".to_owned(),
            model_route_snapshot: "route_ar07".to_owned(),
            character_sheet_template_version: "sheet_ar07".to_owned(),
        },
        created_at_unix_ms: now,
    })
    .expect("valid AR07 authority")
}

fn draft(
    ordinal: u64,
    expected_version: i64,
    event_type: &str,
    visibility_label: &str,
    visibility_subject: &str,
) -> AtomicCommitDraft {
    AtomicCommitDraft {
        commit_id: format!("commit_ar07_{ordinal}"),
        campaign_id: CAMPAIGN.to_owned(),
        stream_id: CAMPAIGN.to_owned(),
        idempotency_key: format!("idempotency_ar07_{ordinal}"),
        expected_version,
        command_id: format!("command_ar07_{ordinal}"),
        authenticated_actor_id: "workflow_ar07".to_owned(),
        authenticated_actor_role: "workflow".to_owned(),
        authenticated_actor_origin: EventActorOriginWire::Workload {
            role: "workflow_engine".to_owned(),
        },
        authority_mode: "human_kp".to_owned(),
        authority_contract_version: 1,
        authority_contract_id: "authority_ar07_real_1".to_owned(),
        authority_owner: KEEPER.to_owned(),
        visibility_label: visibility_label.to_owned(),
        visibility_subject: visibility_subject.to_owned(),
        data_subject_id: if matches!(
            visibility_label,
            "private_to_player" | "investigator_private"
        ) {
            visibility_subject.to_owned()
        } else {
            "not_applicable".to_owned()
        },
        provenance_kind: "rules_engine_decision".to_owned(),
        provenance_reference: format!("decision_ar07_{ordinal}"),
        provenance_recorded_by: "rules_engine_ar07".to_owned(),
        correlation_id: format!("correlation_ar07_{ordinal}"),
        causation_id: format!("causation_ar07_{ordinal}"),
        trace_id: format!("trace_ar07_{ordinal}"),
        events: vec![CanonicalEventDraft {
            event_type: event_type.to_owned(),
            payload_json: format!(r#"{{"ordinal":{ordinal}}}"#),
            visibility: None,
            projection_targets: Vec::new(),
        }],
        audit: PolicyAuditDraft {
            actor_id: KEEPER.to_owned(),
            actor_origin: "user_session".to_owned(),
            authentication_reference: "session_ar07".to_owned(),
            resource_type: "campaign".to_owned(),
            resource_id: CAMPAIGN.to_owned(),
            action: "write_official_state".to_owned(),
            requested_role: "human_keeper".to_owned(),
            openfga_decision_id: format!("openfga_ar07_{ordinal}"),
            openfga_policy_revision: "openfga_ar07".to_owned(),
            opa_decision_id: format!("opa_ar07_{ordinal}"),
            opa_policy_revision: "opa_ar07".to_owned(),
        },
    }
}

async fn reset_dedicated_database(database_url: &str, expected_name: &str) {
    assert_eq!(
        env::var("AR07_ALLOW_DATABASE_RESET").as_deref(),
        Ok("1"),
        "AR07_ALLOW_DATABASE_RESET=1 is required"
    );
    let options = PgConnectOptions::from_str(database_url).expect("valid AR07 database URL");
    assert!(matches!(
        options.get_host(),
        "127.0.0.1" | "localhost" | "::1"
    ));
    assert_eq!(options.get_database(), Some(expected_name));
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect dedicated AR07 database");
    sqlx::raw_sql(
        "DROP SCHEMA IF EXISTS core_domain CASCADE; \
         DROP SCHEMA public CASCADE; \
         CREATE SCHEMA public; \
         GRANT ALL ON SCHEMA public TO public;",
    )
    .execute(&pool)
    .await
    .expect("reset dedicated AR07 database");
}

async fn realtime_pool(realtime_url: &str) -> sqlx::PgPool {
    let options = PgConnectOptions::from_str(realtime_url).expect("valid AR07 realtime URL");
    PgPoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .expect("connect through least-privilege realtime login")
}

async fn visible_types(
    session: &RealtimeIdentitySession,
    events: &[trpg_data_eventing::event_store_sqlx_outbox_projection::CanonicalReplayEvent],
    now: u64,
) -> Vec<String> {
    let mut visible = Vec::new();
    for event in events {
        let campaign = EntityId::new(&event.campaign_id).expect("valid event campaign");
        let visibility = Visibility::try_from_parts(
            &event.visibility_label,
            (event.visibility_subject != "not_applicable")
                .then_some(event.visibility_subject.as_str()),
        )
        .expect("valid canonical visibility");
        if session
            .can_view(&campaign, &visibility, now)
            .await
            .expect("live visibility decision")
        {
            visible.push(event.event_type.clone());
        }
    }
    visible
}
