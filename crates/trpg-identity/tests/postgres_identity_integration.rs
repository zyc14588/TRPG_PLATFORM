use postgres::{Client, NoTls};
use trpg_identity::{CampaignRole, GlobalRole, IdentityError, IdentityService};
use trpg_shared_kernel::{
    AuthorityContract, AuthorityContractDraft, AuthorityMode, AuthorityVersionSnapshotDraft,
    EntityId,
};

const KEY: [u8; 32] = [0x63; 32];

fn contract(campaign_id: &str, owner: &str) -> AuthorityContract {
    AuthorityContract::new_locked(AuthorityContractDraft {
        contract_id: format!("authority_contract_{campaign_id}_1"),
        campaign_id: campaign_id.to_owned(),
        mode: AuthorityMode::HumanKp,
        authority_owner: owner.to_owned(),
        version: 1,
        snapshot: AuthorityVersionSnapshotDraft {
            ruleset_version: "coc7_rules_1".to_owned(),
            house_rules_version: "house_rules_1".to_owned(),
            scenario_version: "scenario_1".to_owned(),
            prompt_version: "prompt_1".to_owned(),
            agent_pack_version: "agent_pack_1".to_owned(),
            tool_schema_version: "tool_schema_1".to_owned(),
            safety_profile_version: "safety_profile_1".to_owned(),
            ai_provider_snapshot: "provider_snapshot_1".to_owned(),
            model_route_snapshot: "model_route_1".to_owned(),
            character_sheet_template_version: "character_template_1".to_owned(),
        },
        created_at_unix_ms: 1_000,
    })
    .unwrap()
}

