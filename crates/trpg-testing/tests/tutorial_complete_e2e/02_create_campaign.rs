
async fn create_campaign(
    repository: &CoreDomainRepository,
    campaign_id: &str,
    authority_id: &str,
    room_id: &str,
    suffix: &str,
) -> i64 {
    repository
        .create_campaign(
            &metadata(
                authority_id,
                KEEPER_ID,
                "human_keeper",
                campaign_id,
                "campaign",
                0,
                suffix,
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &CreateCampaignRequest {
                campaign_id: campaign_id.to_owned(),
                owner_user_id: KEEPER_ID.to_owned(),
                title: format!("P08 Tutorial {suffix}"),
                room_id: room_id.to_owned(),
                room_name: "Tutorial table".to_owned(),
                created_at_unix_ms: if campaign_id == CAMPAIGN_ID {
                    NOW_MS
                } else {
                    NOW_MS + 1
                },
                authority: authority(authority_id),
            },
        )
        .await
        .expect("create event-backed Campaign")
        .last_event_sequence
}
