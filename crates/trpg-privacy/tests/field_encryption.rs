use trpg_privacy::{PayloadCipher, PrivacyError};

#[test]
fn protected_payload_round_trips_without_persisting_plaintext() {
    let cipher = PayloadCipher::new("payload_key_v1", &[0x31; 32]).unwrap();
    let plaintext = br#"{"keeper_truth":"the lighthouse ledger"}"#;
    let aad = ["campaign_a", "stream_a", "command_a", "ClueRecorded"];

    let first = cipher.encrypt_json(plaintext, &aad).unwrap();
    let second = cipher.encrypt_json(plaintext, &aad).unwrap();
    let stored = serde_json::to_string(&first).unwrap();
    assert!(!stored.contains("keeper_truth"));
    assert!(!stored.contains("lighthouse ledger"));
    assert_ne!(first, second, "a fresh nonce is required for every field");
    assert_eq!(
        cipher.decrypt_json(&first, &aad).unwrap().as_bytes(),
        plaintext
    );
    assert!(matches!(
        cipher.decrypt_json(
            &first,
            &["campaign_b", "stream_a", "command_a", "ClueRecorded"]
        ),
        Err(PrivacyError::Cryptography)
    ));

    let fields = cipher.encrypt_json_field(plaintext, &aad).unwrap();
    assert_eq!(fields.nonce().len(), 12);
    assert!(fields.ciphertext().len() >= plaintext.len() + 16);
    assert_eq!(fields.key_reference().as_str(), "payload_key_v1");
    assert!(!fields
        .ciphertext()
        .windows(b"lighthouse ledger".len())
        .any(|window| window == b"lighthouse ledger"));
    assert_eq!(
        cipher
            .decrypt_json(fields.envelope(), &aad)
            .unwrap()
            .as_bytes(),
        plaintext
    );
}
