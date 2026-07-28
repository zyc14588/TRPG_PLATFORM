
impl CoreDomainRepository {

    async fn lock_unconsumed_gameplay_rolls(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        consumptions: &[GameplayRollConsumption],
        metadata: &CoreCommandMetadata,
    ) -> Result<(), CoreDomainRepositoryError> {
        let roll_ids = consumptions
            .iter()
            .map(|consumption| consumption.roll_id.as_str())
            .collect::<BTreeSet<_>>();
        for roll_id in roll_ids {
            sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
                .bind(format!("p08-gameplay-roll:{roll_id}"))
                .execute(&mut **transaction)
                .await
                .map_err(database_error("lock_gameplay_roll_consumption"))?;
            let existing = sqlx::query(
                r#"
                SELECT formal.commit_id
                  FROM public.gameplay_roll_consumptions AS consumption
                  LEFT JOIN public.formal_commits AS formal
                    ON consumption.last_event_sequence BETWEEN
                       formal.first_event_sequence AND formal.last_event_sequence
                   AND formal.status = 'committed'
                 WHERE consumption.roll_id = $1
                "#,
            )
            .bind(roll_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(database_error("load_gameplay_roll_consumption"))?;
            if let Some(existing) = existing {
                let owning_commit_id = existing.get::<Option<String>, _>("commit_id");
                if owning_commit_id.as_deref() == Some(metadata.commit_id.as_str()) {
                    continue;
                }
                return Err(CoreDomainRepositoryError::InvalidInput(
                    "gameplay_roll_reuse",
                ));
            }
        }
        Ok(())
    }

    async fn validate_initial_combat_participants(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        campaign_id: &str,
        session_id: &str,
        combat_id: &str,
        state_json: &str,
    ) -> Result<(), CoreDomainRepositoryError> {
        let requested = combat_participant_values(state_json)?;
        for participant_id in requested.keys() {
            sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
                .bind(format!(
                    "p08-combat-participant:{campaign_id}:{participant_id}"
                ))
                .execute(&mut **transaction)
                .await
                .map_err(database_error("lock_combat_participant"))?;
        }

        // The replay reader verifies the canonical HMAC/Witness chains and
        // decrypts the formal event payloads. Holding the participant locks
        // while reading prevents another initial Combat from racing this
        // authoritative health lookup.
        let replay_events = self.load_campaign_events(campaign_id).await?;
        let history =
            canonical_combat_participant_snapshots(&replay_events, campaign_id, combat_id)?;
        if let Some(initial_same_combat) = history.initial_same_combat {
            if initial_same_combat != requested {
                return Err(CoreDomainRepositoryError::Integrity(
                    "idempotent_combat_initial_state_conflict",
                ));
            }
            return Ok(());
        }

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
        .map_err(database_error("load_combat_scenario"))?
        .ok_or(CoreDomainRepositoryError::NotFound("combat_session"))?;
        let scenario_document: Value = scenario.get("document_json");
        let active_scene_key = scenario
            .get::<Option<String>, _>("active_scene_key")
            .ok_or(CoreDomainRepositoryError::Integrity("combat_active_scene"))?;
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
        .map_err(database_error("load_combat_character_profiles"))?;
        let mut character_profiles = BTreeMap::<String, Value>::new();
        for row in character_rows {
            let character_id: String = row.get("character_id");
            if !requested.contains_key(&character_id) {
                continue;
            }
            let sheet: Value = row.get("sheet_json");
            let profile = sheet.get("combat_profile").cloned().ok_or(
                CoreDomainRepositoryError::Integrity("combat_character_profile_missing"),
            )?;
            character_profiles.insert(character_id, profile);
        }
        let character_ids = character_profiles.keys().cloned().collect::<BTreeSet<_>>();
        if !scenario_combat_authorizes_participants(
            &scenario_document,
            &active_scene_key,
            &requested.keys().cloned().collect(),
            &character_ids,
        ) {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "combat_participant_authority",
            ));
        }
        let mut npc_profiles = BTreeMap::<String, Value>::new();
        let npcs = scenario_document
            .get("npcs")
            .and_then(Value::as_array)
            .ok_or(CoreDomainRepositoryError::Integrity("combat_scenario_npcs"))?;
        for npc in npcs {
            let npc_id = npc
                .get("id")
                .and_then(Value::as_str)
                .ok_or(CoreDomainRepositoryError::Integrity("combat_scenario_npc"))?;
            if !requested.contains_key(npc_id) {
                continue;
            }
            let profile =
                npc.get("combat_profile")
                    .cloned()
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "combat_npc_profile_missing",
                    ))?;
            if npc_profiles.insert(npc_id.to_owned(), profile).is_some() {
                return Err(CoreDomainRepositoryError::Integrity("combat_scenario_npc"));
            }
        }

        for (participant_id, actual) in &requested {
            let profile = character_profiles
                .get(participant_id)
                .or_else(|| npc_profiles.get(participant_id))
                .ok_or(CoreDomainRepositoryError::InvalidInput(
                    "combat_participant_authority",
                ))?;
            let mut expected = combat_profile_participant(participant_id, profile)?;
            if let Some(snapshot) = history.latest.get(participant_id) {
                if snapshot.combat_id != combat_id && snapshot.status == "ONGOING" {
                    return Err(CoreDomainRepositoryError::InvalidInput(
                        "combat_participant_already_active",
                    ));
                }
                let expected_fields =
                    expected
                        .as_object_mut()
                        .ok_or(CoreDomainRepositoryError::Integrity(
                            "combat_participant_profile",
                        ))?;
                let historical_fields = snapshot.participant.as_object().ok_or(
                    CoreDomainRepositoryError::Integrity("combat_participant_history"),
                )?;
                for field in ["current_hp", "condition"] {
                    expected_fields.insert(
                        field.to_owned(),
                        historical_fields.get(field).cloned().ok_or(
                            CoreDomainRepositoryError::Integrity("combat_participant_history"),
                        )?,
                    );
                }
                let mut expected_static = expected_fields.clone();
                let mut historical_static = historical_fields.clone();
                for field in ["current_hp", "condition"] {
                    expected_static.remove(field);
                    historical_static.remove(field);
                }
                if expected_static != historical_static {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "combat_participant_profile_history",
                    ));
                }
            }
            if &expected != actual {
                return Err(CoreDomainRepositoryError::InvalidInput(
                    "combat_participant_authority",
                ));
            }
        }
        Ok(())
    }
}
