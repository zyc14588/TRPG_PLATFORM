
impl CoreDomainRepository {

    async fn validate_initial_chase_participants(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        campaign_id: &str,
        session_id: &str,
        chase_id: &str,
        state_json: &str,
    ) -> Result<(), CoreDomainRepositoryError> {
        let requested = chase_participant_values(state_json)?;
        let requested_state: Value = serde_json::from_str(state_json)
            .map_err(|_| CoreDomainRepositoryError::Integrity("chase_participant_state"))?;
        for participant_id in requested.keys() {
            sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
                .bind(format!(
                    "p08-chase-participant:{campaign_id}:{participant_id}"
                ))
                .execute(&mut **transaction)
                .await
                .map_err(database_error("lock_chase_participant"))?;
        }

        let replay_events = self.load_campaign_events(campaign_id).await?;
        if let Some(initial) = canonical_initial_chase_state(&replay_events, campaign_id, chase_id)?
        {
            if initial != requested_state {
                return Err(CoreDomainRepositoryError::Integrity(
                    "idempotent_chase_initial_state_conflict",
                ));
            }
            return Ok(());
        }

        let inspected = inspect_chase_state(state_json)
            .map_err(|_| CoreDomainRepositoryError::Integrity("chase_participant_state"))?;
        let scenario = sqlx::query(
            r#"
            SELECT scenario.document_json, active_scene.scene_key AS active_scene_key
              FROM core_domain.sessions AS session
              JOIN public.scenarios AS scenario
                ON scenario.scenario_id = session.scenario_id
               AND scenario.campaign_id = session.campaign_id
              JOIN public.scenes AS active_scene
                ON active_scene.scene_id = session.active_scene_id
               AND active_scene.session_id = session.session_id
               AND active_scene.campaign_id = session.campaign_id
             WHERE session.session_id = $1
               AND session.campaign_id = $2
            "#,
        )
        .bind(session_id)
        .bind(campaign_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(database_error("load_chase_scenario"))?
        .ok_or(CoreDomainRepositoryError::NotFound("chase_session"))?;
        let scenario_document: Value = scenario.get("document_json");
        let active_scene_key = scenario
            .get::<Option<String>, _>("active_scene_key")
            .ok_or(CoreDomainRepositoryError::Integrity("chase_active_scene"))?;

        let character_rows = sqlx::query(
            r#"
            SELECT character.character_id, sheet.sheet_json
              FROM public.characters AS character
              JOIN public.character_sheet_versions AS sheet
                ON sheet.character_id = character.character_id
               AND sheet.version = character.current_sheet_version
               AND sheet.campaign_id = character.campaign_id
             WHERE character.campaign_id = $1
               AND character.state = 'APPROVED'
               AND character.initial_version_locked
               AND sheet.locked
            "#,
        )
        .bind(campaign_id)
        .fetch_all(&mut **transaction)
        .await
        .map_err(database_error("load_chase_character_profiles"))?;
        let mut character_profiles = BTreeMap::<String, Value>::new();
        for row in character_rows {
            let character_id: String = row.get("character_id");
            if !requested.contains_key(&character_id) {
                continue;
            }
            let sheet: Value = row.get("sheet_json");
            let profile =
                sheet
                    .get("chase_profile")
                    .cloned()
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "chase_character_profile_missing",
                    ))?;
            character_profiles.insert(character_id, profile);
        }
        let character_ids = character_profiles.keys().cloned().collect::<BTreeSet<_>>();
        if !scenario_chase_authorizes_participants(
            &scenario_document,
            &active_scene_key,
            &requested.keys().cloned().collect(),
            &character_ids,
            inspected.range(),
        ) {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "chase_participant_authority",
            ));
        }

        let mut npc_profiles = BTreeMap::<String, Value>::new();
        let npcs = scenario_document
            .get("npcs")
            .and_then(Value::as_array)
            .ok_or(CoreDomainRepositoryError::Integrity("chase_scenario_npcs"))?;
        for npc in npcs {
            let npc_id = npc
                .get("id")
                .and_then(Value::as_str)
                .ok_or(CoreDomainRepositoryError::Integrity("chase_scenario_npc"))?;
            if !requested.contains_key(npc_id) {
                continue;
            }
            let profile =
                npc.get("chase_profile")
                    .cloned()
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "chase_npc_profile_missing",
                    ))?;
            if npc_profiles.insert(npc_id.to_owned(), profile).is_some() {
                return Err(CoreDomainRepositoryError::Integrity("chase_scenario_npc"));
            }
        }

        for (participant_id, actual) in &requested {
            let profile = character_profiles
                .get(participant_id)
                .or_else(|| npc_profiles.get(participant_id))
                .ok_or(CoreDomainRepositoryError::InvalidInput(
                    "chase_participant_authority",
                ))?;
            let expected = chase_profile_participant(participant_id, profile)?;
            if &expected != actual {
                return Err(CoreDomainRepositoryError::InvalidInput(
                    "chase_participant_authority",
                ));
            }
        }
        Ok(())
    }

    async fn lock_active_gameplay_session(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        campaign_id: &str,
        session_id: &str,
    ) -> Result<(), CoreDomainRepositoryError> {
        let state = sqlx::query_scalar::<_, String>(
            r#"
            SELECT state
              FROM core_domain.sessions
             WHERE session_id = $1
               AND campaign_id = $2
             FOR SHARE
            "#,
        )
        .bind(session_id)
        .bind(campaign_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(database_error("lock_gameplay_session"))?
        .ok_or(CoreDomainRepositoryError::NotFound("gameplay_session"))?;
        if state != "ACTIVE" {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "gameplay_session_state",
            ));
        }
        Ok(())
    }
}
