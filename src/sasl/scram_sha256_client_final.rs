//! Разбирает SCRAM-SHA-256 client-final и строит AuthMessage transcript.

use super::{
    scram_attributes::{self, AttributeName, ParseError},
    scram_client_first::ClientFirst,
    scram_sha256_server_first::ServerFirst,
};

/// Client proof имеет фиксированную длину SHA-256.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientFinal<'a> {
    pub client_final_without_proof: &'a str,
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
pub fn parse<'a>(
    message: &'a str,
    expected_nonce: &str,
) -> Result<ClientFinal<'a>, ClientFinalError> {
    parse_with_binding(message, expected_nonce, b"n,,")
}

/// Проверяет точное SASL `c=` значение для предоставленного GS2 header и binding.
pub fn parse_with_binding<'a>(
    message: &'a str,
    expected_nonce: &str,
    expected_binding: &[u8],
) -> Result<ClientFinal<'a>, ClientFinalError> {
    let attributes =
        scram_attributes::parse_client_final(message).map_err(ClientFinalError::Attributes)?;
    let (channel_binding, nonce, proof) = match attributes.as_slice() {
        [channel_binding, nonce, extensions @ .., proof] => match (
            channel_binding.name,
            nonce.name,
            proof.name,
            extensions
                .iter()
                .all(|extension| matches!(extension.name, AttributeName::Extension(_))),
        ) {
            (AttributeName::ChannelBinding, AttributeName::Nonce, AttributeName::Proof, true) => {
                (channel_binding.value, nonce.value, proof.value)
            }
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
        client_final_without_proof: without_proof,
        proof,
    })
}

impl ClientFinal<'_> {
    /// Собирает исходные поля SCRAM transcript без нормализации.
    pub fn auth_message(&self, client: &ClientFirst, server: &ServerFirst) -> Vec<u8> {
        let mut message = Vec::with_capacity(
            client.client_first_bare.len()
                + server.message.len()
                + self.client_final_without_proof.len()
                + 2,
        );
        message.extend_from_slice(client.client_first_bare.as_bytes());
        message.push(b',');
        message.extend_from_slice(server.message.as_bytes());
        message.push(b',');
        message.extend_from_slice(self.client_final_without_proof.as_bytes());
        message
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
