
impl RepositoryCampaignCharacterPort {
    #[allow(clippy::too_many_arguments)]
    fn metadata(
        context: &AuthorizedCoreApiContext,
        command: &ApiCommandFields,
        _stream_id: &str,
        _resource_type: &str,
        _action: &str,
        visibility_label: &str,
        visibility_subject: &str,
    ) -> CoreCommandMetadata {
        let provenance_kind = if context.actor_role() == "human_keeper" {
            "human_keeper_statement"
        } else {
            "user_statement"
        };
        CoreCommandMetadata {
            commit_id: format!("commit_{}", command.command_id),
            command_id: command.command_id.clone(),
            idempotency_key: command.idempotency_key.clone(),
            expected_version: command.expected_version,
            requesting_actor_id: context.actor_id().to_owned(),
            requesting_actor_role: context.actor_role().to_owned(),
            authenticated_actor_id: context.workflow_actor_id().to_owned(),
            authenticated_actor_role: context.workflow_actor_role().to_owned(),
            authenticated_actor_origin: EventActorOriginWire::Workload {
                role: "workflow_engine".to_owned(),
            },
            authority_mode: context.authority_mode().to_owned(),
            authority_contract_version: context.authority_contract_version(),
            authority_contract_id: context.authority_contract_id().to_owned(),
            authority_owner: context.authority_owner().to_owned(),
            visibility_label: visibility_label.to_owned(),
            visibility_subject: visibility_subject.to_owned(),
            data_subject_id: if visibility_subject == "not_applicable" {
                "not_applicable".to_owned()
            } else {
                visibility_subject.to_owned()
            },
            provenance_kind: provenance_kind.to_owned(),
            provenance_reference: command.command_id.clone(),
            provenance_recorded_by: context.actor_id().to_owned(),
            correlation_id: command.correlation_id.clone(),
            causation_id: command.causation_id.clone(),
            trace_id: command.trace_id.clone(),
            audit: PolicyAuditDraft {
                actor_id: context.policy_audit().actor_id.clone(),
                actor_origin: context.policy_audit().actor_origin.clone(),
                authentication_reference: context.policy_audit().authentication_reference.clone(),
                resource_type: context.policy_audit().resource_type.clone(),
                resource_id: context.policy_audit().resource_id.clone(),
                action: context.policy_audit().action.clone(),
                requested_role: context.policy_audit().requested_role.clone(),
                openfga_decision_id: context.policy_audit().openfga_decision_id.clone(),
                openfga_policy_revision: context.policy_audit().openfga_policy_revision.clone(),
                opa_decision_id: context.policy_audit().opa_decision_id.clone(),
                opa_policy_revision: context.policy_audit().opa_policy_revision.clone(),
            },
        }
    }

    fn receipt(
        persisted: trpg_data_eventing::event_store_sqlx_outbox_projection::PersistedCommit,
    ) -> CoreApiCommitReceipt {
        CoreApiCommitReceipt {
            last_event_sequence: persisted.last_event_sequence,
            aggregate_version: persisted.last_stream_version,
        }
    }

    fn map_error(error: CoreDomainRepositoryError) -> CoreApiError {
        match error {
            CoreDomainRepositoryError::Forbidden
            | CoreDomainRepositoryError::PolicyEvidenceMismatch => CoreApiError::Forbidden,
            CoreDomainRepositoryError::InvalidInput(field) => CoreApiError::InvalidInput(field),
            CoreDomainRepositoryError::Domain(_) => CoreApiError::Conflict("domain_transition"),
            CoreDomainRepositoryError::ConcurrentStart => {
                CoreApiError::Conflict("concurrent_start")
            }
            CoreDomainRepositoryError::Integrity(_) => CoreApiError::Conflict("integrity_conflict"),
            CoreDomainRepositoryError::NotFound(_) => CoreApiError::Conflict("not_found"),
            CoreDomainRepositoryError::Canonical(_)
            | CoreDomainRepositoryError::Database(_)
            | CoreDomainRepositoryError::Serialization => CoreApiError::Unavailable("repository"),
        }
    }
}

