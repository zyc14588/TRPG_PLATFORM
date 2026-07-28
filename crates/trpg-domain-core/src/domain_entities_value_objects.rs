// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

macro_rules! stable_id {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub struct $name(EntityId);

        impl $name {
            pub fn new(value: impl Into<String>) -> CoreEntityResult<Self> {
                Ok(Self(
                    EntityId::new(value).map_err(|_| CoreEntityError::InvalidIdentifier)?,
                ))
            }

            pub fn as_entity_id(&self) -> &EntityId {
                &self.0
            }

            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

stable_id!(UserId);
stable_id!(CampaignId);
stable_id!(RoomId);
stable_id!(SessionId);
stable_id!(SceneId);
stable_id!(ScenarioId);
stable_id!(CharacterId);
stable_id!(CharacterSheetVersionId);
stable_id!(InviteId);
stable_id!(CampaignForkId);
stable_id!(ReconsiderationId);

include!("domain_entities_value_objects/01_module_prelude.rs");
include!("domain_entities_value_objects/02_campaign_invite_new.rs");
include!("domain_entities_value_objects/03_core_domain_event.rs");
