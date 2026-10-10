// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

export type Scope = { workspace_id: string; room_id: string; game_id: string }
export type Context = { state: 'anonymous' | 'authenticated'; csrf_token: string; principal?: { kind: 'account'; account_id: string; display_name: string } | { kind: 'guest'; participation_id: string; scope: Scope } }
export type Game = { game_id: string; configuration_id: string; title: string; content_tags: string[]; safety_tags: string[]; seats: { id: string; required: boolean; modes: ('human' | 'ai')[] }[] }
export type Room = Scope & { name: string; state: 'lobby' | 'launched' | 'closed'; owner_account_id: string; caller_roles: string[]; participants: { participant_id: string; display_name: string; kind: 'account' | 'guest'; roles: string[] }[] }
export type Slot = { id: string; mode: 'human' | 'ai' | 'empty'; participant_id?: string; model_selection?: string }
export type Consent = { revision: string; consent: boolean; ready: boolean; safety_confirmed: boolean; boundaries: string[] }
export type Lobby = { revision: string; configuration_id: string | null; configuration_hash: string | null; graph_hash: string | null; slots: Slot[]; own_consent: Consent; readiness: { ready: boolean; reasons: string[] }; can_manage: boolean }
export type Presentation = { workspace_id: string; configuration_id: string; game_id: string; configuration_hash: string; graph_hash: string; packages: { package_id: string; version: string; title: string; artifact_digest: string; rights_digest: string; license: string; permissions: string[] }[]; model_selections: { selection_id: string; label: string; seat_ids: string[]; capabilities: string[]; ready: boolean }[] }
export type Admission = Scope & { admission_id: string; status: 'pending' | 'approved' | 'rejected' | 'expired'; mode: 'account' | 'guest'; participant_id: string | null; display_name: string; expires_at: string }
export type Invitation = { invitation_id: string; invite_token: string; room_code: string; expires_at: string }
export type Value = { kind: 'nil' } | { kind: 'boolean'; boolean?: boolean } | { kind: 'integer' | 'float'; number: string } | { kind: 'string'; string?: string } | { kind: 'array'; array?: Value[] } | { kind: 'table'; table?: Record<string, Value> }
export type Control = { revision: string; paused: boolean; reason: 'none' | 'safety' | 'disconnect' | 'service_unavailable'; own_resume_required: boolean }
export type Connection = { connection_id: string; session_id: string; seat_id: string; expires_in_seconds: number; control: Control }
export type Event = { sequence: string; version: string; command_id: string; type: string; data: Value }
export type Snapshot = { session_id: string; seat_id: string; state_version: string; event_cursor: string; view: Value; events: Event[]; more: boolean; ended: boolean; control: Control }
export type Result = { command_id: string; state_version: string; event_cursor: string; replayed: boolean; result: Value; control: Control }
export type Export = Omit<Snapshot, 'control'> & { format_version: 1; kind: 'public' | 'personal' | 'host' | 'administrator'; next_cursor: string }
export type RecoveryPoint = { session_id: string; state_version: string; event_cursor: string }
