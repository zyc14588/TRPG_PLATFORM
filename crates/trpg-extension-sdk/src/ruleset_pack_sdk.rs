crate::define_extension_sdk_module!(
    RulesetPackSdkCommand,
    RulesetPackSdkService,
    append_ruleset_pack_sdk_event,
    "CODEX-0106-12-EXTENSION-SDK-34e4277c8c",
    "ruleset_pack_sdk",
    "ExtensionRulesetPackSdkRecorded",
    crate::ExtensionOperation::RulesetPackSdk,
    ["ruleset_pack_manifest", "rules_engine_decision"],
    [
        crate::ExtensionCapability::RegisterRulesetPack,
        crate::ExtensionCapability::EmitProposedDecision,
        crate::ExtensionCapability::ReadProjection,
    ],
    "evidence/batches/BATCH-044/ruleset-pack-sdk.md"
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
    pub fn coc7_fixture() -> Self {
        Self {
            ruleset_id: "coc7".to_owned(),
            ruleset_version: "7e".to_owned(),
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
