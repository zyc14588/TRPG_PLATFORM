# Permission Matrix

Batch: `BATCH-035-09-security-governance`

This matrix is derived from S04 fixture data and is enforced by `security_governance::permission_matrix`.

| Actor role | Authority mode | Action | Result |
|---|---:|---|---|
| ServerOwner | any | pause_room | ALLOW |
| ServerOwner | any | override_dice_roll | DENY |
| Moderator | any | mute_player | ALLOW |
| Moderator | any | change_game_decision | DENY |
| HumanKP | HUMAN_KP | confirm_agent_draft | ALLOW |
| Player | AI_KP | request_reconsideration | ALLOW |
| Player | AI_KP | override_ai_decision | DENY |

Additional hard gates:

- Workflow, rules engine, and system actors may write official state only through validated command envelopes.
- Agent and provider actors are denied formal state writes.
- Local model cloud fallback is denied unless fallback is explicitly enabled, user notice is present, and a snapshot is recorded.
