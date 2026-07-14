package security_governance

default allow := false

allow if {
	input.openfga_decision == "PERMIT"
	permission_allowed
	not visibility_forbidden
}

permission_allowed if {
	input.principal_role == "server_owner"
	input.action == "pause_room"
}

permission_allowed if {
	input.principal_role == "moderator"
	input.action == "mute_player"
}

permission_allowed if {
	input.principal_role == "human_kp"
	input.authority_mode == "human_kp"
	input.action == "confirm_agent_draft"
}

permission_allowed if {
	input.principal_role == "player"
	input.authority_mode == "ai_kp"
	input.action == "request_reconsideration"
}

permission_allowed if {
	input.principal_role in {"workflow", "rules_engine", "system"}
	input.action in {"write_official_state", "record_audit"}
}

permission_allowed if {
	input.principal_role in {"workflow", "system"}
	input.action in {"delete_retained_data", "export_player_report", "generate_party_summary", "index_rag_chunk"}
}

permission_allowed if {
	input.principal_role == "system"
	input.action == "connect_provider"
}

permission_allowed if {
	input.principal_role in {"server_owner", "campaign_owner"}
	input.action == "manage_campaign_membership"
}

visibility_forbidden if {
	input.source_visibility == "keeper_only"
	input.target_output == "player_export"
}

visibility_forbidden if {
	startswith(input.source_visibility, "private_to_player")
	input.target_output == "party_summary"
}

visibility_forbidden if {
	input.source_visibility == "ai_internal"
	input.target_output == "player_export"
}

visibility_forbidden if {
	input.source_visibility == "ai_internal"
	input.target_output == "party_summary"
}

visibility_forbidden if {
	input.source_visibility == "ai_internal"
	input.target_output == "rag_chunk"
}
