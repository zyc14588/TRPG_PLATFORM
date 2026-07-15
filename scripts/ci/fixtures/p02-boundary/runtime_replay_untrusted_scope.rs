use trpg_runtime::{runtime, EventStore, PrincipalScope};

fn main() {
    let store = EventStore::default();
    let _ = runtime::replay_runtime_for_principal(&store, &PrincipalScope::Keeper);
}
