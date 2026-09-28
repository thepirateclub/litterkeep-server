//! Разбирает SCRAM-SHA-256 client-final и строит AuthMessage transcript.

use super::{
    scram_attributes::{self, AttributeName, ParseError},
    scram_client_first::ClientFirst,
    scram_sha256_server_first::ServerFirst,
};

/// Client proof имеет фиксированную длину SHA-256.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientFinal {
    pub client_final_without_proof: String,
    pub proof: [u8; 32],
}

/// Причины отклонения client-final без утечки недоверенных полей.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientFinalError {
    Attributes(ParseError),
    InvalidFieldSequence,
    InvalidChannelBinding,
    NonceMismatch,
    InvalidProofEncoding,
}

/// Проверяет RFC 5802 поле-порядок и GS2 `n,,` без channel binding.
pub fn parse(message: &str, expected_nonce: &str) -> Result<ClientFinal, ClientFinalError> {
    parse_with_binding(message, expected_nonce, b"n,,")
}

/// Проверяет точное SASL `c=` значение для предоставленного GS2 header и binding.
pub fn parse_with_binding(
    message: &str,
    expected_nonce: &str,
    expected_binding: &[u8],
) -> Result<ClientFinal, ClientFinalError> {
    let attributes = scram_attributes::parse(message).map_err(ClientFinalError::Attributes)?;
    let (channel_binding, nonce, proof) = match attributes.as_slice() {
        [channel_binding, nonce, extensions @ .., proof] => match (
            channel_binding.name,
            nonce.name,
            proof.name,
            extensions
                .iter()
                .all(|extension| matches!(extension.name, AttributeName::Extension(_))),
        ) {
            (AttributeName::ChannelBinding, AttributeName::Nonce, AttributeName::Proof, true) => (
                channel_binding.value.as_str(),
                nonce.value.as_str(),
                proof.value.as_str(),
            ),
            _ => return Err(ClientFinalError::InvalidFieldSequence),
        },
        _ => return Err(ClientFinalError::InvalidFieldSequence),
    };
    validate_binding(channel_binding, expected_binding)?;
    if nonce != expected_nonce {
        return Err(ClientFinalError::NonceMismatch);
    }
    let proof = decode_proof(proof)?;
    let (without_proof, _) = message
        .rsplit_once(",p=")
        .ok_or(ClientFinalError::InvalidFieldSequence)?;
    Ok(ClientFinal {
        client_final_without_proof: without_proof.to_owned(),
        proof,
    })
}

impl ClientFinal {
    /// Собирает исходные поля SCRAM transcript без нормализации.
    pub fn auth_message(&self, client: &ClientFirst, server: &ServerFirst) -> Vec<u8> {
        format!(
            "{},{},{}",
            client.client_first_bare, server.message, self.client_final_without_proof
        )
        .into_bytes()
    }
}

fn validate_binding(value: &str, expected: &[u8]) -> Result<(), ClientFinalError> {
    let decoded = crate::mime::decode_base64(value.as_bytes())
        .map_err(|_| ClientFinalError::InvalidChannelBinding)?;
    match (
        decoded.as_slice() == expected,
        crate::sasl::encode_base64(&decoded) == value,
    ) {
        (true, true) => Ok(()),
        _ => Err(ClientFinalError::InvalidChannelBinding),
    }
}

fn decode_proof(value: &str) -> Result<[u8; 32], ClientFinalError> {
    let decoded = crate::mime::decode_base64(value.as_bytes())
        .map_err(|_| ClientFinalError::InvalidProofEncoding)?;
    match (
        decoded.len() == 32,
        crate::sasl::encode_base64(&decoded) == value,
    ) {
        (true, true) => decoded
            .try_into()
            .map_err(|_| ClientFinalError::InvalidProofEncoding),
        _ => Err(ClientFinalError::InvalidProofEncoding),
    }
}
