crate::define_extension_sdk_module!(
    RulesetPackSdkCommand,
    RulesetPackSdkService,
    append_ruleset_pack_sdk_event,
    "ruleset_pack_sdk",
    "ExtensionRulesetPackSdkRecorded",
    crate::ExtensionOperation::RulesetPackSdk,
    ["ruleset_pack_manifest", "rules_engine_decision"],
    [
        crate::ExtensionCapability::RegisterRulesetPack,
        crate::ExtensionCapability::EmitProposedDecision,
        crate::ExtensionCapability::ReadProjection,
    ],
    "extensions/ruleset-pack"
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiceAuthority {
    ServerOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RulesetPackManifest {
    pub ruleset_id: String,
    pub ruleset_version: String,
    pub dice_authority: DiceAuthority,
}

impl RulesetPackManifest {
    pub fn new(ruleset_id: impl Into<String>, ruleset_version: impl Into<String>) -> Self {
        Self {
            ruleset_id: ruleset_id.into(),
            ruleset_version: ruleset_version.into(),
            dice_authority: DiceAuthority::ServerOnly,
        }
    }

    pub fn official_dice_must_be_server_generated(&self) -> bool {
        self.dice_authority == DiceAuthority::ServerOnly
    }

    pub fn pack_can_forge_dice(&self) -> bool {
        false
    }
}
