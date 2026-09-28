//! Проверки server-final RFC token-ов и mutual-authentication signature.
use super::*;

#[test]
fn valid_published_client_proof_produces_the_published_server_signature() {
    let (client, server, response, verifier) = rfc_exchange();
    let final_message = ServerFinal::complete(&client, &server, &response, &verifier);
    assert_eq!(final_message.encode(), "v=rmF9pqV8S7suAoZWja4dJRkFsKQ=");
}

#[test]
fn invalid_client_proof_produces_typed_invalid_proof_error() {
    let (client, server, mut response, verifier) = rfc_exchange();
    response.proof = [0; 20];
    let final_message = ServerFinal::complete(&client, &server, &response, &verifier);
    assert_eq!(final_message, ServerFinal::Error(ServerError::InvalidProof));
    assert_eq!(final_message.encode(), "e=invalid-proof");
}

#[test]
fn every_server_error_variant_maps_to_its_rfc_wire_token() {
    let values = [
        (ServerError::InvalidEncoding, "e=invalid-encoding"),
        (
            ServerError::ExtensionsNotSupported,
            "e=extensions-not-supported",
        ),
        (
            ServerError::ChannelBindingsDontMatch,
            "e=channel-bindings-dont-match",
        ),
        (
            ServerError::ServerDoesSupportChannelBinding,
            "e=server-does-support-channel-binding",
        ),
        (
            ServerError::ChannelBindingNotSupported,
            "e=channel-binding-not-supported",
        ),
        (
            ServerError::UnsupportedChannelBindingType,
            "e=unsupported-channel-binding-type",
        ),
        (ServerError::UnknownUser, "e=unknown-user"),
        (
            ServerError::InvalidUsernameEncoding,
            "e=invalid-username-encoding",
        ),
        (ServerError::NoResources, "e=no-resources"),
        (ServerError::Other, "e=other-error"),
    ];
    for (error, expected) in values {
        assert_eq!(ServerFinal::Error(error).encode(), expected);
    }
}

/// Строит полный известный RFC 5802 §5.1 SCRAM-SHA-1 пример.
fn rfc_exchange() -> (ClientFirst, ServerFirst, ClientFinal, ScramSha1Verifier) {
    let client = crate::sasl::scram_client_first::parse("n,,n=user,r=fyko+d2lbbFgONRv9qkxdawL")
        .expect("RFC client-first");
    let server = ServerFirst {
        nonce: "fyko+d2lbbFgONRv9qkxdawL3rfcNHYJY1ZVvWVs7j".to_owned(),
        message: "r=fyko+d2lbbFgONRv9qkxdawL3rfcNHYJY1ZVvWVs7j,s=QSXCR+Q6sek8bf92,i=4096"
            .to_owned(),
    };
    let response = crate::sasl::scram_client_final::parse(
        "c=biws,r=fyko+d2lbbFgONRv9qkxdawL3rfcNHYJY1ZVvWVs7j,p=v0X8v3Bz2T0CJGbJQyF0X+HI4Ts=",
        &server.nonce,
    )
    .expect("RFC client-final");
    let salt = crate::mime::decode_base64(b"QSXCR+Q6sek8bf92").expect("RFC salt");
    let verifier = ScramSha1Verifier::derive(b"pencil", &salt, 4096).expect("RFC verifier");
    (client, server, response, verifier)
}
