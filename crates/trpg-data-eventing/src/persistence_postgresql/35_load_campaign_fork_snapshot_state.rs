async fn load_campaign_fork_snapshot_state(
    primary: &PgPool,
    parent_campaign_id: &str,
    source_session_id: &str,
) -> Result<Value, CoreDomainRepositoryError> {
    let snapshot_query = sqlx::query_scalar(
        r#"
        WITH snapshot_gameplay AS (
            SELECT source_session.*,
                   GREATEST(
                       source_session.last_event_sequence,
                       COALESCE((
                           SELECT max(combat.last_event_sequence)
                             FROM public.combat_states AS combat
                            WHERE combat.session_id = source_session.session_id
                       ), 0),
                       COALESCE((
                           SELECT max(chase.last_event_sequence)
                             FROM public.chase_states AS chase
                            WHERE chase.session_id = source_session.session_id
                       ), 0),
                       COALESCE((
                           SELECT max(ending.last_event_sequence)
                             FROM public.ending_events AS ending
                            WHERE ending.session_id = source_session.session_id
                       ), 0),
                       COALESCE((
                           SELECT max(growth.last_event_sequence)
                             FROM public.growth_events AS growth
                            WHERE growth.session_id = source_session.session_id
                       ), 0)
                   ) AS gameplay_cutoff_event_sequence
             FROM core_domain.sessions AS source_session
             WHERE source_session.session_id = $1
               AND source_session.campaign_id = $2
               AND source_session.state = 'ENDED'
        ),
        snapshot_source AS (
            SELECT snapshot_gameplay.*,
                   snapshot_gameplay.gameplay_cutoff_event_sequence
                       AS snapshot_cutoff_event_sequence
              FROM snapshot_gameplay
        )
        SELECT jsonb_build_object(
            'source_campaign_id', source_session.campaign_id,
            'source_session_id', source_session.session_id,
            'source_cutoff_event_sequence',
                source_session.snapshot_cutoff_event_sequence,
            'session_state', jsonb_build_object(
                'state', source_session.state,
                'active_scene_id', source_session.active_scene_id,
                'version', source_session.version,
                'visibility_label', source_session.visibility_label,
                'visibility_subject', source_session.visibility_subject,
                'started_at_unix_ms',
                    floor(extract(epoch FROM source_session.started_at) * 1000)::BIGINT,
                'ended_at_unix_ms',
                    floor(extract(epoch FROM source_session.ended_at) * 1000)::BIGINT
            ),
            'character_state', '[]'::JSONB,
            'public_events', '[]'::JSONB,
            'discovered_clues', COALESCE((
                SELECT jsonb_agg(
                    jsonb_build_object(
                        'clue_id', clue.clue_id,
                        'importance', clue.importance,
                        'outcome', clue.outcome,
                        'cost', clue.cost,
                        'version', clue.version,
                        'visibility_label', clue.visibility_label,
                        'visibility_subject', clue.visibility_subject
                    )
                    ORDER BY clue.clue_id
                )
                  FROM public.clues AS clue
                 WHERE clue.campaign_id = source_session.campaign_id
                   AND (
                        clue.last_event_sequence < (
                            SELECT min(source_start.sequence)
                              FROM public.event_store AS source_start
                             WHERE source_start.campaign_id =
                                   source_session.campaign_id
                               AND source_start.stream_id =
                                   source_session.session_id
                               AND source_start.event_type = 'SessionStarted'
                               AND source_start.integrity_status = 'verified_hmac'
                               AND source_start.request_hash_source = 'formal_commit'
                        )
                        OR EXISTS(
                            SELECT 1
                              FROM public.player_actions AS action
                              JOIN public.scenes AS action_scene
                                ON action_scene.scene_id = action.scene_id
                             WHERE action.action_id = clue.action_id
                               AND action_scene.session_id =
                                   source_session.session_id
                        )
                   )
                   AND clue.revealed_to_party
                   AND clue.outcome <> 'NOT_FOUND'
                   AND clue.visibility_label::TEXT
                       IN ('public', 'party_visible')
            ), '[]'::JSONB),
            'scene_state', COALESCE((
                SELECT jsonb_agg(
                    jsonb_build_object(
                        'scene_id', scene.scene_id,
                        'scene_key', scene.scene_key,
                        'name', scene.name,
                        'state', scene.state,
                        'version', scene.version,
                        'visibility_label', scene.visibility_label,
                        'visibility_subject', scene.visibility_subject
                    )
                    ORDER BY scene.scene_id
                )
                  FROM public.scenes AS scene
                 WHERE scene.session_id = source_session.session_id
                   AND scene.last_event_sequence
                       <= source_session.snapshot_cutoff_event_sequence
                   AND scene.visibility_label::TEXT
                       IN ('public', 'party_visible')
            ), '[]'::JSONB),
            'world_state', jsonb_build_object(
                'room_id', source_session.room_id,
                'scenario_id', source_session.scenario_id,
                'ruleset_id', (
                    SELECT scenario.ruleset_id
                      FROM public.scenarios AS scenario
                     WHERE scenario.scenario_id = source_session.scenario_id
                       AND scenario.campaign_id = source_session.campaign_id
                ),
                'visibility_label', (
                    SELECT scenario.visibility_label
                      FROM public.scenarios AS scenario
                     WHERE scenario.scenario_id = source_session.scenario_id
                       AND scenario.campaign_id = source_session.campaign_id
                ),
                'visibility_subject', (
                    SELECT scenario.visibility_subject
                      FROM public.scenarios AS scenario
                     WHERE scenario.scenario_id = source_session.scenario_id
                       AND scenario.campaign_id = source_session.campaign_id
                )
            ),
            'combat_state', COALESCE((
                SELECT jsonb_agg(
                    jsonb_build_object(
                        'combat_id', combat.combat_id,
                        'status', combat.status,
                        'round', combat.round,
                        'current_turn_index', combat.current_turn_index,
                        'state', combat.state_json,
                        'version', combat.version,
                        'visibility_label', combat.visibility_label,
                        'visibility_subject', combat.visibility_subject
                    )
                    ORDER BY combat.combat_id
                )
                  FROM public.combat_states AS combat
                 WHERE combat.session_id = source_session.session_id
                   AND combat.last_event_sequence
                       <= source_session.snapshot_cutoff_event_sequence
                   AND combat.visibility_label::TEXT
                       IN ('public', 'party_visible')
            ), '[]'::JSONB),
            'chase_state', COALESCE((
                SELECT jsonb_agg(
                    jsonb_build_object(
                        'chase_id', chase.chase_id,
                        'status', chase.status,
                        'range_band', chase.range_band,
                        'segment', chase.segment,
                        'state', chase.state_json,
                        'version', chase.version,
                        'visibility_label', chase.visibility_label,
                        'visibility_subject', chase.visibility_subject
                    )
                    ORDER BY chase.chase_id
                )
                  FROM public.chase_states AS chase
                 WHERE chase.session_id = source_session.session_id
                   AND chase.last_event_sequence
                       <= source_session.snapshot_cutoff_event_sequence
                   AND chase.visibility_label::TEXT
                       IN ('public', 'party_visible')
            ), '[]'::JSONB),
            'conclusion_state', COALESCE((
                SELECT jsonb_agg(
                    jsonb_build_object(
                        'ending_event_id', ending.ending_event_id,
                        'ending_id', ending.ending_id,
                        'summary', ending.summary,
                        'growth_awards', (
                            SELECT COALESCE(
                                scenario_ending -> 'growth_awards',
                                '[]'::JSONB
                            )
                              FROM public.scenarios AS scenario
                              CROSS JOIN LATERAL jsonb_array_elements(
                                  scenario.document_json -> 'endings'
                              ) AS scenario_ending
                             WHERE scenario.scenario_id =
                                   source_session.scenario_id
                               AND scenario.campaign_id =
                                   source_session.campaign_id
                               AND scenario_ending ->> 'id' =
                                   ending.ending_id
                             LIMIT 1
                        ),
                        'consumed_growth_awards', COALESCE((
                            SELECT jsonb_agg(
                                jsonb_build_object(
                                    'character_id', growth.character_id,
                                    'skill_name', growth.skill_name
                                )
                                ORDER BY growth.character_id, growth.skill_name
                            )
                              FROM public.growth_events AS growth
                             WHERE growth.ending_event_id =
                                   ending.ending_event_id
                               AND growth.session_id =
                                   source_session.session_id
                               AND growth.last_event_sequence
                                   <= source_session.snapshot_cutoff_event_sequence
                        ), '[]'::JSONB),
                        'version', ending.version,
                        'ended_at_unix_ms',
                            floor(extract(epoch FROM ending.ended_at) * 1000)::BIGINT,
                        'visibility_label', ending.visibility_label,
                        'visibility_subject', ending.visibility_subject
                    )
                    ORDER BY ending.ending_event_id
                )
                  FROM public.ending_events AS ending
                 WHERE ending.session_id = source_session.session_id
                   AND ending.last_event_sequence
                       <= source_session.snapshot_cutoff_event_sequence
                   AND ending.visibility_label::TEXT
                       IN ('public', 'party_visible')
            ), '[]'::JSONB),
            'npc_state', '[]'::JSONB
        )
          FROM snapshot_source AS source_session
        "#,
    )
    .bind(source_session_id)
    .bind(parent_campaign_id);
    let state: Value = snapshot_query
        .fetch_optional(primary)
        .await
        .map_err(database_error("load_campaign_fork_snapshot"))?
        .ok_or(CoreDomainRepositoryError::NotFound("fork_source_session"))?;
    Ok(state)
}
