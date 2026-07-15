use trpg_contracts::{canonical_event_registry, CanonicalEventHeader};
use trpg_data_eventing::event_store_projections::canonical_projection_route;

#[test]
fn every_canonical_event_routes_to_its_registered_projection() {
    for descriptor in canonical_event_registry() {
        let route = canonical_projection_route(CanonicalEventHeader::new(descriptor.event_type));
        assert_eq!(route.event_name, descriptor.name);
        assert_eq!(route.schema_version, descriptor.schema_version);
        assert_eq!(route.schema_id, descriptor.schema_id);
        assert_eq!(route.projection, descriptor.projection);
    }
}
