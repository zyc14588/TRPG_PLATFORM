use api_server::ApiApplication;

fn steal_canonical_store(application: &ApiApplication) {
    let _ = &application.canonical_custody;
}

fn main() {}
