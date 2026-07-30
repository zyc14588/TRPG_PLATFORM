use trpg_platform::admin_control_plane::provider_probe_confirms_model;

#[test]
fn provider_probe_requires_the_exact_configured_model() {
    let openai = br#"{"data":[{"id":"model-a"},{"id":"model-b"}]}"#;
    assert!(provider_probe_confirms_model(openai, "model-b"));
    assert!(!provider_probe_confirms_model(openai, "model-c"));

    let local = br#"{"models":[{"name":"local-a"},{"model":"local-b"}]}"#;
    assert!(provider_probe_confirms_model(local, "local-a"));
    assert!(provider_probe_confirms_model(local, "local-b"));
    assert!(!provider_probe_confirms_model(
        b"<html>ok</html>",
        "local-a"
    ));
    assert!(!provider_probe_confirms_model(
        br#"{"data":[{"id":"model-a"}],"api_key":"secret"} trailing"#,
        "model-a"
    ));
}
