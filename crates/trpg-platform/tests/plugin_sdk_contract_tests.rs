use trpg_platform::plugin_sdk::{
    register_plugin_tool_grant, PluginSdkRepository, RegisterPluginToolGrant,
    PLUGIN_SDK_METRIC_MODULE, PLUGIN_SDK_REQUIRED_METRICS, PLUGIN_TOOL_GRANT_REGISTERED_EVENT,
};
use trpg_shared_kernel::{
    ActorRole, AuthorityMode, CommandEnvelope, FormalWritePath, PrincipalScope, TrpgError,
    Visibility, VisibilityLabel,
};

fn command() -> CommandEnvelope<RegisterPluginToolGrant> {
    trpg_test_support::governed_command(
        RegisterPluginToolGrant {
            plugin_id: "investigator_helper".to_owned(),
            tool_name: "read_public_projection".to_owned(),
            granted_write_path: FormalWritePath::ToolDecision,
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    )
}

#[test]
fn plugin_sdk_rejects_authority_contract_violation() {
    let command = trpg_test_support::governed_command(
        command().payload,
        ActorRole::AiKeeper,
        AuthorityMode::HumanKp,
    );
    let mut repository = PluginSdkRepository::default();

    let err = register_plugin_tool_grant(&mut repository, &command).expect_err("authority denied");

    assert_eq!(err, TrpgError::AuthorityViolation);
    assert!(repository.events().is_empty());
}

#[test]
fn plugin_sdk_keeps_visibility_and_fact_provenance_on_replay() {
    let mut command = command();
    command.visibility = Visibility::new(VisibilityLabel::SystemPrivate);
    let mut repository = PluginSdkRepository::default();

    let event =
        register_plugin_tool_grant(&mut repository, &command).expect("plugin grant recorded");

    assert_eq!(event.event_type, PLUGIN_TOOL_GRANT_REGISTERED_EVENT);
    assert_eq!(event.fact_provenance, command.fact_provenance);
    assert!(repository
        .replay_visible(&PrincipalScope::Public)
        .is_empty());
    assert_eq!(repository.replay_visible(&PrincipalScope::System).len(), 1);
}

#[test]
fn plugin_sdk_rejects_direct_state_write_grants() {
    for write_path in [
        FormalWritePath::DirectAgent,
        FormalWritePath::DirectBusiness,
    ] {
        let mut command = command();
        command.payload.granted_write_path = write_path;
        let mut repository = PluginSdkRepository::default();

        let err =
            register_plugin_tool_grant(&mut repository, &command).expect_err("direct write denied");

        assert_eq!(err, TrpgError::PolicyDenied);
        assert!(repository.events().is_empty());
    }
}

#[test]
fn plugin_sdk_uses_current_safe_event_and_metric_names() {
    assert_eq!(PLUGIN_SDK_METRIC_MODULE, "plugin_sdk");
    assert_eq!(
        PLUGIN_TOOL_GRANT_REGISTERED_EVENT,
        "platform.plugin_sdk.tool_grant_registered"
    );
    assert!(!PLUGIN_TOOL_GRANT_REGISTERED_EVENT.contains("impl"));
    assert!(PLUGIN_SDK_REQUIRED_METRICS.contains(&"trpg_command_total"));
    assert!(PLUGIN_SDK_REQUIRED_METRICS.contains(&"trpg_visibility_redaction_total"));
}
