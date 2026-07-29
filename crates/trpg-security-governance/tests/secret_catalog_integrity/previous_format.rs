use super::*;

const PREVIOUS_GENESIS: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields, tag = "operation", rename_all = "snake_case")]
enum PreviousMutation {
    Register {
        backend: SecretBackend,
        secret_id: String,
        version: u64,
    },
    Rotate {
        backend: SecretBackend,
        secret_id: String,
        current_version: u64,
        replacement_version: u64,
    },
    Revoke {
        backend: SecretBackend,
        secret_id: String,
        version: u64,
    },
}

#[derive(serde::Serialize)]
struct PreviousPayload<'a> {
    schema_version: u32,
    sequence: u64,
    previous_hash: &'a str,
    source: &'static str,
    mutation: &'a PreviousMutation,
}

#[derive(serde::Serialize)]
struct PreviousAnchorPayload<'a> {
    schema_version: u32,
    sequence: u64,
    chain_head: &'a str,
}

pub fn replace_with_previous_anchored_format(path: &Path) {
    let mut previous_hash = PREVIOUS_GENESIS.to_owned();
    let mut lines = Vec::new();
    for (index, line) in catalog_lines(path).iter().enumerate() {
        let value: Value = serde_json::from_str(line).unwrap();
        let mutation: PreviousMutation = serde_json::from_value(value["mutation"].clone()).unwrap();
        let sequence = index as u64 + 1;
        let payload = serde_json::to_vec(&PreviousPayload {
            schema_version: 1,
            sequence,
            previous_hash: &previous_hash,
            source: "native",
            mutation: &mutation,
        })
        .unwrap();
        let record_hash = format!("sha256:{:x}", Sha256::digest(payload));
        lines.push(
            serde_json::json!({
                "schema_version": 1,
                "sequence": sequence,
                "previous_hash": previous_hash,
                "source": "native",
                "mutation": mutation,
                "record_hash": record_hash,
            })
            .to_string(),
        );
        previous_hash = record_hash;
    }
    write_lines(path, &lines);
    let payload = serde_json::to_vec(&PreviousAnchorPayload {
        schema_version: 1,
        sequence: lines.len() as u64,
        chain_head: &previous_hash,
    })
    .unwrap();
    let anchor_hash = format!("sha256:{:x}", Sha256::digest(payload));
    let anchor = serde_json::json!({
        "schema_version": 1,
        "sequence": lines.len(),
        "chain_head": previous_hash,
        "anchor_hash": anchor_hash,
    });
    fs::write(companion(path, ".head"), format!("{anchor}\n")).unwrap();
}
