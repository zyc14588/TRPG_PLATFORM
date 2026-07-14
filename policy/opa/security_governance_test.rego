package security_governance

test_default_deny if {
	not allow with input as {"principal_role": "player", "action": "override_ai_decision"}
}

test_visibility_keeper_only_player_export_denied if {
	not allow with input as {
		"principal_role": "server_owner",
		"action": "pause_room",
		"openfga_decision": "PERMIT",
		"source_visibility": "keeper_only",
		"target_output": "player_export",
	}
}

test_visibility_private_to_player_party_summary_denied if {
	not allow with input as {
		"principal_role": "server_owner",
		"action": "pause_room",
		"openfga_decision": "PERMIT",
		"source_visibility": "private_to_player:user_player_a",
		"target_output": "party_summary",
	}
}

test_visibility_ai_internal_export_denied if {
	not allow with input as {
		"principal_role": "server_owner",
		"action": "pause_room",
		"openfga_decision": "PERMIT",
		"source_visibility": "ai_internal",
		"target_output": "player_export",
	}
}

test_permission_server_owner_pause_room_allowed if {
	allow with input as {
		"principal_role": "server_owner",
		"action": "pause_room",
		"openfga_decision": "PERMIT",
		"source_visibility": "public",
		"target_output": "debug_log",
	}
}

test_permission_server_owner_override_dice_roll_denied if {
	not allow with input as {
		"principal_role": "server_owner",
		"action": "override_dice_roll",
		"openfga_decision": "PERMIT",
		"source_visibility": "public",
		"target_output": "debug_log",
	}
}

test_permission_moderator_mute_player_allowed if {
	allow with input as {
		"principal_role": "moderator",
		"action": "mute_player",
		"openfga_decision": "PERMIT",
		"source_visibility": "public",
		"target_output": "debug_log",
	}
}

test_permission_moderator_change_game_decision_denied if {
	not allow with input as {
		"principal_role": "moderator",
		"action": "change_game_decision",
		"openfga_decision": "PERMIT",
		"source_visibility": "public",
		"target_output": "debug_log",
	}
}

test_permission_human_kp_confirm_agent_draft_allowed if {
	allow with input as {
		"principal_role": "human_kp",
		"authority_mode": "human_kp",
		"action": "confirm_agent_draft",
		"openfga_decision": "PERMIT",
		"source_visibility": "public",
		"target_output": "debug_log",
	}
}

test_permission_player_request_reconsideration_allowed if {
	allow with input as {
		"principal_role": "player",
		"authority_mode": "ai_kp",
		"action": "request_reconsideration",
		"openfga_decision": "PERMIT",
		"source_visibility": "public",
		"target_output": "debug_log",
	}
}

test_permission_player_override_ai_decision_denied if {
	not allow with input as {
		"principal_role": "player",
		"authority_mode": "ai_kp",
		"action": "override_ai_decision",
		"openfga_decision": "PERMIT",
		"source_visibility": "public",
		"target_output": "debug_log",
	}
}

test_permission_server_owner_manage_membership_allowed if {
	allow with input as {
		"principal_role": "server_owner",
		"authority_mode": "human_kp",
		"action": "manage_campaign_membership",
		"openfga_decision": "PERMIT",
		"source_visibility": "system_only",
		"target_output": "debug_log",
	}
}