impl CampaignCharacterCommandPort for RepositoryCampaignCharacterPort {
    fn create_campaign<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a CreateCampaignApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let persisted = self
                .repository
                .create_campaign(
                    &Self::metadata(
                        context,
                        &request.command,
                        &request.campaign_id,
                        "campaign",
                        "campaign.create",
                        "party_visible",
                        "not_applicable",
                    ),
                    &CreateCampaignRequest {
                        campaign_id: request.campaign_id.clone(),
                        owner_user_id: request.owner_user_id.clone(),
                        title: request.title.clone(),
                        room_id: request.room_id.clone(),
                        room_name: request.room_name.clone(),
                        created_at_unix_ms: request.created_at_unix_ms,
                        authority: AuthorityContractSnapshot {
                            contract_id: request.authority.contract_id.clone(),
                            authority_mode: request.authority.authority_mode.clone(),
                            authority_owner: request.authority.authority_owner.clone(),
                            ruleset_version: request.authority.ruleset_version.clone(),
                            house_rules_version: request.authority.house_rules_version.clone(),
                            scenario_version: request.authority.scenario_version.clone(),
                            prompt_version: request.authority.prompt_version.clone(),
                            agent_pack_version: request.authority.agent_pack_version.clone(),
                            tool_schema_version: request.authority.tool_schema_version.clone(),
                            safety_profile_version: request
                                .authority
                                .safety_profile_version
                                .clone(),
                            ai_provider_snapshot: request.authority.ai_provider_snapshot.clone(),
                            model_route_snapshot: request.authority.model_route_snapshot.clone(),
                            character_sheet_template_version: request
                                .authority
                                .character_sheet_template_version
                                .clone(),
                        },
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn issue_invite<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a IssueInviteApiRequest,
    ) -> CoreApiFuture<'a, IssuedInviteApiResponse> {
        Box::pin(async move {
            let role = match request.role.as_str() {
                "PLAYER" => MembershipRole::Player,
                "SPECTATOR" => MembershipRole::Spectator,
                _ => return Err(CoreApiError::InvalidInput("invite_role")),
            };
            let issued = self
                .repository
                .issue_invite(
                    &Self::metadata(
                        context,
                        &request.command,
                        &request.invite_id,
                        "campaign_invite",
                        "campaign.invite.issue",
                        "private_to_player",
                        &request.invited_user_id,
                    ),
                    &IssueInviteRequest {
                        invite_id: request.invite_id.clone(),
                        campaign_id: request.campaign_id.clone(),
                        invited_user_id: request.invited_user_id.clone(),
                        role,
                        expires_at_unix_ms: request.expires_at_unix_ms,
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(IssuedInviteApiResponse {
                invite_id: issued.invite_id,
                raw_token: issued.raw_token,
                expires_at_unix_ms: issued.expires_at_unix_ms,
                receipt: Self::receipt(issued.persisted),
            })
        })
    }

    fn accept_invite<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a AcceptInviteApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let persisted = self
                .repository
                .accept_invite(
                    &Self::metadata(
                        context,
                        &request.command,
                        &request.invite_id,
                        "campaign_invite",
                        "campaign.invite.accept",
                        "private_to_player",
                        &request.accepting_user_id,
                    ),
                    &AcceptInviteRequest {
                        campaign_id: request.campaign_id.clone(),
                        invite_id: request.invite_id.clone(),
                        accepting_user_id: request.accepting_user_id.clone(),
                        raw_token: request.raw_token.clone(),
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn create_character<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a CreateCharacterApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let sheet: Coc7CharacterSheet = serde_json::from_str(&request.sheet_json)
                .map_err(|_| CoreApiError::InvalidInput("coc7_character_sheet"))?;
            sheet
                .validate()
                .map_err(|_| CoreApiError::InvalidInput("coc7_character_sheet"))?;
            let persisted = self
                .repository
                .create_character(
                    &Self::metadata(
                        context,
                        &request.command,
                        &request.character_id,
                        "character",
                        "character.create",
                        "private_to_player",
                        &request.owner_user_id,
                    ),
                    &CreateCharacterRequest {
                        character_id: request.character_id.clone(),
                        campaign_id: request.campaign_id.clone(),
                        owner_user_id: request.owner_user_id.clone(),
                        display_name: request.display_name.clone(),
                        sheet_version_id: request.sheet_version_id.clone(),
                        sheet_json: request.sheet_json.clone(),
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn submit_character<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a CharacterTransitionApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let persisted = self
                .repository
                .submit_character(
                    &Self::metadata(
                        context,
                        &request.command,
                        &request.character_id,
                        "character",
                        "character.submit",
                        "private_to_player",
                        context.actor_id(),
                    ),
                    &request.campaign_id,
                    &request.character_id,
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn review_character<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a CharacterTransitionApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let owner_user_id: String = sqlx::query_scalar(
                "SELECT owner_user_id FROM public.characters WHERE character_id = $1",
            )
            .bind(&request.character_id)
            .fetch_one(&self.repository.primary_pool())
            .await
            .map_err(|_| CoreApiError::Conflict("character_not_found"))?;
            let persisted = self
                .repository
                .approve_character_initial_version(
                    &Self::metadata(
                        context,
                        &request.command,
                        &request.character_id,
                        "character",
                        "character.review_initial",
                        "private_to_player",
                        &owner_user_id,
                    ),
                    &request.campaign_id,
                    &request.character_id,
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }
}

fn valid_sheet_json() -> String {
    serde_json::to_string(&Coc7CharacterSheet {
        name: "Evelyn Hart".to_owned(),
        age: 31,
        occupation: "Investigative journalist".to_owned(),
        era: "1920s".to_owned(),
        birthplace: "Brisbane".to_owned(),
        characteristics: Coc7Characteristics {
            strength: 50,
            dexterity: 60,
            power: 65,
            constitution: 55,
            size: 50,
            appearance: 55,
            intelligence: 70,
            education: 75,
            luck: 60,
        },
        skills: BTreeMap::from([
            ("Library Use".to_owned(), 70),
            ("Psychology".to_owned(), 55),
        ]),
        backstory_anchors: vec![
            "Protects confidential sources".to_owned(),
            "Distrusts official explanations".to_owned(),
        ],
    })
    .unwrap()
}
