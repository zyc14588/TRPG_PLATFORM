fn apply_nested_fork_character_materialization(
    replay: &CanonicalReplayEvent,
    campaign_id: &str,
    characters: &mut BTreeMap<String, ForkSnapshotCharacter>,
) -> Result<(), CoreDomainRepositoryError> {
    let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone()).map_err(|_| {
        CoreDomainRepositoryError::Integrity("fork_nested_materialization_payload")
    })?;
    let CoreDomainEvent::CampaignForkMaterialized {
        child_campaign_id,
        rows,
        ..
    } = event
    else {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_nested_materialization_event",
        ));
    };
    if child_campaign_id != campaign_id {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_nested_materialization_campaign",
        ));
    }
    for row in rows {
        let CampaignForkMaterializedRow::Character {
            character_id,
            owner_user_id,
            display_name,
            state,
            initial_version_locked,
            sheet_json,
            sheet_locked,
            visibility_label,
            visibility_subject,
            ..
        } = row
        else {
            continue;
        };
        if visibility_label != replay.visibility_label
            || visibility_subject != replay.visibility_subject
            || characters.contains_key(&character_id)
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_nested_character_chain",
            ));
        }
        let sheet_json: Value = serde_json::from_str(&sheet_json)
            .map_err(|_| CoreDomainRepositoryError::Integrity("fork_nested_character_sheet"))?;
        characters.insert(
            character_id.clone(),
            ForkSnapshotCharacter {
                character_id,
                owner_user_id,
                display_name,
                state,
                initial_version_locked,
                visibility_label: visibility_label.clone(),
                visibility_subject: visibility_subject.clone(),
                current_sheet: Some(ForkSnapshotSheet {
                    sheet_json,
                    locked: sheet_locked,
                    visibility_label,
                    visibility_subject,
                }),
            },
        );
    }
    Ok(())
}
