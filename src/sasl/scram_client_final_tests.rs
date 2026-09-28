//! Проверки SCRAM client-final и полного published RFC proof transcript.
use super::*;

const CLIENT_NONCE: &str = "fyko+d2lbbFgONRv9qkxdawL";
const SERVER_NONCE: &str = "fyko+d2lbbFgONRv9qkxdawL3rfcNHYJY1ZVvWVs7j";
const SERVER_FIRST: &str = "r=fyko+d2lbbFgONRv9qkxdawL3rfcNHYJY1ZVvWVs7j,s=QSXCR+Q6sek8bf92,i=4096";

#[test]
fn rfc_5802_exchange_builds_exact_auth_message_and_validates_proof() {
    let client = crate::sasl::scram_client_first::parse(&format!("n,,n=user,r={CLIENT_NONCE}"))
        .expect("RFC client-first");
    let server = ServerFirst {
        nonce: SERVER_NONCE.to_owned(),
        message: SERVER_FIRST.to_owned(),
    };
    let final_message = format!("c=biws,r={SERVER_NONCE},p=v0X8v3Bz2T0CJGbJQyF0X+HI4Ts=");
    let client_final = parse(&final_message, SERVER_NONCE).expect("RFC client-final");
    let auth_message = client_final.auth_message(&client, &server);
    let verifier = verifier();

    assert_eq!(
        auth_message,
        format!("n=user,r={CLIENT_NONCE},{SERVER_FIRST},c=biws,r={SERVER_NONCE}").as_bytes()
    );
    assert!(verifier.verify_client_proof(&auth_message, &client_final.proof));
    assert_eq!(
        crate::sasl::encode_base64(&verifier.server_signature(&auth_message)),
        "rmF9pqV8S7suAoZWja4dJRkFsKQ="
    );
}

#[test]
fn rejects_wrong_channel_binding_nonce_field_order_and_optional_tail_after_proof() {
    for invalid in [
        format!("c=AAAA,r={SERVER_NONCE},p=AAAAAAAAAAAAAAAAAAAAAAAAAAA="),
        "c=biws,r=wrong,p=AAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_owned(),
        format!("r={SERVER_NONCE},c=biws,p=AAAAAAAAAAAAAAAAAAAAAAAAAAA="),
        format!("c=biws,r={SERVER_NONCE},p=AAAAAAAAAAAAAAAAAAAAAAAAAAA=,x=extension"),
    ] {
        assert!(
            parse(&invalid, SERVER_NONCE).is_err(),
            "accepted {invalid:?}"
        );
    }
}

#[test]
fn optional_extensions_are_included_in_auth_message_but_not_interpreted() {
    let final_message =
        format!("c=biws,r={SERVER_NONCE},x=extension,p=v0X8v3Bz2T0CJGbJQyF0X+HI4Ts=");
    let parsed = parse(&final_message, SERVER_NONCE).expect("optional extension");
    assert_eq!(
        parsed.client_final_without_proof,
        format!("c=biws,r={SERVER_NONCE},x=extension")
    );
}

#[test]
fn rejects_noncanonical_or_wrong_length_proof() {
    for proof in [
        "AA",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAA===",
    ] {
        let message = format!("c=biws,r={SERVER_NONCE},p={proof}");
        assert_eq!(
            parse(&message, SERVER_NONCE).err(),
            Some(ClientFinalError::InvalidProofEncoding)
        );
    }
}

/// Производит ключи для опубликованного RFC SCRAM-SHA-1 пользователя.
fn verifier() -> crate::sasl::scram_sha1_verifier::ScramSha1Verifier {
    let salt = crate::mime::decode_base64(b"QSXCR+Q6sek8bf92").expect("RFC salt");
    crate::sasl::scram_sha1_verifier::ScramSha1Verifier::derive(b"pencil", &salt, 4096)
        .expect("RFC verifier")
}
