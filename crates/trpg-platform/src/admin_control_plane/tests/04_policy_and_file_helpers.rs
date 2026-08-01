#[test]
fn openfga_policy_check_requires_an_explicit_allow_boolean() {
    assert!(openfga_check_allows(br#"{"allowed":true}"#));
    assert!(!openfga_check_allows(br#"{"allowed":false}"#));
    assert!(!openfga_check_allows(br#"{"result":true}"#));
}

fn request(
    method: &str,
    path: &str,
    bearer: &str,
    expected_version: Option<u64>,
    body: Value,
) -> AdminHttpRequest {
    request_with_key(
        method,
        path,
        bearer,
        expected_version,
        &format!(
            "key-{}-{}",
            method,
            path.trim_matches('/').replace('/', "-")
        ),
        body,
    )
}

fn request_with_key(
    method: &str,
    path: &str,
    bearer: &str,
    expected_version: Option<u64>,
    idempotency_key: &str,
    body: Value,
) -> AdminHttpRequest {
    let mut headers = HashMap::new();
    if !bearer.is_empty() {
        headers.insert("authorization".to_owned(), format!("Bearer {bearer}"));
    }
    if let Some(version) = expected_version {
        headers.insert("idempotency-key".to_owned(), idempotency_key.to_owned());
        headers.insert("x-expected-version".to_owned(), version.to_string());
        headers.insert("x-correlation-id".to_owned(), "test-correlation".to_owned());
        headers.insert("x-causation-id".to_owned(), "test-causation".to_owned());
    }
    AdminHttpRequest {
        method: method.to_owned(),
        path: path.to_owned(),
        headers,
        body: if body.is_null() {
            Vec::new()
        } else {
            serde_json::to_vec(&body).expect("encode request")
        },
    }
}

fn write_secret(root: &Path, id: &str, value: &[u8]) {
    let path = root.join(format!("{id}.v1"));
    fs::write(&path, value).expect("write secret");
    set_private_permissions(&path, 0o600).expect("protect secret");
}

fn read_tree_text(root: &Path) -> String {
    let mut output = String::new();
    for directory in ["state", "audit"] {
        let path = root.join(directory);
        for entry in fs::read_dir(path).expect("read evidence directory") {
            let path = entry.expect("directory entry").path();
            if path.is_file() {
                output.push_str(&fs::read_to_string(path).expect("read evidence file"));
            }
        }
    }
    output
}
