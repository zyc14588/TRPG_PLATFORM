use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoomRole {
    Owner,
    Kp,
    AssistantKp,
    Pl,
    Observer,
    PublicScreen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VisibilityScope {
    PublicRule,
    RoomRule,
    PlVisibleClue,
    KpOnlyModule,
    KpSecret,
    CharacterPrivate,
    SessionLog,
    MemoryPrivate,
    SystemInternal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoomPrivacyMode {
    Standard,
    PrivateHybrid,
    LocalOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthContext {
    pub user_id: Uuid,
    pub room_id: Option<Uuid>,
    pub role: RoomRole,
}

impl AuthContext {
    pub fn can_view(&self, scope: VisibilityScope) -> bool {
        match scope {
            VisibilityScope::PublicRule
            | VisibilityScope::RoomRule
            | VisibilityScope::PlVisibleClue => true,
            VisibilityScope::CharacterPrivate | VisibilityScope::SessionLog => {
                !matches!(self.role, RoomRole::PublicScreen)
            }
            VisibilityScope::KpOnlyModule
            | VisibilityScope::KpSecret
            | VisibilityScope::MemoryPrivate => {
                matches!(
                    self.role,
                    RoomRole::Owner | RoomRole::Kp | RoomRole::AssistantKp
                )
            }
            VisibilityScope::SystemInternal => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pl_cannot_view_kp_secret() {
        let ctx = AuthContext {
            user_id: Uuid::nil(),
            room_id: Some(Uuid::nil()),
            role: RoomRole::Pl,
        };

        assert!(!ctx.can_view(VisibilityScope::KpSecret));
        assert!(ctx.can_view(VisibilityScope::PlVisibleClue));
    }
}
