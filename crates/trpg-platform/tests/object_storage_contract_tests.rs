use trpg_platform::object_storage::{public_object_descriptor, store_object, StoreObject};
use trpg_platform::{PlatformEvent, PlatformEventStore};
use trpg_shared_kernel::{ActorRole, AuthorityMode, Visibility, VisibilityLabel};

#[test]
fn restricted_object_descriptor_is_redacted() {
    let mut command = trpg_test_support::governed_command!(
        StoreObject {
            object_id: "object_001".to_owned(),
            display_name: "keeper clue handout".to_owned(),
            content_type: "text/markdown".to_owned(),
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    );
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);

    let descriptor = public_object_descriptor(&command);

    assert_eq!(descriptor.display_name, "[redacted]");
}

#[test]
fn object_store_event_uses_redacted_descriptor() {
    let mut command = trpg_test_support::governed_command!(
        StoreObject {
            object_id: "object_001".to_owned(),
            display_name: "private dossier".to_owned(),
            content_type: "application/pdf".to_owned(),
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    );
    command.visibility = Visibility::new(VisibilityLabel::PrivateToPlayer);
    let mut store = PlatformEventStore::default();

    let event = store_object(&mut store, &command).expect("object stored");

    assert!(matches!(
        event.payload,
        PlatformEvent::ObjectStored {
            display_name,
            ..
        } if display_name == "[redacted]"
    ));
}
