
fn fork_row_projection_targets(
    row: &CampaignForkMaterializedRow,
) -> Vec<CanonicalProjectionTarget> {
    match row {
        CampaignForkMaterializedRow::Scenario { scenario_id, .. } => {
            vec![projection_target("public.scenarios", scenario_id)]
        }
        CampaignForkMaterializedRow::Character {
            character_id,
            sheet_version_id,
            ..
        } => vec![
            projection_target("public.characters", character_id),
            projection_target("public.character_sheet_versions", sheet_version_id),
        ],
        CampaignForkMaterializedRow::Session { session_id, .. } => {
            vec![projection_target("core_domain.sessions", session_id)]
        }
        CampaignForkMaterializedRow::Scene { scene_id, .. } => {
            vec![projection_target("public.scenes", scene_id)]
        }
        CampaignForkMaterializedRow::PublicEvent { fork_event_id, .. } => {
            vec![projection_target(
                "public.campaign_fork_public_events",
                fork_event_id,
            )]
        }
        CampaignForkMaterializedRow::DiscoveredClue { fork_clue_id, .. } => {
            vec![projection_target(
                "public.campaign_fork_clues",
                fork_clue_id,
            )]
        }
        CampaignForkMaterializedRow::NpcState { npc_state_id, .. } => {
            vec![projection_target(
                "public.campaign_fork_npc_states",
                npc_state_id,
            )]
        }
        CampaignForkMaterializedRow::Combat { combat_id, .. } => {
            vec![projection_target("public.combat_states", combat_id)]
        }
        CampaignForkMaterializedRow::Chase { chase_id, .. } => {
            vec![projection_target("public.chase_states", chase_id)]
        }
        CampaignForkMaterializedRow::Conclusion {
            ending_event_id, ..
        } => vec![projection_target("public.ending_events", ending_event_id)],
    }
}

fn fork_row_visibility(row: &CampaignForkMaterializedRow) -> (&str, &str) {
    match row {
        CampaignForkMaterializedRow::Scenario {
            visibility_label,
            visibility_subject,
            ..
        }
        | CampaignForkMaterializedRow::Character {
            visibility_label,
            visibility_subject,
            ..
        }
        | CampaignForkMaterializedRow::Session {
            visibility_label,
            visibility_subject,
            ..
        }
        | CampaignForkMaterializedRow::Scene {
            visibility_label,
            visibility_subject,
            ..
        }
        | CampaignForkMaterializedRow::PublicEvent {
            visibility_label,
            visibility_subject,
            ..
        }
        | CampaignForkMaterializedRow::DiscoveredClue {
            visibility_label,
            visibility_subject,
            ..
        }
        | CampaignForkMaterializedRow::NpcState {
            visibility_label,
            visibility_subject,
            ..
        }
        | CampaignForkMaterializedRow::Combat {
            visibility_label,
            visibility_subject,
            ..
        }
        | CampaignForkMaterializedRow::Chase {
            visibility_label,
            visibility_subject,
            ..
        }
        | CampaignForkMaterializedRow::Conclusion {
            visibility_label,
            visibility_subject,
            ..
        } => (visibility_label, visibility_subject),
    }
}

fn fork_row_data_subject(row: &CampaignForkMaterializedRow) -> String {
    let (visibility_label, visibility_subject) = fork_row_visibility(row);
    if matches!(
        visibility_label,
        "private_to_player" | "private_to_group" | "investigator_private"
    ) {
        visibility_subject.to_owned()
    } else {
        "not_applicable".to_owned()
    }
}

fn fork_materialization_batches(
    rows: &[CampaignForkMaterializedRow],
) -> Result<Vec<CampaignForkMaterializationBatch>, CoreDomainRepositoryError> {
    const MAX_TARGETS_PER_EVENT: usize = 32;
    const MAX_ROWS_JSON_BYTES_PER_EVENT: usize = 786_432;
    let mut visibility_groups =
        BTreeMap::<(String, String, String), Vec<CampaignForkMaterializedRow>>::new();
    for row in rows {
        let (label, subject) = fork_row_visibility(row);
        visibility_groups
            .entry((
                label.to_owned(),
                subject.to_owned(),
                fork_row_data_subject(row),
            ))
            .or_default()
            .push(row.clone());
    }
    let mut batches = Vec::new();
    for ((visibility_label, visibility_subject, data_subject_id), grouped_rows) in visibility_groups
    {
        let mut current = Vec::new();
        let mut current_targets = 0_usize;
        for row in grouped_rows {
            let row_targets = row.projection_target_count();
            if row_targets == 0 || row_targets > MAX_TARGETS_PER_EVENT {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_projection_target_shape",
                ));
            }
            let mut candidate = current.clone();
            candidate.push(row.clone());
            let candidate_size = serde_json::to_vec(&candidate)
                .map_err(|_| CoreDomainRepositoryError::Serialization)?
                .len();
            if !current.is_empty()
                && (current_targets + row_targets > MAX_TARGETS_PER_EVENT
                    || candidate_size > MAX_ROWS_JSON_BYTES_PER_EVENT)
            {
                batches.push(CampaignForkMaterializationBatch {
                    rows: std::mem::take(&mut current),
                    visibility_label: visibility_label.clone(),
                    visibility_subject: visibility_subject.clone(),
                    data_subject_id: data_subject_id.clone(),
                });
                current_targets = 0;
            }
            if serde_json::to_vec(&row)
                .map_err(|_| CoreDomainRepositoryError::Serialization)?
                .len()
                > MAX_ROWS_JSON_BYTES_PER_EVENT
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_materialized_row_payload_limit",
                ));
            }
            current.push(row);
            current_targets += row_targets;
        }
        if !current.is_empty() {
            batches.push(CampaignForkMaterializationBatch {
                rows: current,
                visibility_label,
                visibility_subject,
                data_subject_id,
            });
        }
    }
    if batches.is_empty() {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_materialization_empty",
        ));
    }
    Ok(batches)
}