#[test]
fn postgres_persists_sessions_memberships_and_canonical_authority_across_restart() {
    let database_url = std::env::var("P02_DATABASE_URL")
        .expect("P02_DATABASE_URL must identify a real PostgreSQL database");
    let suffix = std::process::id();
    let owner_id = format!("owner_{suffix}");
    let player_id = format!("player_{suffix}");
    let owner_login = format!("owner-{suffix}@example.test");
    let player_login = format!("player-{suffix}@example.test");
    let campaign_id = format!("campaign_{suffix}");

    let mut identity = IdentityService::from_postgres(&database_url, &KEY, 60_000).unwrap();
    assert!(identity.is_persistent());
    identity.check_readiness().unwrap();
    identity
        .create_user(
            &owner_id,
            &owner_login,
            "owner password long enough",
            GlobalRole::ServerOwner,
        )
        .unwrap();
    identity
        .create_user(
            &player_id,
            &player_login,
            "player password long enough",
            GlobalRole::User,
        )
        .unwrap();
    let owner_session = identity
        .login(&owner_login, "owner password long enough", 10_000)
        .unwrap();
    let owner = identity
        .authenticate_session(Some(owner_session.token.expose()), 10_001)
        .unwrap();
    identity
        .grant_membership(
            &owner,
            &campaign_id,
            &owner_id,
            CampaignRole::HumanKeeper,
            10_002,
        )
        .unwrap();
    identity
        .grant_membership(
            &owner,
            &campaign_id,
            &player_id,
            CampaignRole::Player,
            10_002,
        )
        .unwrap();
    identity
        .register_authority_contract(&owner, contract(&campaign_id, &owner_id), 10_002)
        .unwrap();
    let player_session = identity
        .login(&player_login, "player password long enough", 10_000)
        .unwrap();
    let persisted_token = player_session.token.expose().to_owned();
    drop(identity);

    let mut restarted = IdentityService::from_postgres(&database_url, &KEY, 60_000).unwrap();
    restarted.check_readiness().unwrap();
    let player = restarted
        .authenticate_session(Some(&persisted_token), 10_002)
        .unwrap();
    restarted
        .require_membership(
            &player,
            &EntityId::new(&campaign_id).unwrap(),
            &[CampaignRole::Player],
            10_002,
        )
        .unwrap();
    let authority = restarted
        .authority_contract(&EntityId::new(&campaign_id).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(authority.authority_owner().as_str(), owner_id);
    assert_eq!(
        restarted
            .register_authority_contract(&owner, contract(&campaign_id, "different_owner"), 10_002,)
            .unwrap_err(),
        IdentityError::AuthorityContractConflict
    );

    let replacement = restarted.refresh_session(&persisted_token, 10_003).unwrap();
    let replacement_token = replacement.token.expose().to_owned();
    let mut competing = IdentityService::from_postgres(&database_url, &KEY, 60_000).unwrap();
    assert_eq!(
        competing.refresh_session(&persisted_token, 10_004),
        Err(IdentityError::SessionRevoked)
    );
    let mut stale_reader = IdentityService::from_postgres(&database_url, &KEY, 60_000).unwrap();

    let mut database = Client::connect(&database_url, NoTls).unwrap();
    let raw_token_column = database
        .query_one(
            "SELECT count(*)::bigint FROM information_schema.columns \
             WHERE table_name = 'sessions' AND column_name = 'access_token'",
            &[],
        )
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(raw_token_column, 0);
    let token_hash_length = database
        .query_one(
            "SELECT octet_length(token_hash) FROM sessions \
             WHERE user_id = $1 ORDER BY issued_at DESC LIMIT 1",
            &[&player_id],
        )
        .unwrap()
        .get::<_, i32>(0);
    assert_eq!(token_hash_length, 32);
    assert!(database
        .execute(
            "UPDATE authority_contracts SET authority_owner = 'forged' \
             WHERE campaign_id = $1",
            &[&campaign_id],
        )
        .is_err());
    assert!(database
        .execute(
            "UPDATE campaign_memberships SET revoked_at = now() \
             WHERE campaign_id = $1 AND user_id = $2",
            &[&campaign_id, &owner_id],
        )
        .is_err());
    assert!(database
        .execute(
            "DELETE FROM campaign_memberships \
             WHERE campaign_id = $1 AND user_id = $2",
            &[&campaign_id, &owner_id],
        )
        .is_err());
    assert!(database
        .execute(
            "UPDATE campaign_memberships SET campaign_id = $3 \
             WHERE campaign_id = $1 AND user_id = $2",
            &[&campaign_id, &owner_id, &format!("{campaign_id}_moved")],
        )
        .is_err());
    database
        .execute(
            "UPDATE sessions SET revoked_at = now() \
             WHERE user_id = $1 AND revoked_at IS NULL",
            &[&player_id],
        )
        .unwrap();
    assert_eq!(
        stale_reader.authenticate_session(Some(&replacement_token), 10_005),
        Err(IdentityError::SessionRevoked)
    );
    assert!(database
        .execute(
            "UPDATE campaign_memberships SET role = 'HUMAN_KEEPER' \
             WHERE campaign_id = $1 AND user_id = $2",
            &[&campaign_id, &player_id],
        )
        .is_err());

    let genesis = "hmac-sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let previous_hash = database
        .query_opt(
            "SELECT record_hash FROM audit_log ORDER BY sequence DESC LIMIT 1",
            &[],
        )
        .unwrap()
        .map_or_else(|| genesis.to_owned(), |row| row.get::<_, String>(0));
    let record_hash = format!("hmac-sha256:{suffix:064x}");
    database
        .execute(
            "INSERT INTO audit_log (\
                actor_id, actor_origin, authentication_reference, campaign_id, \
                resource_type, resource_id, action, decision, openfga_decision_id, \
                openfga_policy_revision, opa_decision_id, opa_policy_revision, trace_id, \
                integrity_key_id, previous_hash, record_hash\
             ) VALUES (\
                $1, 'workload', $1, $2, 'campaign', $2, 'record_audit', 'PERMIT', \
                'openfga-test', 'model-test', 'opa-test', 'bundle-test', $3, \
                'integration-key-v1', $4, $5\
             )",
            &[
                &owner_id,
                &campaign_id,
                &format!("trace_{suffix}"),
                &previous_hash,
                &record_hash,
            ],
        )
        .unwrap();
    let forged_hash = format!("hmac-sha256:{:064x}", suffix as u64 + 1);
    assert!(database
        .execute(
            "INSERT INTO audit_log (\
                actor_id, actor_origin, authentication_reference, campaign_id, \
                resource_type, resource_id, action, decision, openfga_decision_id, \
                openfga_policy_revision, opa_decision_id, opa_policy_revision, trace_id, \
                integrity_key_id, previous_hash, record_hash\
             ) VALUES (\
                $1, 'workload', $1, $2, 'campaign', $2, 'record_audit', 'PERMIT', \
                'openfga-test', 'model-test', 'opa-test', 'bundle-test', $3, \
                'integration-key-v1', $4, $5\
             )",
            &[
                &owner_id,
                &campaign_id,
                &format!("trace_forged_{suffix}"),
                &genesis,
                &forged_hash
            ],
        )
        .is_err());
    assert!(database.batch_execute("TRUNCATE audit_log").is_err());
    assert!(database
        .batch_execute("TRUNCATE authority_contracts")
        .is_err());
}
