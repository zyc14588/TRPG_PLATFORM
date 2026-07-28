// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("campaign_character_api_integration/01_module_prelude.rs");
include!("campaign_character_api_integration/02_repository_campaign_character_port_metadata.rs");
include!("campaign_character_api_integration/03_campaign_invite_and_character_api_use_the_real_repository.rs");
