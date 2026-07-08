package security_governance

default allow := false

allow if {
	input.openfga_decision == "PERMIT"
	input.opa_decision == "PERMIT"
	permission_allowed
	not visibility_forbidden
}

permission_allowed if {
	input.actor_role == "ServerOwner"
	input.action == "pause_room"
}

permission_allowed if {
	input.actor_role == "Moderator"
	input.action == "mute_player"
}

permission_allowed if {
	input.actor_role == "HumanKP"
	input.authority_mode == "HUMAN_KP"
	input.action == "confirm_agent_draft"
}

permission_allowed if {
	input.actor_role == "Player"
	input.authority_mode == "AI_KP"
	input.action == "request_reconsideration"
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
