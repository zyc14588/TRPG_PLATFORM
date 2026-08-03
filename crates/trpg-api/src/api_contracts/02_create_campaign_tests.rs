mod tests {
    use super::{AcceptInviteApiRequest, IssueInviteApiRequest};

    #[test]
    fn invite_api_rejects_client_supplied_clock_fields() {
        let command = serde_json::json!({
            "command_id": "command_invite_clock",
            "idempotency_key": "idempotency_invite_clock",
            "expected_version": 0,
            "correlation_id": "correlation_invite_clock",
            "causation_id": "causation_invite_clock",
            "trace_id": "trace_invite_clock"
        });
        let issue = serde_json::json!({
            "command": command,
            "campaign_id": "campaign_invite_clock",
            "invite_id": "invite_clock",
            "invited_user_id": "player_invite_clock",
            "role": "PLAYER",
            "expires_at_unix_ms": 2_000_000_000_000_u64,
            "now_unix_ms": 1
        });
        assert!(serde_json::from_value::<IssueInviteApiRequest>(issue).is_err());

        let accept = serde_json::json!({
            "command": {
                "command_id": "command_accept_clock",
                "idempotency_key": "idempotency_accept_clock",
                "expected_version": 1,
                "correlation_id": "correlation_accept_clock",
                "causation_id": "causation_accept_clock",
                "trace_id": "trace_accept_clock"
            },
            "campaign_id": "campaign_invite_clock",
            "invite_id": "invite_clock",
            "accepting_user_id": "player_invite_clock",
            "raw_token": "opaque-token",
            "accepted_at_unix_ms": 1
        });
        assert!(serde_json::from_value::<AcceptInviteApiRequest>(accept).is_err());
    }
}
