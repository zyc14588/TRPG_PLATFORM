#[test]
fn openapi_document_uses_the_canonical_event_registry() {
    let document = trpg_api::openapi::document();
    assert_eq!(
        document.event_registry,
        trpg_contracts::canonical_event_registry()
    );
    assert!(document
        .event_registry
        .iter()
        .all(|event| event.schema_version > 0 && !event.projection.is_empty()));
}
