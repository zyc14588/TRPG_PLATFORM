use super::*;

#[test]
fn certification_registry_rejects_old_snapshot_and_crash_boundary_mismatches() {
    let root = test_root();
    let path = root.join("certification-registry.jsonl");
    let authority = open_authority(&path);
    issue(&authority, "model-one");
    let old_log = fs::read(&path).unwrap();
    issue(&authority, "model-two");
    drop(authority);
    fs::write(&path, old_log).unwrap();
    assert!(open_authority_result(&path).is_err());
    fs::remove_dir_all(root).unwrap();

    let root = test_root();
    let path = root.join("certification-registry.jsonl");
    let authority = open_authority(&path);
    issue(&authority, "model-one");
    let anchor_path = companion(&path, ".head");
    let old_anchor = fs::read(&anchor_path).unwrap();
    issue(&authority, "model-two");
    drop(authority);
    fs::write(anchor_path, old_anchor).unwrap();
    assert!(open_authority_result(&path).is_err());
    fs::remove_dir_all(root).unwrap();

    let root = test_root();
    let path = root.join("certification-registry.jsonl");
    let authority = open_authority(&path);
    issue(&authority, "model-one");
    let witness_path = companion(&path, ".external-witness");
    let old_witness = fs::read(&witness_path).unwrap();
    issue(&authority, "model-two");
    drop(authority);
    fs::write(witness_path, old_witness).unwrap();
    assert!(
        open_authority_result(&path).is_err(),
        "synced local state ahead of its external checkpoint must fail closed"
    );
    fs::remove_dir_all(root).unwrap();
}
