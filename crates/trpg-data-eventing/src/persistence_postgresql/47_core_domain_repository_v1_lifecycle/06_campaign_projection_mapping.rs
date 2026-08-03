
fn campaign_projection_from_row(row: sqlx::postgres::PgRow) -> CampaignProjection {
    CampaignProjection {
        campaign_id: row.get("campaign_id"),
        owner_user_id: row.get("owner_user_id"),
        authority_contract_id: row.get("authority_contract_id"),
        title: row.get("title"),
        state: row.get("state"),
        aggregate_version: row.get("version"),
        last_event_sequence: row.get("last_event_sequence"),
    }
}
